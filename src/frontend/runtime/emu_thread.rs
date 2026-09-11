//! Emulation thread loop — sole owner of [`Machine`].

use super::super::audio::{AudioOut, bind_bus_host_rate};
use super::super::boot_rom::{load_cached_boot_rom, load_cached_cgb_boot_rom};
use super::super::input::HostCommand;
use super::super::launch::Verbosity;
use super::super::pace::{FRAME_DURATION, FpsCounter, HostScheduler, HostWake};
use super::super::playback::{
    SpeedPreset, StepPulse, apply_audio_transition_if_needed, effective_speed,
    run_until_frame_or_budget, should_submit_audio,
};
use super::super::rewind::RewindRing;
use super::super::state_slots::{PendingSlotOp, load_slot, save_slot};
use super::super::telemetry::HostTelemetry;
use super::RuntimeConfig;
use super::commands::EmuCommand;
use super::frame::{FramePacket, LatestDebug, LatestFrame};
use super::policy::{
    FRAME_ADVANCE_FLASH, FRAME_STEP_BUDGET, catch_up_cap, pending_slot_ready, should_allow_snapshot,
};
use graycart::hw::CGB_BOOT_ROM_SIZE;
use graycart::{
    BootMode, Bus, Cpu, ExecSession, HardwareModel, MachineDebug, bus_from_cartridge, capture,
    flush_save, prepare_boot, restore,
};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc::Receiver;
use std::thread;
use std::time::{Duration, Instant};

const T_CYCLES_PER_FRAME: u64 = 70_224;
const DEBUG_PUBLISH_INTERVAL: Duration = Duration::from_millis(66);
const RUNTIME_DIAG_INTERVAL: Duration = Duration::from_secs(1);

struct Machine {
    path: PathBuf,
    save_path: PathBuf,
    title: String,
    cpu: Cpu,
    bus: Bus,
    session: ExecSession,
}

pub(crate) fn emu_loop(
    config: RuntimeConfig,
    cmd_rx: Receiver<EmuCommand>,
    latest: LatestFrame,
    latest_debug: LatestDebug,
) {
    let mut state = EmuState::new(config);
    let runtime_diag =
        env_flag("GRAYCART_RUNTIME_DIAG") || state.config.verbosity == Verbosity::Verbose;

    while !state.quit {
        if !drain_commands(&mut state, &cmd_rx) {
            break;
        }
        if state.quit {
            break;
        }

        if !state.ready {
            // Idle until Start: sleep + try_recv only (never block on presentation).
            thread::sleep(Duration::from_millis(8));
            continue;
        }

        let mut packet = state.run_tick();
        packet.frame_publish_replaced = latest.replaced_count();
        latest.publish(packet);
        state.maybe_publish_debug(&latest_debug);
        if runtime_diag {
            state.maybe_log_runtime_diag(&latest);
        }

        if state.quit {
            break;
        }

        let ff_hold = state.host_down.contains(&HostCommand::FastForwardHold);
        let rewinding = state.rewind_setting && state.host_down.contains(&HostCommand::Rewind);
        let speed = if state.config.unthrottled {
            SpeedPreset::Unlimited
        } else {
            effective_speed(
                state.paused,
                rewinding,
                ff_hold,
                state.ff_toggle,
                state.ff_speed,
            )
        };
        let unlimited = state.config.unthrottled || speed == SpeedPreset::Unlimited;
        let skip_sleep = state.audio_emergency_catch_up(state.paused, unlimited, rewinding, speed);
        let wake = state
            .scheduler
            .wake(state.paused, unlimited, skip_sleep, Instant::now());
        if !state.sleep_for_wake(wake, &cmd_rx) {
            break;
        }
        if !drain_commands(&mut state, &cmd_rx) {
            break;
        }
    }

    if let Err(e) = state.flush_save() {
        let _ = e;
    }
}

fn env_flag(name: &str) -> bool {
    match std::env::var(name) {
        Ok(v) => matches!(
            v.as_str(),
            "1" | "true" | "TRUE" | "yes" | "YES" | "on" | "ON"
        ),
        Err(_) => false,
    }
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

fn rom_title(cart: &graycart::Cartridge, path: &Path) -> String {
    let t = cart.header.title.trim();
    if t.is_empty() {
        path.file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("ROM")
            .to_string()
    } else {
        t.to_string()
    }
}

struct EmuState {
    config: RuntimeConfig,
    machine: Option<Machine>,
    audio: Option<AudioOut>,
    audio_init_error: Option<String>,
    scheduler: HostScheduler,
    fps: FpsCounter,
    telemetry: HostTelemetry,
    host_down: HashSet<HostCommand>,
    rewind_ring: Option<RewindRing>,
    rewind_setting: bool,
    paused: bool,
    ff_toggle: bool,
    ff_speed: SpeedPreset,
    audio_gain: f32,
    button_mask: u8,
    pending_slot: Option<PendingSlotOp>,
    frame_advance_pending: bool,
    frame_advance_flash_until: Option<Instant>,
    prev_speed: SpeedPreset,
    prev_rewinding: bool,
    tick_profiling: bool,
    debug_publish: bool,
    last_debug_publish: Instant,
    diag_window_start: Instant,
    diag_cycles: u64,
    diag_frames: u64,
    runtime_tcycles_per_sec: f64,
    runtime_emu_fps: f64,
    last_runtime_diag_log: Instant,
    missed_frames: u64,
    last_host_fps: f64,
    peak_l: f32,
    peak_r: f32,
    ready: bool,
    quit: bool,
    status_toast: Option<String>,
    exit_error: Option<String>,
}

impl EmuState {
    fn new(config: RuntimeConfig) -> Self {
        let ff_speed = config.ff_speed;
        let rewind_setting = config.rewind_enabled;
        let audio_gain = config.audio_gain;
        let pacer_enabled = config.pacer_enabled;
        Self {
            config,
            machine: None,
            audio: None,
            audio_init_error: None,
            scheduler: HostScheduler::new(pacer_enabled),
            fps: FpsCounter::new(),
            telemetry: HostTelemetry::new(),
            host_down: HashSet::new(),
            rewind_ring: None,
            rewind_setting,
            paused: false,
            ff_toggle: false,
            ff_speed,
            audio_gain,
            button_mask: 0,
            pending_slot: None,
            frame_advance_pending: false,
            frame_advance_flash_until: None,
            prev_speed: SpeedPreset::X1,
            prev_rewinding: false,
            tick_profiling: false,
            debug_publish: false,
            last_debug_publish: Instant::now(),
            diag_window_start: Instant::now(),
            diag_cycles: 0,
            diag_frames: 0,
            runtime_tcycles_per_sec: 0.0,
            runtime_emu_fps: 0.0,
            last_runtime_diag_log: Instant::now(),
            missed_frames: 0,
            last_host_fps: 0.0,
            peak_l: 0.0,
            peak_r: 0.0,
            ready: false,
            quit: false,
            status_toast: None,
            exit_error: None,
        }
    }

    fn handle_command(&mut self, cmd: EmuCommand) {
        match cmd {
            EmuCommand::Quit => self.quit = true,
            EmuCommand::Start => {
                match AudioOut::open_pref(&self.config.audio_output) {
                    Ok(a) => {
                        if self.config.verbosity != Verbosity::Quiet {
                            println!("{}", a.startup_line());
                        }
                        self.audio = Some(a);
                        self.audio_init_error = None;
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        self.set_status_toast(format!("AUDIO OFFLINE: {}", e.message));
                        self.audio = None;
                        self.audio_init_error = Some(e.message);
                    }
                }
                if let Some(launch) = self.config.initial_rom.take() {
                    let title = rom_title(&launch.cart, &launch.path);
                    if self
                        .install_rom(launch.path, launch.save_path, title, launch.cart)
                        .is_ok()
                    {
                        let _ = self.reset_machine(self.config.boot_mode);
                    }
                }
                self.ready = true;
            }
            EmuCommand::LoadRom {
                path,
                save_path,
                title,
                cart,
            } => {
                let _ = self.flush_save();
                if self.install_rom(path, save_path, title, *cart).is_ok() {
                    let _ = self.reset_machine(self.config.boot_mode);
                }
            }
            EmuCommand::Reset { boot_mode } => {
                let _ = self.reset_machine(boot_mode);
            }
            EmuCommand::SetButtons(mask) => {
                self.apply_button_mask(mask);
            }
            EmuCommand::HostPress(cmd) => {
                if cmd == HostCommand::Rewind || cmd == HostCommand::FastForwardHold {
                    self.host_down.insert(cmd);
                }
            }
            EmuCommand::HostRelease(cmd) => {
                self.host_down.remove(&cmd);
                if cmd == HostCommand::Rewind
                    && let Some(ring) = self.rewind_ring.as_mut()
                {
                    ring.end_scrub();
                }
            }
            EmuCommand::FocusLost {
                pause_when_unfocused,
            } => {
                let mut ff_hold = self.host_down.contains(&HostCommand::FastForwardHold);
                let mut rewind_held = self.host_down.contains(&HostCommand::Rewind);
                if super::super::playback::clear_transient_holds(&mut ff_hold, &mut rewind_held) {
                    self.host_down.remove(&HostCommand::FastForwardHold);
                    self.host_down.remove(&HostCommand::Rewind);
                    if let Some(ring) = self.rewind_ring.as_mut() {
                        ring.end_scrub();
                    }
                }
                if pause_when_unfocused {
                    self.paused = true;
                }
            }
            EmuCommand::SetPaused(p) => {
                self.paused = p;
                if p {
                    self.host_down.remove(&HostCommand::FastForwardHold);
                }
            }
            EmuCommand::SetFfSpeed(s) => self.ff_speed = s,
            EmuCommand::SetFfToggle(t) => self.ff_toggle = t,
            EmuCommand::SetRewindEnabled(e) => {
                self.rewind_setting = e;
                if !e {
                    self.rewind_ring = None;
                }
            }
            EmuCommand::SetAudioGain(g) => self.audio_gain = g,
            EmuCommand::SetTickProfiling(on) => {
                self.tick_profiling = on;
                if let Some(m) = self.machine.as_mut() {
                    m.bus.set_tick_profiling(on);
                }
            }
            EmuCommand::SetDebugPublish(on) => {
                self.debug_publish = on;
            }
            EmuCommand::SlotSave(slot) => {
                self.pending_slot = Some(PendingSlotOp::Save(slot));
            }
            EmuCommand::SlotLoad(slot) => {
                self.pending_slot = Some(PendingSlotOp::Load(slot));
            }
            EmuCommand::FrameAdvance => {
                if self.paused {
                    self.frame_advance_pending = true;
                }
            }
            EmuCommand::FlushSave { reply } => {
                let result = self.flush_save();
                let _ = reply.send(result);
            }
            EmuCommand::SetHardwarePref(pref) => {
                self.config.hardware_pref = pref;
                if let Some(machine) = self.machine.as_ref() {
                    let path = machine.path.clone();
                    let save_path = machine.save_path.clone();
                    let title = machine.title.clone();
                    match graycart::Cartridge::load(&path) {
                        Ok(cart) => {
                            let _ = self.flush_save();
                            if self.install_rom(path, save_path, title, cart).is_ok() {
                                let _ = self.reset_machine(self.config.boot_mode);
                            }
                        }
                        Err(e) => self.set_status_toast(e.to_string()),
                    }
                }
            }
            EmuCommand::SetAudioOutput(pref) => {
                self.config.audio_output = pref.clone();
                match AudioOut::open_pref(&pref) {
                    Ok(a) => {
                        if self.config.verbosity != Verbosity::Quiet {
                            println!("{}", a.startup_line());
                        }
                        self.audio = Some(a);
                        self.audio_init_error = None;
                        self.bind_host_audio();
                    }
                    Err(e) => {
                        eprintln!("{e}");
                        self.set_status_toast(format!("AUDIO OFFLINE: {}", e.message));
                        self.audio = None;
                        self.audio_init_error = Some(e.message);
                    }
                }
            }
        }
    }

    fn install_rom(
        &mut self,
        path: PathBuf,
        save_path: PathBuf,
        title: String,
        cart: graycart::Cartridge,
    ) -> Result<(), String> {
        let mut bus = match bus_from_cartridge(cart, self.config.hardware_pref) {
            Ok(bus) => bus,
            Err(e) => {
                let msg = e.to_string();
                self.set_status_toast(msg.clone());
                return Err(msg);
            }
        };
        if let Some(ref a) = self.audio {
            bind_bus_host_rate(&mut bus, Some(a.sample_rate));
        }
        self.machine = Some(Machine {
            path,
            save_path,
            title,
            cpu: Cpu::new(),
            bus,
            session: ExecSession::new(),
        });
        self.reset_rewind_ring();
        Ok(())
    }

    fn flush_save(&mut self) -> Result<(), String> {
        let Some(machine) = self.machine.as_mut() else {
            return Ok(());
        };
        match flush_save(&machine.save_path, &mut machine.bus.cartridge) {
            Ok(true) => {
                if self.config.verbosity != Verbosity::Quiet {
                    println!("save: wrote {}", machine.save_path.display());
                }
                Ok(())
            }
            Ok(false) => Ok(()),
            Err(e) => Err(format!(
                "failed to write save {}: {e}",
                machine.save_path.display()
            )),
        }
    }

    fn reset_machine(&mut self, boot_mode: BootMode) -> Result<(), String> {
        if self.machine.is_none() {
            return Ok(());
        }
        self.pending_slot = None;

        let mut mode = boot_mode;
        let machine = self.machine.as_mut().unwrap();
        let cgb_silicon = matches!(machine.bus.hardware_model(), HardwareModel::Cgb { .. });

        enum Overlay {
            Dmg(Box<[u8; 256]>),
            Cgb(Box<[u8; CGB_BOOT_ROM_SIZE]>),
            None,
        }

        let overlay = if mode == BootMode::BootRom {
            if cgb_silicon {
                match exe_dir().and_then(|dir| load_cached_cgb_boot_rom(&dir)) {
                    Some(bytes) => Overlay::Cgb(bytes),
                    None => {
                        mode = BootMode::Fast;
                        Overlay::None
                    }
                }
            } else {
                match exe_dir().and_then(|dir| load_cached_boot_rom(&dir)) {
                    Some(bytes) => Overlay::Dmg(Box::new(bytes)),
                    None => {
                        mode = BootMode::Fast;
                        Overlay::None
                    }
                }
            }
        } else {
            Overlay::None
        };

        machine.cpu = Cpu::new();
        machine.session = ExecSession::new();
        machine.bus.power_on_keep_battery();
        match &overlay {
            Overlay::Dmg(bytes) => machine.bus.enable_boot_rom(bytes),
            Overlay::Cgb(bytes) => machine.bus.enable_cgb_boot_rom(bytes),
            Overlay::None => {}
        }
        prepare_boot(mode, &mut machine.cpu, &mut machine.bus);
        bind_bus_host_rate(&mut machine.bus, self.audio.as_ref().map(|a| a.sample_rate));
        let _ = machine.bus.apu.take_samples();
        machine.bus.apu.stats.reset_window();
        self.paused = false;
        self.ff_toggle = false;
        self.host_down.remove(&HostCommand::FastForwardHold);
        self.sync_buttons_to_joypad();
        self.reset_rewind_ring();
        Ok(())
    }

    fn apply_button_mask(&mut self, mask: u8) {
        if mask == self.button_mask {
            return;
        }
        let Some(machine) = self.machine.as_mut() else {
            self.button_mask = mask;
            return;
        };
        for i in 0..8u8 {
            let bit = 1u8 << i;
            let was = self.button_mask & bit != 0;
            let now = mask & bit != 0;
            if was == now {
                continue;
            }
            let button = graycart::GameBoyButton::ALL[i as usize];
            if now {
                machine.bus.press_button(button);
            } else {
                machine.bus.release_button(button);
            }
        }
        self.button_mask = mask;
    }

    fn sync_buttons_to_joypad(&mut self) {
        let mask = self.button_mask;
        self.button_mask = 0;
        self.apply_button_mask(mask);
    }

    fn bind_host_audio(&mut self) {
        let rate = self.audio.as_ref().map(|a| a.sample_rate);
        if let Some(machine) = self.machine.as_mut() {
            bind_bus_host_rate(&mut machine.bus, rate);
        }
    }

    fn after_machine_restore(&mut self) {
        if let Some(machine) = self.machine.as_mut() {
            machine.bus.disable_boot_rom();
        }
        if let Some(a) = &self.audio {
            a.after_restore();
        }
        self.bind_host_audio();
        if let Some(machine) = self.machine.as_mut() {
            let _ = machine.bus.apu.take_samples();
        }
        self.sync_buttons_to_joypad();
    }

    fn sync_rewind_ring(&mut self) {
        if !self.rewind_setting || self.machine.is_none() {
            self.rewind_ring = None;
            return;
        }
        if self.rewind_ring.is_none() {
            let machine = self.machine.as_ref().unwrap();
            if !machine.bus.boot_rom_active() {
                let template = capture(&machine.cpu, &machine.bus);
                self.rewind_ring = Some(RewindRing::new(&template));
            }
        }
    }

    fn reset_rewind_ring(&mut self) {
        if let Some(ring) = self.rewind_ring.as_mut() {
            ring.clear();
        }
        self.rewind_ring = None;
        self.sync_rewind_ring();
    }

    fn apply_rewind_scrub(&mut self) {
        let Some(ring) = self.rewind_ring.as_mut() else {
            return;
        };
        let Some(machine) = self.machine.as_mut() else {
            return;
        };
        ring.begin_scrub();
        if let Some(state) = ring.scrub_back() {
            restore(state, &mut machine.cpu, &mut machine.bus);
            self.after_machine_restore();
        }
    }

    fn apply_pending_slot_op(&mut self) {
        let op = match self.pending_slot.take() {
            Some(op) => op,
            None => return,
        };
        let Some(machine) = self.machine.as_mut() else {
            return;
        };
        match op {
            PendingSlotOp::Save(slot) => {
                if machine.bus.boot_rom_active() {
                    self.pending_slot = Some(PendingSlotOp::Save(slot));
                    return;
                }
                let state = capture(&machine.cpu, &machine.bus);
                let rom = machine.bus.cartridge.rom_bytes();
                match save_slot(rom, &machine.path, &machine.title, slot, &state) {
                    Ok(()) => self.set_status_toast(format!("Saved slot {slot}")),
                    Err(e) => self.set_status_toast(format!("Save failed: {e}")),
                }
            }
            PendingSlotOp::Load(slot) => {
                let rom = machine.bus.cartridge.rom_bytes();
                match load_slot(rom, &machine.path, slot) {
                    Ok((state, _)) => {
                        restore(&state, &mut machine.cpu, &mut machine.bus);
                        self.after_machine_restore();
                        self.reset_rewind_ring();
                        self.set_status_toast(format!("Loaded slot {slot}"));
                    }
                    Err(e) => self.set_status_toast(format!("Load rejected: {e}")),
                }
            }
        }
    }

    fn set_status_toast(&mut self, message: String) {
        if self.config.verbosity != Verbosity::Quiet {
            eprintln!("status: {message}");
        }
        self.status_toast = Some(message);
    }

    fn take_status_toast(&mut self) -> Option<String> {
        self.status_toast.take()
    }

    fn emulate_until_frame_or_budget(&mut self) -> Option<StepPulse> {
        let pulse = {
            let machine = self.machine.as_mut()?;
            if self.tick_profiling {
                machine.bus.set_tick_profiling(true);
                machine.bus.tick_profile.reset();
            }

            let mut fault = None;
            let pulse = run_until_frame_or_budget(
                &mut || {
                    match machine.session.step(&mut machine.cpu, &mut machine.bus) {
                        Ok(_) => {}
                        Err(e) => {
                            fault = Some(e);
                            return true;
                        }
                    }
                    if machine.bus.ppu.take_frame_ready() {
                        machine.session.note_frame();
                        return true;
                    }
                    false
                },
                FRAME_STEP_BUDGET,
            );

            if let Some(e) = fault {
                let report = machine
                    .session
                    .fault_from_step(e, &machine.cpu, &machine.bus);
                eprintln!("{report}");
                self.exit_error = Some(report.to_string());
                self.quit = true;
                return None;
            }
            pulse
        };

        if pulse == StepPulse::FrameReady {
            self.note_emulated_frame();
        }
        Some(pulse)
    }

    fn note_emulated_frame(&mut self) {
        self.diag_cycles = self.diag_cycles.saturating_add(T_CYCLES_PER_FRAME);
        self.diag_frames = self.diag_frames.saturating_add(1);
        self.refresh_runtime_rates();
    }

    fn refresh_runtime_rates(&mut self) {
        let elapsed = self.diag_window_start.elapsed();
        if elapsed >= Duration::from_millis(250) {
            let secs = elapsed.as_secs_f64().max(1e-9);
            self.runtime_tcycles_per_sec = self.diag_cycles as f64 / secs;
            self.runtime_emu_fps = self.diag_frames as f64 / secs;
            self.diag_window_start = Instant::now();
            self.diag_cycles = 0;
            self.diag_frames = 0;
        }
    }

    fn maybe_publish_debug(&mut self, latest_debug: &LatestDebug) {
        if !self.debug_publish {
            return;
        }
        if self.last_debug_publish.elapsed() < DEBUG_PUBLISH_INTERVAL {
            return;
        }
        self.last_debug_publish = Instant::now();
        if let Some(m) = self.machine.as_ref() {
            latest_debug.publish(MachineDebug::capture(&m.cpu, &m.bus, &m.session));
        }
    }

    fn maybe_log_runtime_diag(&mut self, latest: &LatestFrame) {
        if self.last_runtime_diag_log.elapsed() < RUNTIME_DIAG_INTERVAL {
            return;
        }
        self.last_runtime_diag_log = Instant::now();
        let audio_missing = self
            .audio
            .as_ref()
            .map(|a| a.stats().missing_samples)
            .unwrap_or(0);
        let audio_underruns = self
            .audio
            .as_ref()
            .map(|a| a.stats().underrun_events)
            .unwrap_or(0);
        eprintln!(
            "runtime: {:.3} MHz  frames/s={:.1}  frame_replaced={}  audio_underruns={}  audio_missing={}",
            self.runtime_tcycles_per_sec / 1_000_000.0,
            self.runtime_emu_fps,
            latest.replaced_count(),
            audio_underruns,
            audio_missing,
        );
    }

    fn push_rewind_snapshot_if_due(&mut self, frame_done: bool) {
        let boot_active = self
            .machine
            .as_ref()
            .is_some_and(|m| m.bus.boot_rom_active());
        if should_allow_snapshot(boot_active, frame_done)
            && self.rewind_setting
            && !self.host_down.contains(&HostCommand::Rewind)
            && let (Some(ring), Some(machine)) = (self.rewind_ring.as_mut(), self.machine.as_mut())
        {
            let state = capture(&machine.cpu, &machine.bus);
            ring.push(state);
        }
    }

    fn submit_frame_audio(&mut self, profile_detail: bool, submit: bool) {
        if self.machine.is_none() {
            return;
        }
        let machine = self.machine.as_mut().unwrap();
        let samples = machine.bus.apu.take_samples();
        if profile_detail {
            let mut peak_l = 0.0f32;
            let mut peak_r = 0.0f32;
            for s in &samples {
                peak_l = peak_l.max(s.left.abs());
                peak_r = peak_r.max(s.right.abs());
            }
            self.peak_l = peak_l;
            self.peak_r = peak_r;
        } else {
            let _ = machine.bus.take_tick_profile();
        }
        if submit && let Some(a) = &self.audio {
            a.submit_gain(&samples, self.audio_gain);
        }
    }

    fn audio_emergency_catch_up(
        &self,
        paused: bool,
        unlimited: bool,
        rewinding: bool,
        speed: SpeedPreset,
    ) -> bool {
        !paused
            && !unlimited
            && !rewinding
            && speed == SpeedPreset::X1
            && self.config.pacer_enabled
            && self
                .audio
                .as_ref()
                .is_some_and(|a| a.needs_emergency_catch_up())
    }

    fn run_tick(&mut self) -> FramePacket {
        let frame_start = Instant::now();
        let now = frame_start;

        let ff_hold = self.host_down.contains(&HostCommand::FastForwardHold);
        let rewinding = self.rewind_setting && self.host_down.contains(&HostCommand::Rewind);
        let speed = if self.config.unthrottled {
            SpeedPreset::Unlimited
        } else {
            effective_speed(
                self.paused,
                rewinding,
                ff_hold,
                self.ff_toggle,
                self.ff_speed,
            )
        };
        let unlimited = self.config.unthrottled || speed == SpeedPreset::Unlimited;

        apply_audio_transition_if_needed(
            self.audio.as_ref(),
            self.prev_speed,
            self.prev_rewinding,
            speed,
            rewinding,
        );
        self.prev_speed = speed;
        self.prev_rewinding = rewinding;

        let mut frame_done = false;
        self.sync_rewind_ring();

        let profile_detail = self.tick_profiling;
        let submit_audio = should_submit_audio(speed, rewinding, self.paused);

        if self.machine.is_some() {
            if self.paused {
                if self.frame_advance_pending {
                    self.frame_advance_pending = false;
                    if let Some(pulse) = self.emulate_until_frame_or_budget() {
                        frame_done = pulse == StepPulse::FrameReady;
                        if frame_done {
                            self.frame_advance_flash_until = Some(now + FRAME_ADVANCE_FLASH);
                            self.push_rewind_snapshot_if_due(true);
                        }
                    }
                    self.submit_frame_audio(profile_detail, submit_audio);
                }
            } else if rewinding {
                self.apply_rewind_scrub();
            } else if unlimited || !self.config.pacer_enabled {
                if let Some(pulse) = self.emulate_until_frame_or_budget() {
                    frame_done = pulse == StepPulse::FrameReady;
                    self.push_rewind_snapshot_if_due(frame_done);
                }
                self.submit_frame_audio(profile_detail, submit_audio);
            } else {
                let cap = catch_up_cap(speed);
                let mut frames = self.scheduler.frames_to_catch_up(speed, now, cap);
                if frames == 0
                    && self.audio_emergency_catch_up(self.paused, unlimited, rewinding, speed)
                {
                    frames = 1;
                }
                if frames > 0 {
                    for _ in 0..frames {
                        if let Some(pulse) = self.emulate_until_frame_or_budget() {
                            if pulse == StepPulse::FrameReady {
                                frame_done = true;
                                self.push_rewind_snapshot_if_due(true);
                                self.scheduler.on_frame_presented(speed, Instant::now());
                            } else {
                                break;
                            }
                        } else {
                            break;
                        }
                    }
                    self.submit_frame_audio(profile_detail, submit_audio);
                }
            }
        }

        let boot_active = self
            .machine
            .as_ref()
            .is_some_and(|m| m.bus.boot_rom_active());
        if self
            .pending_slot
            .is_some_and(|op| pending_slot_ready(op, boot_active, self.paused, frame_done))
        {
            self.apply_pending_slot_op();
        }

        let overlay_speed = if !self.paused && !rewinding && speed != SpeedPreset::X1 {
            Some(speed)
        } else {
            None
        };

        let framebuffer = self.machine.as_ref().map(|m| m.bus.ppu.framebuffer.clone());
        let title = self
            .machine
            .as_ref()
            .map(|m| m.title.clone())
            .unwrap_or_default();

        let frame_time = frame_start.elapsed();
        if self.machine.is_some() && !self.paused {
            self.telemetry.note_frame(frame_time, Duration::ZERO);
            if frame_time > FRAME_DURATION + FRAME_DURATION / 2 {
                self.missed_frames = self.missed_frames.saturating_add(1);
            }
        }

        if let Some(measured) = self.fps.tick() {
            self.last_host_fps = measured;
        }

        let (
            audio_queued,
            audio_target,
            audio_missing,
            audio_underrun_events,
            audio_dropped,
            audio_resample_step,
            audio_sample_rate,
            audio_produced,
            audio_consumed,
            audio_callbacks,
            audio_elapsed_secs,
            audio_device,
            apu_ch1_debug,
        ) = if let Some(a) = &self.audio {
            let stats = a.stats();
            let apu_line = if self.config.verbosity == Verbosity::Verbose {
                self.machine.as_mut().map(|m| {
                    let line = m.bus.apu.debug_ch1();
                    m.bus.apu.stats.reset_window();
                    line.to_string()
                })
            } else {
                None
            };
            (
                Some(a.queued_frames()),
                Some(a.target_frames),
                Some(stats.missing_samples),
                Some(stats.underrun_events),
                Some(stats.dropped),
                Some(a.last_resample_step()),
                Some(a.sample_rate),
                Some(stats.produced),
                Some(stats.consumed),
                Some(a.callbacks()),
                Some(a.elapsed_secs()),
                Some(a.device_name.clone()),
                apu_line,
            )
        } else {
            (
                None, None, None, None, None, None, None, None, None, None, None, None, None,
            )
        };

        FramePacket {
            framebuffer,
            rom_loaded: self.machine.is_some(),
            title,
            paused: self.paused,
            ff_toggle: self.ff_toggle,
            rewinding,
            overlay_speed,
            frame_advance_flash: self
                .frame_advance_flash_until
                .is_some_and(|until| Instant::now() < until),
            status_toast: self.take_status_toast(),
            fault: self.exit_error.clone(),
            last_frame_time: frame_time,
            host_fps: self.last_host_fps,
            runtime_tcycles_per_sec: self.runtime_tcycles_per_sec,
            runtime_emu_fps: self.runtime_emu_fps,
            frame_publish_replaced: 0, // filled by UI from LatestFrame if needed; keep field for packet
            peak_l: self.peak_l,
            peak_r: self.peak_r,
            missed_frames: self.missed_frames,
            audio_queued,
            audio_target,
            audio_missing,
            audio_underrun_events,
            audio_dropped,
            audio_resample_step,
            audio_sample_rate,
            audio_produced,
            audio_consumed,
            audio_callbacks,
            audio_elapsed_secs,
            audio_device,
            audio_init_error: self.audio_init_error.clone(),
            apu_ch1_debug,
        }
    }

    /// Pace on this thread only: `thread::sleep` + non-blocking command drain.
    /// Returns `false` if the command channel disconnected.
    fn sleep_for_wake(&mut self, wake: HostWake, cmd_rx: &Receiver<EmuCommand>) -> bool {
        match wake {
            HostWake::WaitUntil(deadline) => wait_until(deadline, cmd_rx, self),
            HostWake::Poll => {
                thread::yield_now();
                true
            }
            HostWake::Wait => {
                let end = Instant::now() + Duration::from_millis(16);
                while Instant::now() < end {
                    if !drain_commands(self, cmd_rx) {
                        return false;
                    }
                    if self.quit {
                        return true;
                    }
                    let rem = end.saturating_duration_since(Instant::now());
                    if rem.is_zero() {
                        break;
                    }
                    thread::sleep(rem.min(Duration::from_millis(1)));
                }
                true
            }
        }
    }
}

fn wait_until(deadline: Instant, cmd_rx: &Receiver<EmuCommand>, state: &mut EmuState) -> bool {
    while Instant::now() < deadline {
        if !drain_commands(state, cmd_rx) {
            return false;
        }
        if state.quit {
            return true;
        }
        let rem = deadline.saturating_duration_since(Instant::now());
        if rem.is_zero() {
            break;
        }
        thread::sleep(rem.min(Duration::from_millis(1)));
    }
    true
}

/// Drain pending commands with `try_recv` only. Returns `false` on disconnect.
fn drain_commands(state: &mut EmuState, cmd_rx: &Receiver<EmuCommand>) -> bool {
    loop {
        match cmd_rx.try_recv() {
            Ok(cmd) => {
                state.handle_command(cmd);
                if state.quit {
                    return true;
                }
            }
            Err(std::sync::mpsc::TryRecvError::Empty) => return true,
            Err(std::sync::mpsc::TryRecvError::Disconnected) => return false,
        }
    }
}
