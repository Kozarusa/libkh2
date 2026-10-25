"""Diffs the library's exported functions against include/kh2.h and unity/Kh2.cs.

Reads the export table of the built library itself (PE on Windows: target/release/kh2.dll; ELF or
Mach-O elsewhere through `nm`), the `KH2_API` declarations of kh2.h and the `[DllImport]` externs
of Kh2.cs, and prints what each side lacks. Exit code 1 when the DLL and kh2.h differ (the header is
the contract); Kh2.cs gaps are reported as warnings (the C# bindings cover what a Unity host uses).

    python scripts/check_bindings.py [path to the library]
"""
import re
import struct
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


def pe_exports(path):
    b = path.read_bytes()
    pe = struct.unpack_from('<I', b, 0x3c)[0]
    if b[pe:pe + 4] != b'PE\0\0':
        raise SystemExit(f'{path}: not a PE file')
    nsec = struct.unpack_from('<H', b, pe + 6)[0]
    opt_size = struct.unpack_from('<H', b, pe + 20)[0]
    opt = pe + 24
    magic = struct.unpack_from('<H', b, opt)[0]
    dirs = opt + (112 if magic == 0x20b else 96)
    exp_rva = struct.unpack_from('<I', b, dirs)[0]
    secs = opt + opt_size

    def off(rva):
        for i in range(nsec):
            s = secs + 40 * i
            vsize, va, rsize, raw = struct.unpack_from('<IIII', b, s + 8)
            if va <= rva < va + max(vsize, rsize):
                return rva - va + raw
        raise ValueError(hex(rva))

    if exp_rva == 0:
        return set()
    e = off(exp_rva)
    n_names = struct.unpack_from('<I', b, e + 24)[0]
    names_rva = struct.unpack_from('<I', b, e + 32)[0]
    names = set()
    for i in range(n_names):
        r = struct.unpack_from('<I', b, off(names_rva) + 4 * i)[0]
        o = off(r)
        names.add(b[o:b.index(b'\0', o)].decode())
    return names


def nm_exports(path):
    out = subprocess.run(['nm', '-g', '--defined-only', str(path)], capture_output=True, text=True, check=True).stdout
    return {line.split()[-1].lstrip('_') for line in out.splitlines() if line.split()}


def main():
    default = ROOT / 'target' / 'release' / ('kh2.dll' if sys.platform == 'win32' else 'libkh2.so')
    lib = Path(sys.argv[1]) if len(sys.argv) > 1 else default
    if not lib.exists():
        raise SystemExit(f'{lib} not found: cargo build --release -p libkh2 first')
    dll = {n for n in (pe_exports(lib) if lib.suffix == '.dll' else nm_exports(lib)) if n.startswith('kh2_')}
    header = set(re.findall(r'KH2_API[^;(]*?\b(kh2_\w+)\s*\(', (ROOT / 'include' / 'kh2.h').read_text(encoding='utf-8')))
    cs = set(re.findall(r'\[DllImport\([^)]*\)\][^;]*?\b(kh2_\w+)\s*\(', (ROOT / 'unity' / 'Kh2.cs').read_text(encoding='utf-8')))
    print(f'{lib.name}: {len(dll)} exports, kh2.h: {len(header)} declarations, Kh2.cs: {len(cs)} imports')
    bad = False
    for what, names in [('exported but not in kh2.h', dll - header), ('in kh2.h but not exported', header - dll), ('imported by Kh2.cs but not exported', cs - dll)]:
        if names:
            bad = True
            print(f'ERROR {what}: ' + ', '.join(sorted(names)))
    missing = sorted(header - cs)
    if missing:
        print(f'warning: in kh2.h but not in Kh2.cs ({len(missing)}): ' + ', '.join(missing))
    if not bad:
        print('the DLL, kh2.h and Kh2.cs agree' + (' (Kh2.cs is a subset)' if missing else ''))
    return 1 if bad else 0


if __name__ == '__main__':
    sys.exit(main())
