#!/usr/bin/env python3
"""Dump ROM bytes at a CPU address for one or more banks.

Examples:
  python tools/dump_bank.py carts/pokemon_red-version__usa-eu.gb 614D
  python tools/dump_bank.py carts/pokemon_red-version__usa-eu.gb 614D --bank 1C
  python tools/dump_bank.py carts/pokemon_red-version__usa-eu.gb 614D --banks 0-7
  python tools/dump_bank.py carts/pokemon_red-version__usa-eu.gb 6100 --bank 1C -n 32
"""

from __future__ import annotations

import argparse
import sys
from pathlib import Path

# Allow `python tools/dump_bank.py` without installing a package.
sys.path.insert(0, str(Path(__file__).resolve().parent))

from gb_rom import hex_bytes, load_rom, physical_offset, read_banked, title


def parse_int(text: str) -> int:
    text = text.strip().lower().replace("$", "").replace("0x", "")
    return int(text, 16)


def parse_banks(text: str) -> list[int]:
    text = text.strip().lower().replace("$", "").replace("0x", "")
    if "-" in text:
        a, b = text.split("-", 1)
        lo, hi = parse_int(a), parse_int(b)
        if hi < lo:
            lo, hi = hi, lo
        return list(range(lo, hi + 1))
    return [parse_int(p) for p in text.split(",") if p]


def main() -> None:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    ap.add_argument("rom", type=Path, help="path to .gb ROM")
    ap.add_argument("addr", type=parse_int, help="CPU address (hex), e.g. 614D or 0x6100")
    ap.add_argument("-n", "--length", type=int, default=16, help="bytes to dump (default 16)")
    g = ap.add_mutually_exclusive_group()
    g.add_argument("--bank", type=parse_int, help="single ROM bank (hex)")
    g.add_argument("--banks", type=str, help="bank list/range, e.g. 0-7 or 1C,1D")
    args = ap.parse_args()

    rom = load_rom(args.rom)
    print(f"{args.rom}  title={title(rom)!r}  size={len(rom)}  dump ${args.addr:04X}")

    if args.addr < 0x4000:
        data = read_banked(rom, 0, args.addr, args.length)
        print(f"fixed bank0  off=${args.addr:06X}: {hex_bytes(data)}")
        return

    if args.bank is not None:
        banks = [args.bank]
    elif args.banks is not None:
        banks = parse_banks(args.banks)
    else:
        banks = list(range(8))

    for bank in banks:
        off = physical_offset(bank, args.addr)
        data = read_banked(rom, bank, args.addr, args.length)
        print(f"bank ${bank:02X}  off=${off:06X}: {hex_bytes(data)}")


if __name__ == "__main__":
    main()
