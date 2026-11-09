"""User interface strings for translation.

    python -I xtask/i18n.py <repo> [lang]          (lang: pt-BR)

Lists every English string the program shows: the captions and tooltips of the generated menus,
toolbars and context menus (crates/anyvector/src/ui/data.rs) and the string literals of the app's
source that look like user interface text. Format strings keep their `{}` placeholders.

Writes target/i18n/strings.tsv (index, English, where it is used) and target/i18n/missing-<lang>.tsv
(the strings crates/anyvector/assets/i18n/<lang>.tsv does not translate yet) and prints the coverage.
"""
import os
import re
import sys

root = sys.argv[1]
lang = sys.argv[2] if len(sys.argv) > 2 else 'pt-BR'
src = os.path.join(root, 'crates', 'anyvector', 'src')

SKIP_LINE = re.compile(r'\b(assert\w*!|debug_assert\w*!|println!|eprintln!|print!|log::|debug!|trace!|panic!|unreachable!|todo!|include_str!|include_bytes!|env!|concat!|\.expect\(|from_id_salt\(|Id::new\(|\.with\(|id_salt\(|scope_id\(|get\(ui\.ctx\(\)|icons\.get\(|\.header\(|regex|Regex)')
PLACEHOLDER = re.compile(r'\{[^{}]*\}')


def literals(text):
    """(line, value, is_format) for each string literal outside comments."""
    i, n, line = 0, len(text), 1
    out = []
    while i < n:
        c = text[i]
        if c == '\n':
            line += 1
            i += 1
        elif text.startswith('//', i):
            j = text.find('\n', i)
            i = n if j < 0 else j
        elif text.startswith('/*', i):
            j = text.find('*/', i + 2)
            j = n if j < 0 else j + 2
            line += text.count('\n', i, j)
            i = j
        elif c == 'r' and re.match(r'r#*"', text[i:i + 10]) and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == '_')):
            m = re.match(r'r(#*)"', text[i:])
            hashes = m.group(1)
            start = i + len(m.group(0))
            end = text.find('"' + hashes, start)
            end = n if end < 0 else end
            out.append((line, text[start:end], False))
            line += text.count('\n', i, end)
            i = end + 1 + len(hashes)
        elif c == 'b' and text.startswith('b"', i) and (i == 0 or not (text[i - 1].isalnum() or text[i - 1] == '_')):
            j = i + 2
            while j < n and text[j] != '"':
                j += 2 if text[j] == '\\' else 1
            line += text.count('\n', i, j)
            i = j + 1
        elif c == '"':
            j = i + 1
            buf = []
            while j < n and text[j] != '"':
                if text[j] == '\\':
                    e = text[j + 1]
                    if e == 'n':
                        buf.append('\n')
                    elif e == 't':
                        buf.append('\t')
                    elif e == '\\':
                        buf.append('\\')
                    elif e == '"':
                        buf.append('"')
                    elif e == "'":
                        buf.append("'")
                    elif e == '0':
                        buf.append('\0')
                    elif e == 'u':
                        k = text.index('}', j)
                        buf.append(chr(int(text[j + 3:k], 16)))
                        j = k - 1
                    elif e == '\n':
                        # line continuation: skip the newline and the next line's indentation
                        k = j + 2
                        while k < n and text[k] in ' \t\r\n':
                            k += 1
                        j = k - 2
                    j += 2
                else:
                    buf.append(text[j])
                    j += 1
            out.append((line, ''.join(buf), True))
            line += text.count('\n', i, j)
            i = j + 1
        elif c == "'":
            # char literal ('x', '\n', '\u{..}') or a lifetime
            m = re.match(r"'(\\u\{[0-9a-fA-F]+\}|\\.|[^\\'])'", text[i:])
            i += len(m.group(0)) if m else 1
        else:
            i += 1
    return out


def ui_like(v):
    if len(v.strip()) < 2 or not re.search(r'[A-Za-z]{2}', v):
        return False
    if '://' in v or v.lstrip().startswith(('<', '#', '{"', 'M ', 'm ')):
        return False
    if not (re.search(r'[A-Z]', v) or ' ' in v.strip()):
        return False
    if re.fullmatch(r'[A-Za-z0-9_]+(::[A-Za-z0-9_]+)+', v):
        return False
    if re.fullmatch(r'(Ctrl|Alt|Shift)(\+[A-Za-z0-9]+)+|F\d+', v):
        return False
    t = v.strip()
    if ' ' not in t:
        # identifiers (ThreeDRotate, BestFit), codes (AFK, af-ZA) and units are not text
        if re.fullmatch(r'[A-Z]?[a-z0-9]+(?:[A-Z][a-z0-9]+)+', t) or re.fullmatch(r'[A-Z0-9]{2,5}', t):
            return False
        if re.fullmatch(r'[a-z]{2,3}(-[A-Za-z]{2,4})+', t):
            return False
    if re.fullmatch(r'[\w.\-]+\.(png|svg|json|xml|txt|cdr|rs|tsv|md|ico|exe|dll|ttf|otf|html|css|js)', v):
        return False
    return True


def key_of(v, is_format):
    if not is_format:
        return v
    # a format string: each placeholder becomes {} ({{ and }} are literal braces)
    tmp = v.replace('{{', '\x01').replace('}}', '\x02')
    if not PLACEHOLDER.search(tmp):
        return v
    tmp = PLACEHOLDER.sub('{}', tmp)
    return tmp.replace('\x01', '{').replace('\x02', '}')


def strip_tests(text):
    k = text.find('#[cfg(test)]')
    return text if k < 0 else text[:k]


found = {}


def add(v, where):
    v = v.strip('\n')
    if v and v not in found:
        found[v] = where


# 1. generated user interface data: captions and tooltips
data = open(os.path.join(src, 'ui', 'data.rs'), encoding='utf-8').read()
for m in re.finditer(r'text: "((?:[^"\\]|\\.)*)", tip: "((?:[^"\\]|\\.)*)"', data):
    for v in m.groups():
        v = v.replace('\\"', '"').replace('\\\\', '\\').replace('\\n', '\n')
        if v and re.search(r'[A-Za-z]{2}', v):
            add(v.replace('&&', '\x01').replace('&', '').replace('\x01', '&'), 'ui/data.rs')
for m in re.finditer(r'name: "((?:[^"\\]|\\.)*)"', data):
    v = m.group(1)
    if re.search(r'[A-Za-z]{2}', v):
        add(v, 'ui/data.rs')

# 2. string literals of the app's source
skip_files = {'data.rs', 'visual_tests.rs'}
for dirpath, _, files in os.walk(src):
    for f in sorted(files):
        if not f.endswith('.rs') or f in skip_files:
            continue
        path = os.path.join(dirpath, f)
        rel = os.path.relpath(path, src).replace('\\', '/')
        text = strip_tests(open(path, encoding='utf-8').read())
        lines = text.split('\n')
        for ln, v, fmt in literals(text):
            l = lines[ln - 1] if ln - 1 < len(lines) else ''
            if l.lstrip().startswith('#[') or SKIP_LINE.search(l):
                continue
            k = key_of(v, fmt)
            if ui_like(k):
                add(k, f'{rel}:{ln}')

strings = list(found.items())


def esc(s):
    return s.replace('\\', '\\\\').replace('\t', '\\t').replace('\n', '\\n')


def unesc(s):
    out, i = [], 0
    while i < len(s):
        if s[i] == '\\' and i + 1 < len(s):
            out.append({'n': '\n', 't': '\t', '\\': '\\'}.get(s[i + 1], s[i + 1]))
            i += 2
        else:
            out.append(s[i])
            i += 1
    return ''.join(out)


def load_dict(path):
    d = {}
    if os.path.exists(path):
        for row in open(path, encoding='utf-8'):
            row = row.rstrip('\n').rstrip('\r')
            if not row or row.startswith('#') or '\t' not in row:
                continue
            k, v = row.split('\t', 1)
            d[unesc(k)] = unesc(v)
    return d


out_dir = os.path.join(root, 'target', 'i18n')
os.makedirs(out_dir, exist_ok=True)
with open(os.path.join(out_dir, 'strings.tsv'), 'w', encoding='utf-8', newline='\n') as w:
    for i, (k, where) in enumerate(strings):
        w.write(f'{i}\t{esc(k)}\t{where}\n')

dict_path = os.path.join(root, 'crates', 'anyvector', 'assets', 'i18n', f'{lang}.tsv')
d = load_dict(dict_path)
missing = [(i, k, where) for i, (k, where) in enumerate(strings) if k not in d and k.replace('%s', '{}') not in d]
with open(os.path.join(out_dir, f'missing-{lang}.tsv'), 'w', encoding='utf-8', newline='\n') as w:
    for i, k, where in missing:
        w.write(f'{i}\t{esc(k)}\t{where}\n')
done = len(strings) - len(missing)
print(f'{len(strings)} strings, {done} translated to {lang}, {len(missing)} missing '
      f'({sum(len(k) for _, k, _ in missing)} characters); see target/i18n/missing-{lang}.tsv')
