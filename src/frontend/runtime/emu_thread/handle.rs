use super::super::super::audio::{AudioOut, bind_bus_host_rate};
use super::super::super::boot_rom::{load_cached_boot_rom, load_cached_cgb_boot_rom};
use super::super::super::input::HostCommand;
use super::super::super::launch::Verbosity;
use super::super::super::rewind::RewindRing;
use super::super::super::state_slots::{PendingSlotOp, load_slot, save_slot};
use super::super::commands::EmuCommand;
use super::machine::{Machine, exe_dir, rom_title};
use super::state::EmuState;
use graycart::hw::CGB_BOOT_ROM_SIZE;
use graycart::{
    BootMode, Cpu, ExecSession, HardwareModel, bus_from_cartridge, capture, flush_save,
    prepare_boot, restore,
};
use std::path::PathBuf;

impl EmuState {
    pub(super) fn handle_command(&mut self, cmd: EmuCommand) {
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
                if super::super::super::playback::clear_transient_holds(
                    &mut ff_hold,
                    &mut rewind_held,
                ) {
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

    pub(super) fn install_rom(
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

    pub(super) fn flush_save(&mut self) -> Result<(), String> {
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

    pub(super) fn reset_machine(&mut self, boot_mode: BootMode) -> Result<(), String> {
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

    pub(super) fn apply_button_mask(&mut self, mask: u8) {
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

    pub(super) fn sync_buttons_to_joypad(&mut self) {
        let mask = self.button_mask;
        self.button_mask = 0;
        self.apply_button_mask(mask);
    }

    pub(super) fn bind_host_audio(&mut self) {
        let rate = self.audio.as_ref().map(|a| a.sample_rate);
        if let Some(machine) = self.machine.as_mut() {
            bind_bus_host_rate(&mut machine.bus, rate);
        }
    }

    pub(super) fn after_machine_restore(&mut self) {
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

    pub(super) fn sync_rewind_ring(&mut self) {
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

    pub(super) fn reset_rewind_ring(&mut self) {
        if let Some(ring) = self.rewind_ring.as_mut() {
            ring.clear();
        }
        self.rewind_ring = None;
        self.sync_rewind_ring();
    }

    pub(super) fn apply_rewind_scrub(&mut self) {
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

    pub(super) fn apply_pending_slot_op(&mut self) {
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

    pub(super) fn set_status_toast(&mut self, message: String) {
        if self.config.verbosity != Verbosity::Quiet {
            eprintln!("status: {message}");
        }
        self.status_toast = Some(message);
    }

    pub(super) fn take_status_toast(&mut self) -> Option<String> {
        self.status_toast.take()
    }
}
