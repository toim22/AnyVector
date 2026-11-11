"""Checks a translation file against the strings AnyVector shows.

    python -I xtask/i18n_check.py <repo> <lang> [--source]

Compares crates/anyvector/assets/i18n/<lang>.tsv with the keys of pt-BR.tsv, the reference
translation (`python -I xtask/i18n.py <repo> pt-BR` lists what pt-BR itself still lacks), and reports:

- strings that are not translated
- translations whose `{}` / `{0}` placeholders, `\\n` line breaks or `&` access keys don't match the English
- keys that are in the file twice

With --source it also writes target/i18n/source.tsv: every string to translate, one per line, as
`English<TAB>pt-BR translation` (the Portuguese shows the meaning).
Exits with 1 when something is wrong.
"""
import os
import re
import sys

root, lang = sys.argv[1], sys.argv[2]
assets = os.path.join(root, 'crates', 'anyvector', 'assets', 'i18n')
out_dir = os.path.join(root, 'target', 'i18n')


def rows(path):
    """(raw key, raw value) in file order, escapes kept."""
    out = []
    if os.path.exists(path):
        for row in open(path, encoding='utf-8-sig'):
            row = row.rstrip('\n').rstrip('\r')
            if not row or row.startswith('#') or '\t' not in row:
                continue
            k, v = row.split('\t', 1)
            out.append((k, v))
    return out


def source_keys():
    """The strings to translate: the keys of pt-BR.tsv (the reference translation), in its order."""
    keys, seen = [], set()
    pt = rows(os.path.join(assets, 'pt-BR.tsv'))
    for k, _ in pt:
        if k not in seen:
            seen.add(k)
            keys.append(k)
    return keys, dict(pt)


keys, pt = source_keys()

if '--source' in sys.argv:
    os.makedirs(out_dir, exist_ok=True)
    with open(os.path.join(out_dir, 'source.tsv'), 'w', encoding='utf-8', newline='\n') as w:
        for k in keys:
            w.write(f'{k}\t{pt.get(k, "")}\n')
    print(f'{len(keys)} strings written to target/i18n/source.tsv')

PH = re.compile(r'\{\d*\}')


def amp_count(s):
    return len(re.findall(r'(?<!&)&(?!&)', s.replace('&&', '')))


have = rows(os.path.join(assets, f'{lang}.tsv'))
d, dupes = {}, []
for k, v in have:
    if k in d:
        dupes.append(k)
    d[k] = v

problems = []
missing = [k for k in keys if k not in d]
for k, v in have:
    if not v.strip():
        problems.append(f'empty: {k}')
        continue
    if len(PH.findall(k)) != len(PH.findall(v)):
        problems.append(f'placeholders: {k}  ->  {v}')
    if k.count('\\n') != v.count('\\n'):
        problems.append(f'line breaks: {k}  ->  {v}')
    if amp_count(v) > 1:
        problems.append(f'more than one access key: {v}')
    if '\t' in v:
        problems.append(f'tab in the translation: {k}')

print(f'{lang}: {len(keys)} strings, {len(keys) - len(missing)} translated, {len(missing)} missing, '
      f'{len(dupes)} duplicate keys, {len(problems)} problems')
for k in missing[:20]:
    print('  missing:', k)
for k in dupes[:20]:
    print('  duplicate:', k)
for p in problems[:40]:
    print(' ', p)
sys.exit(1 if missing or dupes or problems else 0)
