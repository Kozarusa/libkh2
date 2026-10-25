"""Generates docs/API-REFERENCE.md from include/kh2.h: the header's sections (`/* ---- Name ---- */`),
each with its comments, #defines, structs and functions in header order.

    python scripts/gen_api_ref.py          # write docs/API-REFERENCE.md
    python scripts/gen_api_ref.py --check  # exit 1 when the file is not up to date
"""
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HEADER = ROOT / 'include' / 'kh2.h'
OUT = ROOT / 'docs' / 'API-REFERENCE.md'


def clean_comment(text):
    text = text.strip()
    text = re.sub(r'^/\*+', '', text)
    text = re.sub(r'\*+/$', '', text)
    lines = [re.sub(r'^\s*\* ?', '', l).rstrip() for l in text.split('\n')]
    return ' '.join(l.strip() for l in lines if l.strip())


def generate():
    src = HEADER.read_text(encoding='utf-8').replace('\r\n', '\n')
    # Drop the include guard, extern "C" and the export macro block.
    out = ['# libkh2 C API reference', '',
           'Generated from `include/kh2.h` by `scripts/gen_api_ref.py`; do not edit by hand.', '']
    toc = ['- [Core](#core)']
    body = ['## Core', '']
    pending = []  # comments not yet attached
    i = 0
    lines = src.split('\n')
    n = len(lines)
    section = 'Core'

    def flush_pending():
        for c in pending:
            body.append(c)
            body.append('')
        pending.clear()

    while i < n:
        line = lines[i]
        s = line.strip()
        if s.startswith('/*'):
            j = i
            block = [line]
            while '*/' not in lines[j]:
                j += 1
                block.append(lines[j])
            text = '\n'.join(block)
            i = j + 1
            m = re.match(r'\s*/\*\s*-{3,}\s*(.*?)\s*-{3,}(.*)', text, re.S)
            if m:
                flush_pending()
                title = m.group(1).strip()
                rest = clean_comment('/*' + m.group(2)) if m.group(2).strip(' */\n') else ''
                # A section title may carry its note in parentheses.
                t = re.match(r'(.*?)\s*\((.*)\)\s*$', title, re.S)
                name, note = (t.group(1), t.group(2)) if t else (title, '')
                section = name
                anchor = re.sub(r'[^a-z0-9 -]', '', name.lower()).replace(' ', '-')
                toc.append(f'- [{name}](#{anchor})')
                body.append(f'## {name}')
                body.append('')
                for x in (note, rest):
                    x = ' '.join(x.split())
                    if x:
                        body.append(x)
                        body.append('')
                continue
            c = clean_comment(text)
            if c:
                pending.append(c)
            continue
        if s.startswith('#define KH2_') and not s.startswith('#define KH2_API') and not s.startswith('#define KH2_H'):
            m = re.match(r'#define\s+(\w+)\s+(.*?)\s*(/\*(.*?)\*/)?\s*$', s)
            if m and section:
                flush_pending()
                note = f' — {m.group(4).strip()}' if m.group(4) else ''
                body.append(f'- `{m.group(1)}` = `{m.group(2)}`{note}')
                # A define list ends at the next non-define line.
                if not lines[i + 1].strip().startswith('#define KH2_'):
                    body.append('')
            i += 1
            continue
        if s.startswith('typedef struct'):
            j = i
            block = [line]
            while not re.match(r'\s*}\s*\w+\s*;', lines[j]):
                j += 1
                block.append(lines[j])
            name = re.match(r'\s*}\s*(\w+)', lines[j]).group(1)
            flush_pending()
            body.append(f'### `{name}`')
            body.append('')
            body.append('```c')
            body.extend(block)
            body.append('```')
            body.append('')
            i = j + 1
            continue
        if s.startswith('KH2_API'):
            j = i
            decl = [s]
            while not decl[-1].rstrip().endswith(';'):
                j += 1
                decl.append(lines[j].strip())
            d = ' '.join(decl).replace('KH2_API ', '')
            name = re.search(r'(\w+)\s*\(', d).group(1)
            body.append(f'### `{name}`')
            body.append('')
            body.append('```c')
            body.append(d)
            body.append('```')
            body.append('')
            for c in pending:
                body.append(c)
                body.append('')
            pending.clear()
            i = j + 1
            continue
        i += 1
    flush_pending()
    return '\n'.join(out + ['## Sections', ''] + toc + [''] + body).rstrip('\n') + '\n'


def main():
    text = generate()
    if '--check' in sys.argv:
        now = OUT.read_text(encoding='utf-8').replace('\r\n', '\n') if OUT.exists() else ''
        if now != text:
            print(f'{OUT} is not up to date: run python scripts/gen_api_ref.py')
            sys.exit(1)
        return
    OUT.parent.mkdir(exist_ok=True)
    OUT.write_text(text, encoding='utf-8', newline='\n')
    print(f'wrote {OUT} ({text.count(chr(10))} lines)')


if __name__ == '__main__':
    main()
