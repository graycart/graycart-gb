#!/usr/bin/env python3
"""Assert a PE binary uses IMAGE_SUBSYSTEM_WINDOWS_GUI (no console on launch).

Used by CI / release packaging to verify Windows release builds of graycart.
IMAGE_SUBSYSTEM_WINDOWS_GUI = 2
IMAGE_SUBSYSTEM_WINDOWS_CUI = 3
"""

from __future__ import annotations

import struct
import sys

IMAGE_SUBSYSTEM_WINDOWS_GUI = 2


def pe_subsystem(path: str) -> int:
    with open(path, "rb") as f:
        if f.read(2) != b"MZ":
            raise SystemExit(f"{path}: not an MZ executable")
        f.seek(0x3C)
        pe_off = struct.unpack("<I", f.read(4))[0]
        f.seek(pe_off)
        if f.read(4) != b"PE\0\0":
            raise SystemExit(f"{path}: missing PE signature")
        # Optional header starts immediately after the 20-byte COFF header.
        # Subsystem is a u16 at optional-header offset 68 (PE32 and PE32+).
        f.seek(pe_off + 24 + 68)
        return struct.unpack("<H", f.read(2))[0]


def main() -> None:
    if len(sys.argv) != 2:
        raise SystemExit(f"usage: {sys.argv[0]} <path-to.exe>")
    path = sys.argv[1]
    subsystem = pe_subsystem(path)
    names = {2: "WINDOWS_GUI", 3: "WINDOWS_CUI"}
    label = names.get(subsystem, f"other({subsystem})")
    print(f"{path}: subsystem={subsystem} ({label})")
    if subsystem != IMAGE_SUBSYSTEM_WINDOWS_GUI:
        raise SystemExit(
            f"expected IMAGE_SUBSYSTEM_WINDOWS_GUI ({IMAGE_SUBSYSTEM_WINDOWS_GUI}), "
            f"got {subsystem} ({label})"
        )


if __name__ == "__main__":
    main()
