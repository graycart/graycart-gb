#!/usr/bin/env python3
"""Quick ridge helper: show bytes at PC for the active bank (and neighbors).

Useful right after a ridge stop line like:
  pokemon stop: ... PC=6100 bank=1C ...

Examples:
  python tools/ridge_bytes.py carts/pokemon_red-version__usa-eu.gb 6100 1C
  python tools/ridge_bytes.py carts/pokemon_red-version__usa-eu.gb 614D 1C -n 24 --neighbors 1
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))

from gb_rom import hex_bytes, load_rom, physical_offset, read_banked


def parse_int(text: str) -> int:
    text = text.strip().lower().replace("$", "").replace("0x", "")
    return int(text, 16)


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("rom", type=Path)
    ap.add_argument("pc", type=parse_int, help="PC from ridge output")
    ap.add_argument("bank", type=parse_int, help="switchable ROM bank from ridge output")
    ap.add_argument("-n", "--length", type=int, default=16)
    ap.add_argument(
        "--neighbors",
        type=int,
        default=0,
        help="also dump bank±N (default 0)",
    )
    args = ap.parse_args()

    rom = load_rom(args.rom)
    banks = [args.bank]
    for d in range(1, args.neighbors + 1):
        banks.extend([args.bank - d, args.bank + d])
    banks = sorted({b & 0xFF for b in banks if b >= 0})

    print(f"PC=${args.pc:04X}  primary bank=${args.bank:02X}")
    for bank in banks:
        mark = " <<<" if bank == args.bank else ""
        off = physical_offset(bank, args.pc) if args.pc >= 0x4000 else args.pc
        data = read_banked(rom, bank if args.pc >= 0x4000 else 0, args.pc, args.length)
        print(f"bank ${bank:02X}  off=${off:06X}: {hex_bytes(data)}{mark}")


if __name__ == "__main__":
    main()
