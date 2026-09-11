"""Shared helpers for inspecting Game Boy ROM images (flat + MBC banked)."""

from __future__ import annotations

from pathlib import Path

ROM_BANK_SIZE = 0x4000


def load_rom(path: str | Path) -> bytes:
    data = Path(path).read_bytes()
    if len(data) < 0x150:
        raise SystemExit(f"ROM too short: {path} ({len(data)} bytes)")
    return data


def physical_offset(bank: int, addr: int) -> int:
    """Map CPU address + ROM bank to a file offset.

    - `$0000–$3FFF` always uses bank 0 (fixed).
    - `$4000–$7FFF` uses `bank` (MBC switchable; bank 0 selects bank 1 on MBC3).
    """
    addr &= 0xFFFF
    if addr < 0x4000:
        return addr
    if addr < 0x8000:
        return bank * ROM_BANK_SIZE + (addr - 0x4000)
    raise ValueError(f"address ${addr:04X} is outside ROM window")


def read_banked(rom: bytes, bank: int, addr: int, length: int = 16) -> bytes:
    off = physical_offset(bank, addr)
    end = off + length
    if end > len(rom):
        raise SystemExit(
            f"out of range: bank=${bank:02X} addr=${addr:04X} "
            f"→ offset ${off:06X}+{length} (rom {len(rom)} bytes)"
        )
    return rom[off:end]


def hex_bytes(data: bytes) -> str:
    return " ".join(f"{b:02X}" for b in data)


def cart_type_byte(rom: bytes) -> int:
    return rom[0x0147]


def title(rom: bytes) -> str:
    raw = rom[0x0134:0x0143]
    return raw.split(b"\x00", 1)[0].decode("ascii", errors="replace")
