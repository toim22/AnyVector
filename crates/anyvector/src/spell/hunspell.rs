//! The spell checker without Windows: the system's Hunspell dictionaries, read by `spellbook`, with
//! what the Windows Spell Checking API keeps beside them. Add puts a word in the user's dictionary of
//! the language and Auto Replace a pair in its auto-correct list, both kept in
//! `~/.config/AnyVector/Spelling/<language>` (`default.dic`, `default.acl`, as Windows keeps them in
//! `%AppData%\Microsoft\Spelling\<language>`); Skip All lasts until AnyVector closes.

use super::SpellError;
use std::cell::RefCell;
use std::collections::{HashMap, HashSet};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Where the dictionaries are, and the folder of the user's own words.
#[derive(Clone)]
struct Places {
    dirs: Vec<PathBuf>,
    user: PathBuf,
}

/// A language's dictionary with the user's words.
struct Speller {
    dict: spellbook::Dictionary,
    /// Skip All: until AnyVector closes.
    ignored: HashSet<String>,
    /// Auto Replace pairs.
    auto: Vec<(String, String)>,
    /// `<user>/<language>`.
    dir: PathBuf,
}

thread_local! {
    /// Loaded dictionaries by language (loading one takes a while); `None` when none is installed.
    static CHECKERS: RefCell<HashMap<String, Option<Speller>>> = RefCell::new(HashMap::new());
    /// Other places than the system's (tests).
    static PLACES: RefCell<Option<Places>> = const { RefCell::new(None) };
}

/// The folders Hunspell dictionaries are looked for in, in order: `$DICPATH`, the user's
/// `$XDG_DATA_HOME/hunspell` (`~/.local/share/hunspell`), `hunspell` in each of `$XDG_DATA_DIRS`
/// (`/usr/local/share`, `/usr/share`), then the old MySpell folders. `var` reads an environment
/// variable.
pub fn dictionary_dirs(var: impl Fn(&str) -> Option<OsString>, home: Option<&Path>) -> Vec<PathBuf> {
    let mut out: Vec<PathBuf> = vec![];
    let mut push = |p: PathBuf| {
        if !p.as_os_str().is_empty() && !out.contains(&p) {
            out.push(p);
        }
    };
    if let Some(list) = var("DICPATH") {
        for p in std::env::split_paths(&list) {
            push(p);
        }
    }
    // XDG folders count only when absolute
    let absolute = |v: Option<OsString>| v.map(PathBuf::from).filter(|p| p.is_absolute());
    match absolute(var("XDG_DATA_HOME")) {
        Some(d) => push(d.join("hunspell")),
        None => {
            if let Some(h) = home {
                push(h.join(".local/share/hunspell"));
            }
        }
    }
    let data_dirs = var("XDG_DATA_DIRS").filter(|v| !v.is_empty()).unwrap_or_else(|| "/usr/local/share:/usr/share".into());
    for d in std::env::split_paths(&data_dirs).filter(|p| p.is_absolute()) {
        push(d.join("hunspell"));
    }
    for d in ["/usr/share/hunspell", "/usr/share/myspell", "/usr/share/myspell/dicts"] {
        push(PathBuf::from(d));
    }
    out
}

/// A dictionary's name as compared: `pt_BR`, `pt-BR` and `pt_br` are the same.
fn norm(name: &str) -> String {
    name.to_ascii_lowercase().replace('-', "_")
}

/// The installed dictionaries (name without extension, .aff, .dic), the first folder winning when
/// two have one of the same name. A .dic without its .aff (hyphenation patterns) is not one.
fn installed(dirs: &[PathBuf]) -> Vec<(String, PathBuf, PathBuf)> {
    let mut out: Vec<(String, PathBuf, PathBuf)> = vec![];
    for d in dirs {
        let Ok(rd) = std::fs::read_dir(d) else { continue };
        let mut found: Vec<(String, PathBuf, PathBuf)> = rd
            .flatten()
            .filter_map(|e| {
                let p = e.path();
                let ext = p.extension()?.to_str()?;
                if !ext.eq_ignore_ascii_case("dic") {
                    return None;
                }
                let stem = p.file_stem()?.to_str()?.to_string();
                let aff = ["aff", "AFF"].iter().map(|x| p.with_extension(x)).find(|a| a.is_file())?;
                Some((stem, aff, p))
            })
            .collect();
        found.sort_by(|a, b| a.0.cmp(&b.0));
        for f in found {
            if !out.iter().any(|o| norm(&o.0) == norm(&f.0)) {
                out.push(f);
            }
        }
    }
    out
}

/// The dictionary for a language tag (`pt-BR`, `sr-Latn-RS`): the one of the language and its
/// region, else of the language alone, else another region's of the same language (pt_PT for
/// pt-BR, en_GB for en-US): the language's own country first (de_DE, fr_FR, pt_PT), then en_US,
/// then by name. Returns (.aff, .dic).
pub fn find_dictionary(tag: &str, dirs: &[PathBuf]) -> Option<(PathBuf, PathBuf)> {
    let list = installed(dirs);
    let parts: Vec<String> = tag.split(['-', '_']).map(|s| s.to_ascii_lowercase()).collect();
    let lang = parts.first().filter(|l| !l.is_empty())?.clone();
    let script = parts.iter().skip(1).find(|p| p.len() == 4 && p.bytes().all(|b| b.is_ascii_alphabetic())).cloned();
    let region = parts.iter().skip(1).find(|p| p.len() == 2 || (p.len() == 3 && p.bytes().all(|b| b.is_ascii_digit()))).cloned();
    let mut wanted: Vec<String> = vec![];
    match (&script, &region) {
        (Some(s), Some(r)) => wanted.extend([format!("{lang}_{s}_{r}"), format!("{lang}_{s}"), format!("{lang}_{r}")]),
        (Some(s), None) => wanted.push(format!("{lang}_{s}")),
        (None, Some(r)) => wanted.push(format!("{lang}_{r}")),
        (None, None) => {}
    }
    wanted.push(lang.clone());
    for w in &wanted {
        if let Some(d) = list.iter().find(|d| norm(&d.0) == *w) {
            return Some((d.1.clone(), d.2.clone()));
        }
    }
    let prefix = format!("{lang}_");
    let rank = |name: &str| {
        let n = norm(name);
        if n == format!("{lang}_{lang}") {
            0
        } else if n == "en_us" {
            1
        } else {
            2
        }
    };
    let mut same: Vec<&(String, PathBuf, PathBuf)> = list.iter().filter(|d| norm(&d.0).starts_with(&prefix)).collect();
    same.sort_by(|a, b| (rank(&a.0), norm(&a.0)).cmp(&(rank(&b.0), norm(&b.0))));
    same.first().map(|d| (d.1.clone(), d.2.clone()))
}

/// The text of a dictionary file in the character set its .aff names (`SET ISO8859-1`); Hunspell's
/// default is ISO 8859-1, but a file without `SET` that is valid UTF-8 is read as UTF-8.
fn read_in(bytes: &[u8], set: Option<&str>) -> String {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let label = set.map(|s| s.trim().to_ascii_lowercase());
    let encoding = match label.as_deref() {
        Some("utf-8") | None => {
            if let Ok(s) = std::str::from_utf8(bytes) {
                return s.to_string();
            }
            encoding_rs::WINDOWS_1252
        }
        Some("microsoft-cp1251") => encoding_rs::WINDOWS_1251,
        Some(l) if l.starts_with("tis620") => encoding_rs::WINDOWS_874,
        Some(l) => encoding_rs::Encoding::for_label(l.as_bytes()).or_else(|| encoding_rs::Encoding::for_label(l.replace("iso8859", "iso-8859").as_bytes())).unwrap_or(encoding_rs::WINDOWS_1252),
    };
    encoding.decode_without_bom_handling(bytes).0.into_owned()
}

/// A dictionary from its two files, whatever their character set.
fn load_dictionary(aff: &Path, dic: &Path) -> Option<spellbook::Dictionary> {
    let aff_bytes = std::fs::read(aff).ok()?;
    let dic_bytes = std::fs::read(dic).ok()?;
    let set = String::from_utf8_lossy(&aff_bytes)
        .lines()
        .find_map(|l| l.trim_start_matches('\u{FEFF}').strip_prefix("SET").filter(|r| r.starts_with([' ', '\t'])).map(|r| r.trim().to_string()));
    let aff_text = read_in(&aff_bytes, set.as_deref());
    let dic_text = read_in(&dic_bytes, set.as_deref());
    match spellbook::Dictionary::new(&aff_text, &dic_text) {
        Ok(d) => Some(d),
        Err(e) => {
            log::warn!("spell: {} could not be read: {e}", dic.display());
            None
        }
    }
}

/// A word as a .dic line: a slash in it is escaped.
fn dic_line(word: &str) -> String {
    word.replace('/', "\\/")
}

/// The words of a user's dictionary file, one a line.
fn read_words(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path).map(|s| s.lines().map(|l| l.trim().to_string()).filter(|l| !l.is_empty()).collect()).unwrap_or_default()
}

/// The pairs of an auto-correct file, `from<TAB>to` a line.
fn read_pairs(path: &Path) -> Vec<(String, String)> {
    std::fs::read_to_string(path).map(|s| s.lines().filter_map(|l| l.split_once('\t')).filter(|(a, _)| !a.is_empty()).map(|(a, b)| (a.to_string(), b.to_string())).collect()).unwrap_or_default()
}

impl Speller {
    fn load(lang: &str, places: &Places) -> Option<Speller> {
        let (aff, dic) = find_dictionary(lang, &places.dirs)?;
        let mut dict = load_dictionary(&aff, &dic)?;
        let dir = places.user.join(lang);
        for w in read_words(&dir.join("default.dic")) {
            let _ = dict.add(&dic_line(&w));
        }
        let auto = read_pairs(&dir.join("default.acl"));
        Some(Speller { dict, ignored: HashSet::new(), auto, dir })
    }

    fn correct(&self, word: &str) -> bool {
        if self.ignored.contains(word) || self.dict.check(word) {
            return true;
        }
        // the typographic apostrophe as the dictionaries have it
        word.contains('’') && self.dict.check(&word.replace('’', "'"))
    }

    /// The auto-correct replacement of a word: its pair, or the pair of its lower-case form
    /// with the same capitals ("Teh" gives "The").
    fn auto_for(&self, word: &str) -> Option<String> {
        if let Some((_, to)) = self.auto.iter().find(|(f, _)| f == word) {
            return Some(to.clone());
        }
        let lower = word.to_lowercase();
        let (_, to) = self.auto.iter().find(|(f, _)| *f == lower)?;
        if word.chars().count() > 1 && word.chars().all(|c| !c.is_lowercase()) {
            return Some(to.to_uppercase());
        }
        let mut c = to.chars();
        c.next().map(|f| f.to_uppercase().collect::<String>() + c.as_str())
    }
}

fn places() -> Places {
    PLACES.with(|p| p.borrow().clone()).unwrap_or_else(|| Places {
        dirs: dictionary_dirs(|n| std::env::var_os(n), dirs::home_dir().as_deref()),
        user: crate::settings::config_dir().join("Spelling"),
    })
}

/// Use these dictionary folders and this user folder on this thread (tests); forgets the
/// dictionaries loaded.
#[cfg(test)]
pub fn use_places(dirs: Vec<PathBuf>, user: PathBuf) {
    PLACES.with(|p| *p.borrow_mut() = Some(Places { dirs, user }));
    CHECKERS.with(|c| c.borrow_mut().clear());
}

/// Runs `f` on the language's speller, loading it the first time.
fn with<R>(lang: &str, f: impl FnOnce(&mut Speller) -> R) -> Option<R> {
    let lang = if lang.is_empty() { super::DEFAULT_LANG } else { lang };
    CHECKERS.with(|c| {
        let mut c = c.borrow_mut();
        if !c.contains_key(lang) {
            let s = Speller::load(lang, &places());
            c.insert(lang.to_string(), s);
        }
        c.get_mut(lang).and_then(|s| s.as_mut()).map(f)
    })
}

/// Whether the language has a dictionary. Looked for again when there was none, so one installed
/// meanwhile is found.
pub fn supported(lang: &str) -> bool {
    let key = if lang.is_empty() { super::DEFAULT_LANG } else { lang };
    CHECKERS.with(|c| c.borrow_mut().retain(|l, s| l != key || s.is_some()));
    with(lang, |_| ()).is_some()
}

/// Whether no dictionary was found when the language was last asked for.
pub fn missing(lang: &str) -> bool {
    with(lang, |_| ()).is_none()
}

/// The words of a text, as Windows finds them: (start, length) in chars. A word is letters and
/// digits, with apostrophes, hyphens, full stops, colons and underscores inside it. Web and e-mail
/// addresses, and what follows #, @ or $, are not words.
fn words(text: &str) -> Vec<(usize, usize)> {
    let chars: Vec<char> = text.chars().collect();
    let alnum = |i: usize| chars.get(i).is_some_and(|c| c.is_alphanumeric());
    // the chars of addresses and tags
    let mut skip = vec![false; chars.len()];
    let mut i = 0;
    while i < chars.len() {
        if chars[i].is_whitespace() {
            i += 1;
            continue;
        }
        let s = i;
        while i < chars.len() && !chars[i].is_whitespace() {
            i += 1;
        }
        let chunk: String = chars[s..i].iter().collect::<String>().to_lowercase();
        let mail = (s + 1..i).any(|k| chars[k] == '@' && alnum(k - 1) && alnum(k + 1));
        if chunk.contains("://") || chunk.starts_with("www.") || mail {
            skip[s..i].fill(true);
        } else if matches!(chars[s], '#' | '@' | '$') {
            let end = (s + 1..i).find(|k| !chars[*k].is_alphanumeric() && chars[*k] != '_').unwrap_or(i);
            skip[s..end].fill(true);
        }
    }
    let inner = |c: char| matches!(c, '\'' | '’' | '-' | '.' | ':' | '_');
    let mut out = vec![];
    let mut i = 0;
    while i < chars.len() {
        if !alnum(i) || skip[i] {
            i += 1;
            continue;
        }
        let s = i;
        while i < chars.len() && (alnum(i) || (inner(chars[i]) && alnum(i + 1))) {
            i += 1;
        }
        out.push((s, i - s));
    }
    out
}

/// Endings that make a word with a full stop a file name, which is not checked.
const FILE_ENDINGS: &[&str] = &["txt", "doc", "docx", "xls", "xlsx", "ppt", "pptx", "pdf", "rtf", "csv", "cdr", "cdt", "cmx", "svg", "eps", "ai", "psd", "png", "jpg", "jpeg", "gif", "bmp", "tif", "tiff", "webp", "ico", "dxf", "dwg", "emf", "wmf", "ttf", "otf", "zip", "exe", "dll", "htm", "html", "xml", "json", "css", "js", "mp3", "mp4", "com", "org", "net"];

/// The wrong parts of a word: (start, length) in its chars; empty when it is right.
fn wrong_parts(sp: &Speller, word: &str) -> Vec<(usize, usize)> {
    let len = word.chars().count();
    // as Windows: words with digits and words all in capitals are not checked
    if word.chars().any(char::is_numeric) || !word.chars().any(char::is_lowercase) && word.chars().any(char::is_uppercase) && len > 1 {
        return vec![];
    }
    if sp.correct(word) {
        return vec![];
    }
    if word.contains('.') {
        let parts: Vec<&str> = word.split('.').collect();
        let ending = parts.last().copied().unwrap_or_default().to_lowercase();
        // an abbreviation (its last full stop is not part of the word) or a file name
        if sp.correct(&format!("{word}.")) || parts.iter().all(|p| p.chars().count() == 1) || FILE_ENDINGS.contains(&ending.as_str()) {
            return vec![];
        }
    }
    let parts: Vec<&str> = word.split('-').collect();
    if parts.len() > 1 {
        // a hyphenated word: the parts that are wrong, or all of it when every part is
        let mut out = vec![];
        let mut at = 0;
        for p in &parts {
            let n = p.chars().count();
            out.extend(wrong_parts(sp, p).into_iter().map(|(s, l)| (at + s, l)));
            at += n + 1;
        }
        if out.len() < parts.len() {
            return out;
        }
    }
    vec![(0, len)]
}

pub fn check(text: &str, lang: &str) -> Vec<SpellError> {
    with(lang, |sp| {
        let chars: Vec<char> = text.chars().collect();
        let mut out = vec![];
        // the word before, for repeated words
        let mut prev: Option<(String, usize)> = None;
        for (s, l) in words(text) {
            let w: String = chars[s..s + l].iter().collect();
            if w.chars().any(super::unchecked_script) || !w.chars().any(char::is_alphabetic) {
                prev = None;
                continue;
            }
            let lower = w.to_lowercase().replace('’', "'");
            // "the the": the second one goes (only with spaces between them)
            let repeated = prev.as_ref().is_some_and(|(p, end)| *p == lower && chars[*end..s].iter().all(|c| matches!(c, ' ' | '\t' | '\u{A0}')));
            prev = Some((lower, s + l));
            if let Some(to) = sp.auto_for(&w) {
                out.push(SpellError { start: s, len: l, delete: false, replacement: Some(to) });
                continue;
            }
            let wrong = wrong_parts(sp, &w);
            if wrong.is_empty() && repeated {
                out.push(SpellError { start: s, len: l, delete: true, replacement: None });
            }
            out.extend(wrong.into_iter().map(|(a, n)| SpellError { start: s + a, len: n, delete: false, replacement: None }));
        }
        out
    })
    .unwrap_or_default()
}

pub fn suggest(word: &str, lang: &str) -> Vec<String> {
    with(lang, |sp| {
        let mut out = vec![];
        sp.dict.suggest(word, &mut out);
        out
    })
    .unwrap_or_default()
}

/// Add: the word joins the user's dictionary of the language, now and in later sessions.
pub fn add(word: &str, lang: &str) {
    let word = word.trim();
    if word.is_empty() {
        return;
    }
    with(lang, |sp| {
        let _ = sp.dict.add(&dic_line(word));
        let path = sp.dir.join("default.dic");
        let mut list = read_words(&path);
        if !list.iter().any(|w| w == word) {
            list.push(word.to_string());
            let _ = std::fs::create_dir_all(&sp.dir).and_then(|()| std::fs::write(&path, list.join("\n") + "\n"));
        }
    });
}

/// Skip All: the word is right until AnyVector closes.
pub fn ignore(word: &str, lang: &str) {
    with(lang, |sp| {
        sp.ignored.insert(word.to_string());
    });
}

/// Auto Replace: `from` is reported with `to` as its replacement from now on (kept).
pub fn auto_correct(from: &str, to: &str, lang: &str) {
    if from.is_empty() || to.is_empty() || from.contains(['\t', '\n']) || to.contains(['\t', '\n']) {
        return;
    }
    with(lang, |sp| {
        match sp.auto.iter_mut().find(|(f, _)| f == from) {
            Some(p) => p.1 = to.to_string(),
            None => sp.auto.push((from.to_string(), to.to_string())),
        }
        let text: String = sp.auto.iter().map(|(f, t)| format!("{f}\t{t}\n")).collect();
        let _ = std::fs::create_dir_all(&sp.dir).and_then(|()| std::fs::write(sp.dir.join("default.acl"), text));
    });
}

/// The package of a distribution that has a language's Hunspell dictionary, as Debian and Ubuntu
/// name them: `hunspell-en-us`, `hunspell-pt-br`, `hunspell-de-de` for the languages split by
/// country, `hunspell-fr`, `hunspell-ru` for the others.
pub fn package_for(lang: &str) -> String {
    let parts: Vec<String> = lang.split(['-', '_']).map(|s| s.to_ascii_lowercase()).collect();
    let l = parts.first().cloned().unwrap_or_default();
    let region = parts.iter().skip(1).find(|p| p.len() == 2).cloned();
    match (l.as_str(), region) {
        ("en" | "pt" | "de", Some(r)) => format!("hunspell-{l}-{r}"),
        _ => format!("hunspell-{l}"),
    }
}

/// Writes a small dictionary in the Hunspell format, `<dir>/<name>.aff` and `.dic`: a few English
/// words, `S` adding a plural (tests).
#[cfg(test)]
pub fn write_test_dictionary(dir: &Path, name: &str) {
    const AFF: &str = "SET UTF-8\nTRY esianrtolcdugmphbyfvkwzESIANRTOLCDUGMPHBYFVKWZ'\nREP 1\nREP teh the\n\nSFX S Y 1\nSFX S 0 s .\n";
    const DIC: &str = "13\nhello\nworld/S\nthe\ntext/S\nis\na\nwell\nknown\ndon't\ncolour/S\nartistic\nthis\nan\n";
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(dir.join(format!("{name}.aff")), AFF).unwrap();
    std::fs::write(dir.join(format!("{name}.dic")), DIC).unwrap();
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("anyvector-spell-{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn write_dict(dir: &Path, name: &str) {
        write_test_dictionary(dir, name);
    }

    #[test]
    fn dictionary_folders_follow_hunspell_and_xdg() {
        let env = |vars: &'static [(&'static str, &'static str)]| move |n: &str| vars.iter().find(|v| v.0 == n).map(|v| OsString::from(v.1));
        let home = Path::new("/home/u");
        let dirs = dictionary_dirs(env(&[]), Some(home));
        assert_eq!(dirs[0], PathBuf::from("/home/u/.local/share/hunspell"));
        assert!(dirs.contains(&PathBuf::from("/usr/share/hunspell")) && dirs.contains(&PathBuf::from("/usr/share/myspell/dicts")));
        assert_eq!(dirs.iter().filter(|d| *d == Path::new("/usr/share/hunspell")).count(), 1, "{dirs:?}");
        let dirs = dictionary_dirs(env(&[("DICPATH", "/opt/dicts:/srv/d"), ("XDG_DATA_HOME", "/data"), ("XDG_DATA_DIRS", "/app/share:relative")]), Some(home));
        assert_eq!(&dirs[..4], &[PathBuf::from("/opt/dicts"), PathBuf::from("/srv/d"), PathBuf::from("/data/hunspell"), PathBuf::from("/app/share/hunspell")]);
        assert!(!dirs.iter().any(|d| d.starts_with("relative")));
        // a relative XDG_DATA_HOME doesn't count
        let dirs = dictionary_dirs(env(&[("XDG_DATA_HOME", "data")]), Some(home));
        assert_eq!(dirs[0], PathBuf::from("/home/u/.local/share/hunspell"));
    }

    #[test]
    fn dictionaries_are_found_by_language_tag() {
        let root = temp("find");
        let (sys, user) = (root.join("sys"), root.join("user"));
        for n in ["en_GB", "pt_PT", "de_DE", "de_CH", "sr", "sr-Latn", "fr"] {
            write_dict(&sys, n);
        }
        write_dict(&user, "en_US");
        // hyphenation patterns are .dic files without an .aff
        std::fs::write(sys.join("hyph_it_IT.dic"), "x").unwrap();
        let dirs = vec![user.clone(), sys.clone()];
        let name = |tag: &str| find_dictionary(tag, &dirs).map(|(_, d)| d.file_stem().unwrap().to_string_lossy().to_string());
        assert_eq!(name("en-US").as_deref(), Some("en_US"));
        assert_eq!(find_dictionary("en-US", &dirs).unwrap().0, user.join("en_US.aff"));
        assert_eq!(name("en-GB").as_deref(), Some("en_GB"));
        // another region of the language: en_US first for English, the language's own country
        assert_eq!(name("en-AU").as_deref(), Some("en_US"));
        assert_eq!(name("en-029").as_deref(), Some("en_US"));
        assert_eq!(name("pt-BR").as_deref(), Some("pt_PT"));
        assert_eq!(name("de-AT").as_deref(), Some("de_DE"));
        assert_eq!(name("de-CH").as_deref(), Some("de_CH"));
        assert_eq!(name("fr-CA").as_deref(), Some("fr"));
        assert_eq!(name("sr-Latn-RS").as_deref(), Some("sr-Latn"));
        assert_eq!(name("sr-Cyrl-RS").as_deref(), Some("sr"));
        assert_eq!(name("it-IT"), None);
        assert_eq!(name("ja-JP"), None);
        // the first folder wins
        write_dict(&user, "en_GB");
        assert_eq!(find_dictionary("en-GB", &dirs).unwrap().1, user.join("en_GB.dic"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn dictionaries_in_other_character_sets_are_read() {
        let root = temp("latin1");
        // "Olá" and "ação" in ISO 8859-1
        std::fs::write(root.join("pt_BR.aff"), b"SET ISO8859-1\nTRY a\n").unwrap();
        std::fs::write(root.join("pt_BR.dic"), b"2\nOl\xE1\na\xE7\xE3o\n").unwrap();
        std::fs::write(root.join("ru_RU.aff"), b"SET KOI8-R\n").unwrap();
        std::fs::write(root.join("ru_RU.dic"), b"1\n\xD0\xD2\xC9\xD7\xC5\xD4\n").unwrap();
        let (aff, dic) = find_dictionary("pt-BR", &[root.clone()]).unwrap();
        let d = load_dictionary(&aff, &dic).unwrap();
        assert!(d.check("Olá") && d.check("ação") && !d.check("acao"));
        let (aff, dic) = find_dictionary("ru-RU", &[root.clone()]).unwrap();
        assert!(load_dictionary(&aff, &dic).unwrap().check("привет"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn check_suggest_add_ignore_and_auto_correct() {
        let root = temp("check");
        let (sys, user) = (root.join("sys"), root.join("user"));
        write_dict(&sys, "en_US");
        use_places(vec![sys.clone()], user.clone());
        assert!(supported("en-US") && supported("") && supported("en-GB"));
        assert!(!supported("de-DE"));
        assert!(check("Hallo Welt", "de-DE").is_empty() && suggest("Welt", "de-DE").is_empty());

        let text = "Hello wrld, the the texts don't helo! well-known colours 2024 日本語 http://wrld.example me@wrld.example";
        let errors = check(text, "en-US");
        let word = |e: &SpellError| text.chars().skip(e.start).take(e.len).collect::<String>();
        let flagged: Vec<String> = errors.iter().map(word).collect();
        assert_eq!(flagged, ["wrld", "the", "helo"], "{errors:?}");
        // the repeated word is the second one, to delete
        assert!(errors[1].delete && !errors[0].delete && errors[1].start == 16);
        assert!(suggest("helo", "en-US").iter().any(|s| s == "hello"));
        assert!(suggest("wrld", "en-US").iter().any(|s| s == "world"));

        // Skip All: for this session only
        ignore("wrld", "en-US");
        assert!(!check("wrld", "en-US").iter().any(|e| !e.delete));
        // Add: kept in the user's dictionary of the language
        add("helo", "en-US");
        assert!(check("helo", "en-US").is_empty());
        assert_eq!(std::fs::read_to_string(user.join("en-US/default.dic")).unwrap(), "helo\n");
        // Auto Replace: the word is reported with its replacement, also capitalized
        auto_correct("teh", "the", "en-US");
        let e = check("teh, Teh", "en-US");
        assert_eq!(e.iter().map(|e| e.replacement.clone()).collect::<Vec<_>>(), [Some("the".into()), Some("The".into())]);

        // a dictionary installed while AnyVector runs
        assert!(missing("de-DE"));
        write_dict(&sys, "de_DE");
        assert!(missing("de-DE") && supported("de-DE") && !missing("de-DE"));

        // a new session: the added word and the pair are still there, the skipped word isn't
        use_places(vec![sys.clone()], user.clone());
        assert!(check("helo", "en-US").is_empty());
        assert_eq!(check("teh", "en-US")[0].replacement.as_deref(), Some("the"));
        assert_eq!(check("wrld", "en-US").len(), 1);
        // another language has its own list
        add("Anyvector", "en-GB");
        assert!(user.join("en-GB/default.dic").is_file());
        assert_eq!(check("Anyvector", "en-US").len(), 1);
        let _ = std::fs::remove_dir_all(&root);
    }

    /// The system's own dictionaries, those that are installed (hunspell-en-us, -pt-br, -de-de).
    #[test]
    fn system_dictionaries() {
        let dirs = dictionary_dirs(|n| std::env::var_os(n), dirs::home_dir().as_deref());
        for (tag, good, bad, fix) in [("en-US", "colors", "colour", "color"), ("pt-BR", "ação", "acao", "ação"), ("de-DE", "Straße", "Strasze", "Straße"), ("de-AT", "Häuser", "Heuser", "Häuser")] {
            let Some((aff, dic)) = find_dictionary(tag, &dirs) else { continue };
            let d = load_dictionary(&aff, &dic).unwrap_or_else(|| panic!("{} loads", dic.display()));
            assert!(d.check(good) && !d.check(bad), "{tag}");
            let mut sug = vec![];
            d.suggest(bad, &mut sug);
            assert!(sug.iter().any(|s| s == fix), "{tag}: {sug:?}");
        }
    }

    /// What Windows' spell checker answers for the same texts (asked on Windows 11, English): the
    /// words flagged, "-" before one to delete.
    #[test]
    fn check_stops_where_windows_does() {
        let root = temp("windows");
        write_dict(&root.join("sys"), "en_US");
        use_places(vec![root.join("sys")], root.join("user"));
        for (text, want) in [
            ("The the world", &["-the"] as &[&str]),
            ("the THE world", &["-THE"]),
            ("the\u{a0}the", &["-the"]),
            ("the\nthe world", &[]),
            ("the, the", &[]),
            ("1 1 world", &[]),
            ("the the the", &["-the", "-the"]),
            ("wrldd wrldd", &["wrldd", "wrldd"]),
            ("don't don’t", &["-don’t"]),
            ("dog2 dog2", &["-dog2"]),
            ("the-the world", &[]),
            ("XYZQW", &[]),
            ("XYZQW tests", &["tests"]),
            ("abc123 1st 22nd 3d", &[]),
            ("http://wrld.example wrldd", &["wrldd"]),
            ("me@wrldd.example", &[]),
            ("www.wrldd.com", &[]),
            ("well-knwon", &["knwon"]),
            ("well-known", &[]),
            ("Helo-wrldd", &["Helo-wrldd"]),
            ("wrld's", &["wrld's"]),
            ("wrldd’s", &["wrldd’s"]),
            ("tHe wRLDD", &["tHe", "wRLDD"]),
            ("snake_case_wrd", &["snake_case_wrd"]),
            ("#wrldd $wrldd @wrldd", &[]),
            ("wrldd@ wrldd_", &["wrldd", "wrldd"]),
            ("wrldd.txt U.S.A", &[]),
            ("hello.wrld", &["hello.wrld"]),
            ("wrldd:wrlde", &["wrldd:wrlde"]),
            ("the/wrldd", &["wrldd"]),
            ("wrldd(s)", &["wrldd"]),
            ("wrldd—wrlde", &["wrldd", "wrlde"]),
            ("'wrldd'", &["wrldd"]),
            ("日本語 日本語", &[]),
        ] {
            let got: Vec<String> = check(text, "en-US").iter().map(|e| format!("{}{}", if e.delete { "-" } else { "" }, text.chars().skip(e.start).take(e.len).collect::<String>())).filter(|w| w != "s").collect();
            assert_eq!(got, want, "{text:?}");
        }
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn words_and_packages() {
        let w = |t: &str| words(t).into_iter().map(|(s, l)| t.chars().skip(s).take(l).collect::<String>()).collect::<Vec<_>>();
        assert_eq!(w("it's a well-known 'quote' -- end-"), ["it's", "a", "well-known", "quote", "end"]);
        assert_eq!(w("see www.example.com or a@b.c now. Then #tag and 3.5"), ["see", "or", "now", "Then", "and", "3.5"]);
        assert_eq!(package_for("pt-BR"), "hunspell-pt-br");
        assert_eq!(package_for("en-US"), "hunspell-en-us");
        assert_eq!(package_for("de-AT"), "hunspell-de-at");
        assert_eq!(package_for("fr-FR"), "hunspell-fr");
        assert_eq!(package_for("sr-Latn-RS"), "hunspell-sr");
    }
}
