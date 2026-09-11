/// Condition codes for conditional jumps/calls/returns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cond {
    Nz,
    Z,
    Nc,
    C,
}

impl Cond {
    pub fn mnemonic(self) -> &'static str {
        match self {
            Self::Nz => "NZ",
            Self::Z => "Z",
            Self::Nc => "NC",
            Self::C => "C",
        }
    }
}

/// 8-bit CPU register.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reg8 {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
}

impl Reg8 {
    pub fn mnemonic(self) -> &'static str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
            Self::H => "H",
            Self::L => "L",
        }
    }
}

/// 8-bit operand (register, immediate, or `(HL)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Operand8 {
    Reg(Reg8),
    Imm(u8),
    MemHl,
}

impl Operand8 {
    pub fn mnemonic(self) -> String {
        match self {
            Self::Reg(r) => r.mnemonic().to_string(),
            Self::Imm(n) => format!("${n:02X}"),
            Self::MemHl => "(HL)".to_string(),
        }
    }
}

/// 16-bit register / register pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reg16 {
    Af,
    Bc,
    De,
    Hl,
    Sp,
}

impl Reg16 {
    pub fn mnemonic(self) -> &'static str {
        match self {
            Self::Af => "AF",
            Self::Bc => "BC",
            Self::De => "DE",
            Self::Hl => "HL",
            Self::Sp => "SP",
        }
    }
}

/// A decoded LR35902 instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Instruction {
    Nop,
    /// Stop instruction fetch until an interrupt wakes the CPU.
    Halt,
    /// `STOP` — low-power stop (2 bytes). Joypad wake deferred (Phase 6).
    Stop,

    /// Absolute jump. `cond == None` means unconditional.
    Jp {
        cond: Option<Cond>,
        target: u16,
    },
    /// `JP (HL)` — jump to the address in HL (not memory at [HL]).
    JpHl,
    /// Relative jump (signed offset from instruction end). `cond == None` = unconditional.
    Jr {
        cond: Option<Cond>,
        offset: i8,
    },
    /// Call. `cond == None` means unconditional.
    Call {
        cond: Option<Cond>,
        target: u16,
    },
    /// `RST n` — push PC and jump to page-0 vector (`$00`, `$08`, … `$38`).
    Rst(u8),
    /// Return. `cond == None` means unconditional.
    Ret {
        cond: Option<Cond>,
    },
    /// Return from interrupt: pop PC and enable IME immediately.
    Reti,

    /// Disable interrupts (`IME = 0`).
    Di,
    /// Enable interrupts (`IME = 1`, after the following instruction on hardware;
    /// we set immediately for now).
    Ei,
    /// `SCF` — set carry; Z unchanged, N=0, H=0.
    Scf,
    /// `CCF` — complement carry; Z unchanged, N=0, H=0.
    Ccf,
    /// `DAA` — decimal adjust A after BCD add/sub.
    Daa,

    Push(Reg16),
    Pop(Reg16),
    /// `LD rr, d16`
    Ld16Imm {
        dst: Reg16,
        value: u16,
    },
    /// `LD (a16), SP`
    LdAbsSp(u16),
    /// `INC rr` (16-bit; does not affect flags)
    Inc16(Reg16),
    /// `DEC rr` (16-bit; does not affect flags)
    Dec16(Reg16),
    /// `ADD HL, rr`
    AddHl(Reg16),
    /// `ADD SP, e8`
    AddSpE(i8),
    /// `LD HL, SP+e8`
    LdHlSpE(i8),
    /// `LD SP, HL`
    LdSpHl,

    Ld8 {
        dst: Operand8,
        src: Operand8,
    },
    /// `LD (a16), A`
    LdAbsA(u16),
    /// `LD A, (a16)`
    LdAAbs(u16),
    /// `LD (BC), A`
    LdMemBcA,
    /// `LD A, (BC)`
    LdAMemBc,
    /// `LD (DE), A`
    LdMemDeA,
    /// `LD A, (DE)`
    LdAMemDe,
    /// `LDH (a8), A`
    LdhAbsA(u8),
    /// `LDH A, (a8)`
    LdhAAbs(u8),
    /// `LD (C), A` — write A to `$FF00+C`
    LdIoCA,
    /// `LD A, (C)` — read A from `$FF00+C`
    LdAIoC,
    /// `LD (HL+), A`
    LdiMemHlA,
    /// `LD A, (HL+)`
    LdiAMemHl,
    /// `LD (HL-), A`
    LddMemHlA,
    /// `LD A, (HL-)`
    LddAMemHl,
    /// `INC r` / `INC (HL)`
    Inc8(Operand8),
    /// `DEC r` / `DEC (HL)`
    Dec8(Operand8),
    /// `ADD A, n`
    Add(Operand8),
    /// `ADC A, n`
    Adc(Operand8),
    /// `SUB n`
    Sub(Operand8),
    /// `SBC A, n`
    Sbc(Operand8),
    Cp(Operand8),
    And(Operand8),
    Xor(Operand8),
    Or(Operand8),
    Cpl,
    /// Accumulator rotates (non-CB): Z always cleared.
    Rlca,
    Rrca,
    Rla,
    Rra,
    /// `CB`-prefixed RLC r / (HL)
    Rlc(Operand8),
    /// `CB`-prefixed RRC r / (HL)
    Rrc(Operand8),
    /// `CB`-prefixed RL r / (HL)
    Rl(Operand8),
    /// `CB`-prefixed RR r / (HL)
    Rr(Operand8),
    /// `CB`-prefixed SLA r / (HL)
    Sla(Operand8),
    /// `CB`-prefixed SRA r / (HL)
    Sra(Operand8),
    /// `CB`-prefixed SRL r / (HL)
    Srl(Operand8),
    /// `CB`-prefixed SWAP r / (HL)
    Swap(Operand8),
    /// `CB`-prefixed BIT b,r / BIT b,(HL)
    Bit {
        bit: u8,
        op: Operand8,
    },
    /// `CB`-prefixed RES b,r / RES b,(HL)
    Res {
        bit: u8,
        op: Operand8,
    },
    /// `CB`-prefixed SET b,r / SET b,(HL)
    Set {
        bit: u8,
        op: Operand8,
    },
}

impl Instruction {
    /// Mnemonic string for disassembly. `pc` resolves relative branch targets.
    pub fn mnemonic(self, pc: u16) -> String {
        match self {
            Self::Nop => "NOP".to_string(),
            Self::Halt => "HALT".to_string(),
            Self::Stop => "STOP".to_string(),
            Self::Jp { cond, target } => fmt_control("JP", cond, format!("${target:04X}")),
            Self::JpHl => "JP (HL)".to_string(),
            Self::Jr { cond, offset } => {
                fmt_control("JR", cond, format!("${:04X}", jr_target(pc, offset)))
            }
            Self::Call { cond, target } => fmt_control("CALL", cond, format!("${target:04X}")),
            Self::Rst(vec) => format!("RST ${vec:02X}"),
            Self::Ret { cond: None } => "RET".to_string(),
            Self::Ret { cond: Some(c) } => format!("RET {}", c.mnemonic()),
            Self::Reti => "RETI".to_string(),
            Self::Di => "DI".to_string(),
            Self::Ei => "EI".to_string(),
            Self::Scf => "SCF".to_string(),
            Self::Ccf => "CCF".to_string(),
            Self::Daa => "DAA".to_string(),
            Self::Push(rr) => format!("PUSH {}", rr.mnemonic()),
            Self::Pop(rr) => format!("POP {}", rr.mnemonic()),
            Self::Ld16Imm { dst, value } => {
                format!("LD {}, ${value:04X}", dst.mnemonic())
            }
            Self::LdAbsSp(addr) => format!("LD (${addr:04X}), SP"),
            Self::Inc16(rr) => format!("INC {}", rr.mnemonic()),
            Self::Dec16(rr) => format!("DEC {}", rr.mnemonic()),
            Self::AddHl(rr) => format!("ADD HL, {}", rr.mnemonic()),
            Self::AddSpE(e) => format!("ADD SP, {e:+}"),
            Self::LdHlSpE(e) => format!("LD HL, SP{e:+}"),
            Self::LdSpHl => "LD SP, HL".to_string(),
            Self::Ld8 { dst, src } => {
                format!("LD {}, {}", dst.mnemonic(), src.mnemonic())
            }
            Self::LdAbsA(addr) => format!("LD (${addr:04X}), A"),
            Self::LdAAbs(addr) => format!("LD A, (${addr:04X})"),
            Self::LdMemBcA => "LD (BC), A".to_string(),
            Self::LdAMemBc => "LD A, (BC)".to_string(),
            Self::LdMemDeA => "LD (DE), A".to_string(),
            Self::LdAMemDe => "LD A, (DE)".to_string(),
            Self::LdhAbsA(n) => format!("LDH (${n:02X}), A"),
            Self::LdhAAbs(n) => format!("LDH A, (${n:02X})"),
            Self::LdIoCA => "LD (C), A".to_string(),
            Self::LdAIoC => "LD A, (C)".to_string(),
            Self::LdiMemHlA => "LD (HL+), A".to_string(),
            Self::LdiAMemHl => "LD A, (HL+)".to_string(),
            Self::LddMemHlA => "LD (HL-), A".to_string(),
            Self::LddAMemHl => "LD A, (HL-)".to_string(),
            Self::Inc8(op) => format!("INC {}", op.mnemonic()),
            Self::Dec8(op) => format!("DEC {}", op.mnemonic()),
            Self::Add(op) => format!("ADD A, {}", op.mnemonic()),
            Self::Adc(op) => format!("ADC A, {}", op.mnemonic()),
            Self::Sub(op) => format!("SUB {}", op.mnemonic()),
            Self::Sbc(op) => format!("SBC A, {}", op.mnemonic()),
            Self::Cp(op) => format!("CP {}", op.mnemonic()),
            Self::And(op) => format!("AND {}", op.mnemonic()),
            Self::Xor(op) => format!("XOR {}", op.mnemonic()),
            Self::Or(op) => format!("OR {}", op.mnemonic()),
            Self::Cpl => "CPL".to_string(),
            Self::Rlca => "RLCA".to_string(),
            Self::Rrca => "RRCA".to_string(),
            Self::Rla => "RLA".to_string(),
            Self::Rra => "RRA".to_string(),
            Self::Rlc(op) => format!("RLC {}", op.mnemonic()),
            Self::Rrc(op) => format!("RRC {}", op.mnemonic()),
            Self::Rl(op) => format!("RL {}", op.mnemonic()),
            Self::Rr(op) => format!("RR {}", op.mnemonic()),
            Self::Sla(op) => format!("SLA {}", op.mnemonic()),
            Self::Sra(op) => format!("SRA {}", op.mnemonic()),
            Self::Srl(op) => format!("SRL {}", op.mnemonic()),
            Self::Swap(op) => format!("SWAP {}", op.mnemonic()),
            Self::Bit { bit, op } => format!("BIT {}, {}", bit, op.mnemonic()),
            Self::Res { bit, op } => format!("RES {}, {}", bit, op.mnemonic()),
            Self::Set { bit, op } => format!("SET {}, {}", bit, op.mnemonic()),
        }
    }
}

fn fmt_control(op: &str, cond: Option<Cond>, operand: String) -> String {
    match cond {
        None => format!("{op} {operand}"),
        Some(c) => format!("{op} {}, {operand}", c.mnemonic()),
    }
}

/// Absolute target of a relative jump at `pc` with signed `offset`.
///
/// JR is 2 bytes, so the base is `pc + 2`, then the signed displacement is added.
pub fn jr_target(pc: u16, offset: i8) -> u16 {
    ((pc as i32) + 2 + (offset as i32)) as u16
}
