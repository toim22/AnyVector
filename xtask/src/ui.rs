//! `crates/vector-app/src/ui/data.rs`: every menu, right-click menu, toolbar, toolbox flyout and
//! property bar of the program, item by item, in the reference order.
//!
//! The order, separators, captions (with their `&` access keys), shortcut keys, tooltips and item
//! kinds come from the CorelDRAW 2026 user-interface definition the reference was captured from
//! (`<CorelDRAW>\Draw\UIConfig`, `<CorelDRAW>\Resources`, `<CorelDRAW>\Languages\EN\Data\strings.xml`;
//! the folder defaults to `C:\Program Files\Corel\CorelDRAW Graphics Suite\27` and can be changed
//! with `VECTOR_COREL`). Which items are enabled for each kind of selection comes from the
//! reference's `90-relatorios\matriz_contextos.csv`. Icons are Vector's own drawings, chosen by
//! meaning in `crates/vector-app/assets/ui/icon-map.tsv`.
//!
//! The run also writes `target/ui-report.txt`: each reference menu's height checked against the
//! items Vector shows (a wrong count means an item is shown that the reference hides, or the other
//! way round), and the icons that still have no Vector drawing.

use crate::{reference, root, Result};
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::fmt::Write as _;
use std::path::PathBuf;

/// The separator item of the reference definition.
const SEP: &str = "266435b4-6e53-460f-9fa7-f45be187d400";
/// The main shortcut-key table.
const MAIN_KEYS: &str = "bc175625-191c-4b95-9053-756e5eee26fe";

/// Selection states, in the column order of `matriz_contextos.csv`, plus "no document open".
pub const CONTEXTS: [&str; 19] = [
    "doc-vazio",
    "retangulo",
    "elipse",
    "curva",
    "texto-artistico",
    "texto-paragrafo",
    "bitmap",
    "grupo",
    "selecao-multipla",
    "tabela",
    "tabela-celulas",
    "simbolo",
    "powerclip",
    "linha-de-cota",
    "conector",
    "varias-paginas",
    "rollover",
    "com-efeitos",
    "sem-documento",
];
const NO_DOC_BIT: u32 = 1 << 18;

/// Left out by decision (docs/PLAN-0.1.0.md): Corel's online services, accounts and purchase.
const OMIT: &[&str] = &[
    "ac291790-gf51-d4s1-f23x-dsf9dfb6fgf5", // Purchase (menu bar)
    "67cfe9c0-890c-07b2-4de2-cc788654e6ef", // Debug (hidden menu)
    "09360b3d-3317-4375-a128-ca824873441f", // File > Open from Cloud...
    "95d7454c-db4a-49d3-aab4-b44a7d886a30", // File > Save to Cloud...
];
/// Captions left out by decision wherever they appear (compared without `&` and `...`).
const OMIT_CAPTIONS: &[&str] = &[
    "Sign out of Cloud",
    "Refresh your account",
    "Account Settings",
    "Restore Purchases",
    "CorelDRAW Support",
    "Corel Support",
    "Learn",
    "Video Tutorials",
    "CorelDRAW Community",
    "Share Your Experience",
    "Message Settings",
    "Open from Cloud",
    "Save to Cloud",
    "Purchase",
    "Advanced Account Settings",
];

/// `visible` bindings and the value they have in the reference (hidden when false).
const VISIBILITY: &[(&str, bool)] = &[
    ("IsSupportSharePoint", false),
    ("IsCloudFeaturesVisible", true),
    ("IsDesktopApp", true),
    ("IsGenAIAllowed", true),
    // Asian text layout support, off in the reference install
    ("FarEastRulesGeneralUIEnabled", false),
];

/// `visible` bindings Vector decides at run time: the item is kept with flag 16 and shows only
/// when Vector's state says so (the reference captures show it hidden).
const RUNTIME_VISIBILITY: &[&str] = &[
    // View ▸ Pattern Preview: only while a pattern is open as its own document
    "IsPatternDocument",
];

fn runtime_visible(d: &Defs, r: &ItemRef) -> bool {
    let vis = r.attrs.get("visible").map(|s| s.as_str()).or_else(|| d.attr(&r.guid, "visible")).unwrap_or("");
    RUNTIME_VISIBILITY.iter().any(|n| vis.contains(n))
}

/// Items the reference shows although the definition says otherwise, or hides with no rule in
/// the definition (found with the menu-height check of `target/ui-report.txt`).
const FORCE_HIDDEN: &[&str] = &[
    "96325e30-dd2a-4769-bc79-7b82789dc914", // View > Page Sorter View
    "4d4a7f3c-6767-4d2d-96f3-cf44e6acfdb7", // Text > Line-Breaking Rules... (Asian text)
    "3f3fac62-3939-4ae4-845f-92f3f67d3443", // View > Snap To > Perspective Lines
    "6cea2a4e-530c-8380-4323-bef43800a998", // View > Snap To > Symmetry Lines
    "6db2cdb3-67e4-46a8-9f27-2b0b8b2b7a8e", // Effects > Adjust > Brightness/Contrast/Intensity...
    "f9ea7cda-4d99-4ffd-9958-2ee7af210ac4", // Effects > Adjust > Replace Colors (Legacy)...
    "1ed29b86-73ce-4c4d-8b2d-1a9322ba3de5", // Window > Toolbars > Default Button Size > Custom
    "52bd0964-19b0-4cd3-8c88-bfc1254b2069", // Help > Advanced Account Settings...
    "d0caa528-5aa5-4807-8798-abb9b4bc270d", // Help > Share Your Experience... (web app only)
    "07f91076-f70e-4305-ba1b-54e21e1e58ce", // Help > Refresh your account (web app only)
    "42e27a61-091e-46b1-92a2-55afae3ad2a1", // Help > Sign out of Cloud (signed in only)
    "fbc046c4-da74-4f4a-a12a-a671e7e99bde", // Help > Restore Purchases...
    "c0698e4b-3b68-4b39-bf4c-5aebe6b22952", // right-click > Convert to Empty Masking Frame (in no capture)
];

/// Captions set at run time in the reference (bindings), as Vector shows them.
const CAPTIONS: &[(&str, &str)] = &[
    ("1eaa019d-6dff-4c53-bbd3-5abbb970c53e", "&About AnyVector..."),
];

/// Vector's own additions, asked for by the user: (insert after this item, id, caption, kind, submenu items).
const EXTRAS: &[(&str, &str, &str, &[(&str, &str)])] = &[(
    "fafd28e8-21db-45b7-8e5b-97ec22322e85", // Help > Submit Ideas and Feedback...
    "vector:report-bug",
    "Report a &Bug",
    &[("vector:report-bug-new", "&Report a Bug..."), ("vector:view-bugs", "&View Reported Bugs...")],
)];
const FORCE_VISIBLE: &[&str] = &[];

/// Object > AI Generate: the plain items show with nothing selected and their "Guided by
/// Selection" twins with a selection (the `SelectReferenceEnabled` binding). Vector picks at run
/// time, so a menu shows half of these.
const BY_SELECTION: &[&str] = &[
    "9c0ccd48-7a5f-43c7-aff0-f59be1bb1595",
    "a0776aad-7c8b-499b-8e67-df3801bf1769",
    "4bf3be7a-cdd1-40a7-84ae-448ad787cc07",
    "2daeddaf-49a3-4e1b-ab18-430357decbaa",
    "8ed410c6-5402-4bac-8abe-97c235712914",
    "7e547078-e4e7-4ab5-96da-d1f0c556da6b4",
];

/// Right-click menus, toolbars and other bars emitted besides the menu bar and what it reaches.
const ROOTS: &[(&str, &str)] = &[
    ("Standard", "c2b44f69-6dec-444e-a37e-5dbf7ff43dae"),
    ("Toolbox", "7c905e2a-cb64-4ba1-aff0-c306f392680c"),
    ("Project Timer", "0a1f35d2-4bc0-4ecc-b908-fbbc7b640302"),
    ("Layout", "26278692-c566-47cb-83c4-79b58ce69538"),
    ("Zoom", "3cc76550-c8c1-420f-bccb-c5ea2baf2e65"),
    ("Internet", "57ddc38e-3367-479f-8b2a-4b531ae8cf2a"),
    ("Macros", "bf935726-aeb5-4b71-8c3c-8d17ab7d53aa"),
    ("Parallel Lines", "ca73b05f-26c8-49ee-88e2-56f4f9945bb4"),
    ("Text", "e71e8d2e-9f74-44da-8582-20f977363f5b"),
    ("Transform", "f82791a4-048c-48f1-a059-cb2c3fd0815b"),
    // right-click menus
    ("Graphic Object Selection", "b3646177-602e-405c-9325-fa3606269fd0"),
    ("Artistic Text Selected", "812c5b74-7fa6-46d5-9407-345d211e346b"),
    ("Paragraph Text Selected", "f43488a9-5144-46fb-91d4-74ce0b2efc52"),
    ("Bitmap Object Selection", "1F5DE54C-93EE-4925-AC7F-0043FDF43B3D"),
    ("Group Object Selection", "ef1ceb01-6cdd-4660-a394-e023c7394658"),
    ("Multiple Object Selection", "b67fd2fe-283f-40b5-8372-2ac4da829c2c"),
    ("Symbol Instance Context Menu", "746d20b6-2e4a-42a3-828d-7503029f9ee8"),
    ("Locked Object Selection", "38be0c3e-2472-480a-80c0-c37400ed91c4"),
    ("Artistic Text Editing", "53ac2b4e-95c0-4cf2-abbc-363fbba3606d"),
    ("Paragraph Text Editing", "d1fbf76e-c9e7-4573-b2b4-11f5fe589086"),
    ("Text Character Selection", "4175f4ff-7909-41be-adb5-cc57bfa76f69"),
    ("Node Editing of Curves", "5a3d6dc1-e3ad-49af-be19-04281e1e2a2d"),
    ("Node Editing of Text", "47bcf2ba-eae1-4dc6-9db3-b827cfde0dcb"),
    ("Table Edit", "18cf7aad-b568-4ff0-be22-9911fd417d14"),
    ("Table Edit 2", "3a2169b5-0a11-a69c-4e33-eaa8e5f4fd5b"),
    ("Table Edit 3", "592effa4-1cd3-4a39-ac73-0b9d7e8d73b7"),
    ("Connector Edit", "128f1a37-2a93-0593-4b3e-92d92e16dfe7"),
    ("Connector Edit 2", "7e692eca-e0a9-9696-4774-a33b200929d5"),
    ("Document Desktop", "a7ab62d3-19c0-44fe-ba34-915f326344d2"),
    ("View Page", "00a4f1f0-5084-481c-a6e9-8a89cb88c6e8"),
    ("Ruler Menu", "725bafb4-96e5-4106-a39e-ddb067be3580"),
    ("Ruler Menu In Text Mode", "8dc6e868-1d87-4842-b7f4-3945a11efd1b"),
    ("Palette Menu", "08987c45-0909-4cfc-9e2b-0bd11ecc1ce5"),
    ("Page Tab", "83d62464-9728-49c9-ac8a-68e60cc757c7"),
    ("Command Bar Context Menu", "b298b2cb-c7f0-4266-aaf1-28c6b18592b1"),
    ("Customize Statusbar Menu", "2f38543e-e2b5-4bff-9bb3-ffd61a14ac8d"),
    ("Customize Toolbar Menu", "784cd2ac-8c6b-45e6-86fd-50e7574b8182"),
    ("Customize Flyout Menu", "c652de37-7274-4560-b913-83044fe7ef5c"),
    ("Customize Menu Bar Menu", "e89409c1-e7ca-4a3f-86f7-fe834adaa2aa"),
    ("Effect Selection", "42f20cde-a3ce-4cfd-8e48-e2c6667ca671"),
    ("Blend Control Editing", "8a5f6c77-be4e-4803-a173-98e7cddbfb98"),
    ("Blend Path", "1efc4f62-6356-4371-9aa6-5c8e676e7ca4"),
    ("Blend Start/End", "37492bbf-6589-4fcb-81ca-4faa1adb0bce"),
    ("PowerClip", "5f815cd7-d3f8-453e-8e4c-58d06a2b4f9d"),
    ("Rollover", "21c9c938-094a-45a3-8d15-d3eed5b0e947"),
    ("Paragraph Frame", "38ca4af6-7a29-4ac8-bb83-a5095552ef49"),
    ("Spelling Errors", "721d0a03-b5f4-4cd2-8de2-8c64807049d3"),
    ("Grammar/Double Word Errors", "78a506d5-bc5f-4fd4-9734-63ba8fae385d"),
    ("Variable Outline Node Editing", "880598dc-cc1e-4bc5-ac3c-7210c9512917"),
];
/// The property bar (one bar per mode: tool or selection).
const PROPERTY_BAR: &str = "74e03d83-404c-49f5-824a-fe0fd02ab29a";
const MENU_BAR: &str = "f3016f3c-2847-4557-b61a-a2d05319cf18";
const MENU_MODE_DOC: &str = "76d73481-9076-44c9-821c-52de9408cce2";
const MENU_MODE_NO_DOC: &str = "6d1d9e94-13f3-4517-a976-ba9154671db4";

pub fn corel() -> PathBuf {
    std::env::var_os("VECTOR_COREL").map(PathBuf::from).unwrap_or_else(|| PathBuf::from(r"C:\Program Files\Corel\CorelDRAW Graphics Suite\27"))
}

fn key(g: &str) -> String {
    g.to_ascii_lowercase()
}

struct ItemDef {
    ty: String,
    attrs: HashMap<String, String>,
}

struct ItemRef {
    guid: String,
    attrs: HashMap<String, String>,
}

struct BarDef {
    name: String,
    items: Vec<ItemRef>,
    modes: Vec<(String, Vec<ItemRef>)>,
}

struct Defs {
    items: HashMap<String, ItemDef>,
    bars: HashMap<String, BarDef>,
    strings: HashMap<String, String>,
    caption: HashMap<String, String>,
    tooltip: HashMap<String, String>,
    icons: HashMap<String, String>,
    keys: HashMap<String, Vec<String>>,
}

fn attrs(n: roxmltree::Node) -> HashMap<String, String> {
    n.attributes().map(|a| (a.name().to_string(), a.value().to_string())).collect()
}

fn refs(n: roxmltree::Node) -> Vec<ItemRef> {
    n.children().filter(|c| c.has_tag_name("item")).filter_map(|c| Some(ItemRef { guid: c.attribute("guidRef")?.to_string(), attrs: attrs(c) })).collect()
}

fn csv_map(path: PathBuf) -> Result<HashMap<String, String>> {
    let s = std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(s.lines().filter_map(|l| l.split_once(',')).map(|(a, b)| (key(a), key(b.trim()))).collect())
}

fn format_key(n: roxmltree::Node) -> String {
    let mut s = String::new();
    for (m, name) in [("ctrl", "Ctrl+"), ("alt", "Alt+"), ("shift", "Shift+")] {
        if n.attribute(m) == Some("true") {
            s.push_str(name);
        }
    }
    let k = n.text().unwrap_or_default().trim();
    let name = match k {
        "VK_DELETE" => "Delete".to_string(),
        "VK_BACK" => "Backspace".to_string(),
        "VK_PRIOR" => "PgUp".to_string(),
        "VK_NEXT" => "PgDn".to_string(),
        "VK_HOME" => "Home".to_string(),
        "VK_END" => "End".to_string(),
        "VK_RETURN" => "Enter".to_string(),
        "VK_ESCAPE" => "Esc".to_string(),
        "VK_INSERT" => "Insert".to_string(),
        "VK_SPACE" => "Space".to_string(),
        "VK_TAB" => "Tab".to_string(),
        "VK_LEFT" => "Left".to_string(),
        "VK_RIGHT" => "Right".to_string(),
        "VK_UP" => "Up".to_string(),
        "VK_DOWN" => "Down".to_string(),
        "VK_ADD" => "Num +".to_string(),
        "VK_SUBTRACT" => "Num -".to_string(),
        "VK_MULTIPLY" => "Num *".to_string(),
        "VK_DIVIDE" => "Num /".to_string(),
        k if k.starts_with("VK_F") => k[3..].to_string(),
        k if k.starts_with("VK_NUMPAD") => format!("Num {}", &k[9..]),
        k if k.starts_with("VK_") => k[3..].to_string(),
        k => k.to_uppercase(),
    };
    s + &name
}

fn load() -> Result<Defs> {
    let c = corel();
    let read = |p: PathBuf| std::fs::read_to_string(&p).map_err(|e| format!("{}: {e} (set VECTOR_COREL to the CorelDRAW folder)", p.display()));
    let ui_s = read(c.join(r"Draw\UIConfig\DrawUI.xml"))?;
    let items_s = read(c.join(r"Draw\UIConfig\DrawUI.items.xml"))?;
    let strings_s = read(c.join(r"Languages\EN\Data\strings.xml"))?;
    let icons_s = read(c.join(r"Resources\icons.map.xml"))?;
    let opt = || roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
    let ui = roxmltree::Document::parse_with_options(&ui_s, opt())?;
    let it = roxmltree::Document::parse_with_options(&items_s, opt())?;
    let st = roxmltree::Document::parse_with_options(&strings_s, opt())?;
    let ic = roxmltree::Document::parse_with_options(&icons_s, opt())?;

    let mut items = HashMap::new();
    for doc in [&it, &ui] {
        for n in doc.descendants().filter(|n| n.has_tag_name("itemData")) {
            let Some(g) = n.attribute("guid") else { continue };
            items.entry(key(g)).or_insert_with(|| ItemDef { ty: n.attribute("type").unwrap_or_default().to_string(), attrs: attrs(n) });
        }
    }
    let mut bars = HashMap::new();
    for n in ui.descendants().filter(|n| n.has_tag_name("commandBarData")) {
        let Some(g) = n.attribute("guid") else { continue };
        let body = n.children().find(|c| c.is_element() && ["menu", "toolbar", "menubar"].contains(&c.tag_name().name()));
        let (items, modes) = match body {
            Some(b) => (refs(b), b.children().filter(|m| m.has_tag_name("modeData")).map(|m| (m.attribute("guid").unwrap_or_default().to_string(), refs(m))).collect()),
            None => (vec![], vec![]),
        };
        bars.insert(key(g), BarDef { name: n.attribute("nonLocalizableName").unwrap_or_default().to_string(), items, modes });
    }
    let strings = st.descendants().filter(|n| n.has_tag_name("string")).filter_map(|n| Some((key(n.attribute("guid")?), n.text().unwrap_or_default().to_string()))).collect();
    let icons = ic.descendants().filter(|n| n.has_tag_name("map")).filter_map(|n| Some((key(n.attribute("guid")?), n.text()?.to_string()))).collect();
    let mut keys: HashMap<String, Vec<String>> = HashMap::new();
    for t in ui.descendants().filter(|n| n.has_tag_name("table") && n.attribute("tableID").map(key).as_deref() == Some(MAIN_KEYS)) {
        for ks in t.children().filter(|n| n.has_tag_name("keySequence")) {
            let parts: Vec<String> = ks.children().filter(|k| k.has_tag_name("key")).map(format_key).collect();
            if let (Some(r), false) = (ks.attribute("itemRef"), parts.is_empty()) {
                keys.entry(key(r)).or_default().push(parts.join(", "));
            }
        }
    }
    Ok(Defs {
        items,
        bars,
        strings,
        caption: csv_map(c.join(r"Resources\caption.csv"))?,
        tooltip: csv_map(c.join(r"Resources\tooltipHeader.csv"))?,
        icons,
        keys,
    })
}

impl Defs {
    fn item(&self, g: &str) -> Option<&ItemDef> {
        self.items.get(&key(g))
    }
    fn attr(&self, g: &str, a: &str) -> Option<&str> {
        self.item(g).and_then(|d| d.attrs.get(a)).map(|s| s.as_str())
    }
    fn text_of(&self, map: &HashMap<String, String>, g: &str, depth: u8) -> Option<String> {
        if let Some(s) = map.get(&key(g)).and_then(|s| self.strings.get(s)) {
            return Some(s.clone());
        }
        let d = self.item(g)?;
        if depth < 4 {
            for r in ["captionRef", "tooltipRef"] {
                if let Some(t) = d.attrs.get(r).and_then(|r| self.text_of(map, r, depth + 1)) {
                    return Some(t);
                }
            }
        }
        None
    }
    fn caption(&self, g: &str) -> String {
        if let Some((_, c)) = CAPTIONS.iter().find(|(i, _)| i.eq_ignore_ascii_case(g)) {
            return c.to_string();
        }
        let t = self.text_of(&self.caption, g, 0).or_else(|| self.attr(g, "caption").filter(|c| !c.starts_with('*')).map(str::to_string)).unwrap_or_default();
        brand(&t)
    }
    fn tooltip(&self, g: &str) -> String {
        brand(&self.text_of(&self.tooltip, g, 0).unwrap_or_default())
    }
    /// The reference icon file of an item (directly, or through `icon="guid://..."`).
    fn icon(&self, g: &str) -> Option<String> {
        if let Some(i) = self.icons.get(&key(g)) {
            return Some(i.clone());
        }
        for a in ["icon", "toolBarIcon", "leftLabelIcon"] {
            if let Some(r) = self.attr(g, a).and_then(|v| v.strip_prefix("guid://")) {
                if let Some(i) = self.icons.get(&key(r)) {
                    return Some(i.clone());
                }
            }
        }
        None
    }
    fn sub(&self, g: &str) -> Option<String> {
        ["flyoutBarRef", "dropDownRef", "prefillBarGuid"].iter().find_map(|a| self.attr(g, a)).map(key).filter(|b| self.bars.contains_key(b))
    }
}

/// "CorelDRAW" in a caption becomes "AnyVector".
fn brand(s: &str) -> String {
    s.replace("CorelDRAW", "AnyVector")
}

fn plain(s: &str) -> String {
    let mut out = String::new();
    let mut it = s.chars().peekable();
    while let Some(c) = it.next() {
        if c == '&' {
            if it.peek() == Some(&'&') {
                out.push('&');
                it.next();
            }
        } else {
            out.push(c);
        }
    }
    out.trim().to_string()
}

fn omitted(d: &Defs, g: &str) -> bool {
    if OMIT.iter().any(|o| o.eq_ignore_ascii_case(g)) {
        return true;
    }
    let c = plain(&d.caption(g));
    let c = c.trim_end_matches("...").trim_end_matches('…').trim();
    OMIT_CAPTIONS.iter().any(|o| o.eq_ignore_ascii_case(c))
}

fn hidden(d: &Defs, r: &ItemRef) -> bool {
    if FORCE_VISIBLE.iter().any(|o| o.eq_ignore_ascii_case(&r.guid)) {
        return false;
    }
    if FORCE_HIDDEN.iter().any(|o| o.eq_ignore_ascii_case(&r.guid)) {
        return true;
    }
    let vis = r.attrs.get("visible").map(|s| s.as_str()).or_else(|| d.attr(&r.guid, "visible")).unwrap_or("");
    if vis == "false" {
        return true;
    }
    if runtime_visible(d, r) {
        return false;
    }
    for (name, value) in VISIBILITY {
        if vis.contains(name) {
            // a `*Not(...)` around the binding flips it
            let negated = inside_not(vis, name);
            return value == &negated;
        }
    }
    false
}

/// Whether `name` sits inside a `*Not(...)` of the binding expression `vis`.
fn inside_not(vis: &str, name: &str) -> bool {
    let Some(at) = vis.find(name) else { return false };
    let mut from = 0;
    while let Some(p) = vis[from..].find("*Not(") {
        let open = from + p + "*Not(".len();
        let mut depth = 1;
        let mut end = vis.len();
        for (i, c) in vis[open..].char_indices() {
            match c {
                '(' => depth += 1,
                ')' => {
                    depth -= 1;
                    if depth == 0 {
                        end = open + i;
                        break;
                    }
                }
                _ => {}
            }
        }
        if (open..end).contains(&at) {
            return true;
        }
        from = open;
    }
    false
}

/// What kind of control an item is, as the name of the `Kind` variant in the app.
fn kind(ty: &str) -> &'static str {
    match ty {
        "button" => "Button",
        "checkButton" => "Check",
        "groupedRadioButton" => "Radio",
        "flyout" => "Flyout",
        "buttonFlyoutPair" => "ButtonFlyout",
        "toolFlyout" => "ToolFlyout",
        "placeHolder" => "Placeholder",
        "comboBoxEx" | "comboBoxOrCellComboBox" => "Combo",
        "cellCombo" => "CellCombo",
        "spinner" => "Spin",
        "stackedSpinCtrl" => "StackedSpin",
        "popupSlider" => "Slider",
        "dropDownDlgBtn" => "DropDown",
        "groupButtonBegin" => "GroupBegin",
        "groupButtonEnd" => "GroupEnd",
        "anchors" => "Anchors",
        "editBox" => "Edit",
        "statusText" => "Label",
        "spacer" => "Spacer",
        "separator" => "Separator",
        "paletteGridEx" => "Palette",
        _ => "Other",
    }
}

/// Corel's enable expression says the item needs an open document.
fn needs_document(expr: &str) -> bool {
    ["DocumentAvailable", "IsDocumentReady", "WDocCommandsDS", "SelectionInfoDatasource", "DrawingInfoDS", "ToolboxEnable", "WViewDataSource", "WZoomPanPresentationDS", "ExportDialogDS", "PDFComponentDS", "DocInfoDataSource", "ColorViewDS", "DocumentPrefsDS", "WDupOffsetDataSource", "WBringInFocusDS", "WDataMergeDS"]
        .iter()
        .any(|k| expr.contains(k))
}

struct Reference {
    /// menu path -> enabled per context (bits 0..17)
    matrix: HashMap<String, u32>,
    /// (parent path, guid) -> path in the reference
    paths: HashMap<(String, String), String>,
    /// guid -> enabled per context, from any menu path of the same command
    by_guid: HashMap<String, u32>,
}

fn load_reference() -> Result<Reference> {
    let r = reference();
    let m = std::fs::read_to_string(r.join(r"90-relatorios\matriz_contextos.csv"))?;
    let mut matrix = HashMap::new();
    for l in m.lines().skip(1) {
        let f: Vec<&str> = l.trim_start_matches('\u{feff}').split(';').collect();
        let mut bits = 0u32;
        for (i, v) in f.iter().skip(1).enumerate() {
            if v.trim() == "sim" {
                bits |= 1 << i;
            }
        }
        matrix.insert(f[0].to_string(), bits);
    }
    let inv = std::fs::read_to_string(r.join(r"01-inventario\inventory.csv"))?;
    let mut paths = HashMap::new();
    for l in inv.lines().skip(1) {
        let f = split_csv(l);
        if f.len() > 3 && f[0] == "menu_item" {
            let path = f[1].clone();
            let parent = path.rsplit_once(" > ").map(|(p, _)| p.to_string()).unwrap_or_default();
            paths.insert((parent, key(&f[3])), path);
        }
    }
    let mut by_guid: HashMap<String, u32> = HashMap::new();
    for ((_, guid), path) in &paths {
        if let Some(bits) = matrix.get(path) {
            *by_guid.entry(guid.clone()).or_default() |= *bits;
        }
    }
    Ok(Reference { matrix, paths, by_guid })
}

fn split_csv(l: &str) -> Vec<String> {
    let mut out = vec![];
    let mut cur = String::new();
    let mut q = false;
    for c in l.chars() {
        match c {
            '"' => q = !q,
            ',' if !q => out.push(std::mem::take(&mut cur)),
            c => cur.push(c),
        }
    }
    out.push(cur);
    out
}

struct OutItem {
    id: String,
    text: String,
    tip: String,
    kind: &'static str,
    keys: Vec<String>,
    sub: Option<String>,
    icon: String,
    corel_icon: Option<String>,
    menu_icon: bool,
    on: u32,
    face: &'static str,
    /// placeholders: 1 = hide the disabled items it brings in, 2 = a separator before them;
    /// 4 = the enabled state is not known from the reference
    flags: u8,
}

struct OutBar {
    id: String,
    name: String,
    items: Vec<OutItem>,
}

struct Gen<'a> {
    d: &'a Defs,
    r: &'a Reference,
    icon_map: HashMap<String, String>,
    bars: Vec<OutBar>,
    index: HashMap<String, usize>,
    queue: VecDeque<(String, String, String)>, // (bar key, display name, reference path)
}

/// The short name of a reference icon file, the key of `icon-map.tsv`.
fn icon_stem(path: &str) -> String {
    let f = path.rsplit('/').next().unwrap_or(path);
    f.trim_end_matches(".png").trim_end_matches(".ico").trim_end_matches(".svg").to_string()
}

impl Gen<'_> {
    fn queue_bar(&mut self, bar: &str, name: &str, path: &str) {
        let k = key(bar);
        if !self.index.contains_key(&k) && !self.queue.iter().any(|(b, _, _)| *b == k) {
            self.queue.push_back((k, name.to_string(), path.to_string()));
        }
    }

    fn emit(&mut self, id: String, name: String, refs: &[ItemRef], path: &str) {
        let idx = self.bars.len();
        self.index.insert(key(&id), idx);
        self.bars.push(OutBar { id: id.clone(), name, items: vec![] });
        let mut items = vec![];
        for r in refs {
            if key(&r.guid) == SEP {
                items.push(OutItem { id: String::new(), text: String::new(), tip: String::new(), kind: "Separator", keys: vec![], sub: None, icon: String::new(), corel_icon: None, menu_icon: false, on: u32::MAX, face: "Default", flags: 0 });
                continue;
            }
            if self.d.item(&r.guid).is_none() || hidden(self.d, r) {
                continue;
            }
            if omitted(self.d, &r.guid) {
                items.push(OutItem { id: String::new(), text: String::new(), tip: String::new(), kind: "Omitted", keys: vec![], sub: None, icon: String::new(), corel_icon: None, menu_icon: false, on: 0, face: "Default", flags: 0 });
                continue;
            }
            let ty = self.d.item(&r.guid).map(|i| i.ty.as_str()).unwrap_or("");
            let text = self.d.caption(&r.guid);
            let item_path = if path.is_empty() { plain(&text) } else { format!("{path} > {}", plain(&text)) };
            let sub = self.d.sub(&r.guid);
            if let Some(s) = &sub {
                self.queue_bar(s, &plain(&text), &item_path);
            }
            // enabled per selection state: by path in the matrix, or by guid through the inventory
            let ref_path = self.r.paths.get(&(path.to_string(), key(&r.guid))).cloned().unwrap_or(item_path.clone());
            let known = self.r.matrix.get(&ref_path).or_else(|| self.r.by_guid.get(&key(&r.guid))).copied();
            let mut on = known.unwrap_or((1 << 18) - 1);
            let enable = self.d.attr(&r.guid, "enable").unwrap_or("");
            if !needs_document(enable) {
                on |= NO_DOC_BIT;
            }
            let corel_icon = self.d.icon(&r.guid);
            // an item can be mapped by its own id ("id:" + the first 8 characters of its GUID), for
            // items whose reference icon has no name of its own
            let by_id = self.icon_map.get(&format!("id:{}", key(&r.guid).get(..8).unwrap_or("")));
            let icon = by_id
                .cloned()
                .or_else(|| corel_icon.as_deref().map(icon_stem).and_then(|s| self.icon_map.get(&s).cloned()))
                .unwrap_or_default();
            let face = match r.attrs.get("itemFace").or_else(|| self.d.item(&r.guid).and_then(|i| i.attrs.get("itemFace"))).map(|s| s.as_str()) {
                Some("textOnly") => "TextOnly",
                Some("textRightOfImage") => "TextRight",
                Some("imageOnly") => "ImageOnly",
                _ => "Default",
            };
            items.push(OutItem {
                id: r.guid.clone(),
                tip: self.d.tooltip(&r.guid),
                text,
                kind: kind(ty),
                keys: self.d.keys.get(&key(&r.guid)).cloned().unwrap_or_default(),
                sub,
                icon,
                menu_icon: corel_icon.is_some() && self.d.attr(&r.guid, "noBmpOnMenu") != Some("true"),
                corel_icon,
                on,
                face,
                // 1 = hide the disabled items it brings in, 2 = a separator before them,
                // 4 = no reference capture shows when the item is enabled
                flags: (self.d.attr(&r.guid, "hideDisabledPrefillItems") == Some("true")) as u8 | (((self.d.attr(&r.guid, "addSeparator") == Some("true")) as u8) << 1) | ((known.is_none() as u8) << 2)
                    | (((r.attrs.get("excluded").map(|s| s.as_str()) == Some("true")) as u8) << 3)
                    // 16 = shown only when Vector's state says so (hidden in the reference captures)
                    | ((runtime_visible(self.d, r) as u8) << 4),
            });
            if let Some((_, id, text, subs)) = EXTRAS.iter().find(|(after, ..)| after.eq_ignore_ascii_case(&r.guid)) {
                let sub_id = format!("{id}:menu");
                let sub_idx = self.bars.len();
                self.index.insert(key(&sub_id), sub_idx);
                let sub_items = subs.iter().map(|(i, t)| OutItem { id: i.to_string(), text: t.to_string(), tip: String::new(), kind: "Button", keys: vec![], sub: None, icon: String::new(), corel_icon: None, menu_icon: false, on: u32::MAX, face: "Default", flags: 0 }).collect();
                self.bars.push(OutBar { id: sub_id.clone(), name: plain(text), items: sub_items });
                items.push(OutItem { id: id.to_string(), text: text.to_string(), tip: String::new(), kind: "Flyout", keys: vec![], sub: Some(key(&sub_id)), icon: String::new(), corel_icon: None, menu_icon: false, on: u32::MAX, face: "Default", flags: 0 });
            }
        }
        self.bars[idx].items = items;
    }

    fn drain(&mut self) {
        while let Some((k, name, path)) = self.queue.pop_front() {
            if self.index.contains_key(&k) {
                continue;
            }
            let Some(b) = self.d.bars.get(&k) else { continue };
            let refs: Vec<ItemRef> = if b.items.is_empty() { b.modes.first().map(|(_, r)| r.iter().map(|x| ItemRef { guid: x.guid.clone(), attrs: x.attrs.clone() }).collect()).unwrap_or_default() } else { b.items.iter().map(|x| ItemRef { guid: x.guid.clone(), attrs: x.attrs.clone() }).collect() };
            let name = if name.is_empty() { b.name.clone() } else { name };
            let id = k.clone();
            self.emit(id, name, &refs, &path);
        }
    }
}

fn rust_str(s: &str) -> String {
    format!("{s:?}")
}

pub fn run() -> Result<()> {
    let d = load()?;
    let r = load_reference()?;
    let map_path = root().join("crates/vector-app/assets/ui/icon-map.tsv");
    let icon_map: HashMap<String, String> = std::fs::read_to_string(&map_path)
        .unwrap_or_default()
        .lines()
        .filter(|l| !l.starts_with('#'))
        .filter_map(|l| {
            let mut f = l.split('\t');
            Some((f.next()?.trim().to_string(), f.next()?.trim().to_string()))
        })
        .filter(|(_, v)| !v.is_empty())
        .collect();
    let mut g = Gen { d: &d, r: &r, icon_map, bars: vec![], index: HashMap::new(), queue: VecDeque::new() };

    // the menu bar: with a document, and the reduced one without
    let mb = d.bars.get(MENU_BAR).ok_or("menu bar not found")?;
    for (mode, name) in [(MENU_MODE_DOC, "Menu Bar"), (MENU_MODE_NO_DOC, "Menu Bar (no document)")] {
        let refs = &mb.modes.iter().find(|(m, _)| key(m) == mode).ok_or("menu bar mode not found")?.1;
        let refs: Vec<ItemRef> = refs.iter().map(|x| ItemRef { guid: x.guid.clone(), attrs: x.attrs.clone() }).collect();
        g.emit(mode.to_string(), name.to_string(), &refs, "");
        g.drain();
    }
    // property bar modes
    let pb = d.bars.get(PROPERTY_BAR).ok_or("property bar not found")?;
    let mut seen_modes = HashSet::new();
    for (mode, refs) in &pb.modes {
        if !seen_modes.insert(key(mode)) {
            continue;
        }
        let name = plain(&d.caption(mode));
        let name = if name.is_empty() { format!("Property Bar {}", &mode[..8]) } else { format!("Property Bar: {name}") };
        let refs: Vec<ItemRef> = refs.iter().map(|x| ItemRef { guid: x.guid.clone(), attrs: x.attrs.clone() }).collect();
        g.emit(mode.clone(), name, &refs, "");
        g.drain();
    }
    for (name, bar) in ROOTS {
        g.queue_bar(bar, name, "");
        g.drain();
    }

    // ------------------------------------------------------------------ data.rs
    let mut s = String::new();
    s.push_str("// @generated by `cargo run -p xtask -- ui` (xtask/src/ui.rs). Do not edit by hand.\n");
    s.push_str("//! Every menu, right-click menu, toolbar, toolbox flyout and property bar, item by item, in the\n");
    s.push_str("//! reference order. `on` has one bit per selection state of `super::CONTEXTS`.\n\n");
    s.push_str("use super::{Bar, Face::*, Item, Kind::*};\n\n");
    s.push_str("const N: u16 = super::NO_SUB;\n\n");
    writeln!(s, "pub static BARS: [Bar; {}] = [", g.bars.len())?;
    for b in &g.bars {
        writeln!(s, "    Bar {{ id: {}, name: {}, items: &[", rust_str(&b.id), rust_str(&b.name))?;
        for it in b.items.iter().filter(|i| i.kind != "Omitted") {
            let sub = it.sub.as_ref().and_then(|x| g.index.get(x)).map(|i| i.to_string()).unwrap_or_else(|| "N".into());
            let keys: Vec<String> = it.keys.iter().map(|k| rust_str(k)).collect();
            let tip = if plain(&it.tip) == plain(&it.text) { String::new() } else { it.tip.clone() };
            writeln!(
                s,
                "        Item {{ id: {}, text: {}, tip: {}, kind: {}, keys: &[{}], sub: {sub}, icon: {}, menu_icon: {}, on: {:#x}, face: {}, flags: {} }},",
                rust_str(&it.id),
                rust_str(&it.text),
                rust_str(&tip),
                it.kind,
                keys.join(", "),
                rust_str(&it.icon),
                it.menu_icon,
                it.on,
                it.face,
                it.flags
            )?;
        }
        s.push_str("    ] },\n");
    }
    s.push_str("];\n");
    let out = root().join("crates/vector-app/src/ui/data.rs");
    std::fs::create_dir_all(out.parent().unwrap())?;
    std::fs::write(&out, s)?;
    let n_items: usize = g.bars.iter().map(|b| b.items.iter().filter(|i| i.kind != "Omitted").count()).sum();
    println!("wrote {} ({} bars, {} items)", out.display(), g.bars.len(), n_items);

    // ------------------------------------------------------------------ report
    let mut rep = String::new();
    report_heights(&g, &mut rep)?;
    report_icons(&g, &mut rep)?;
    let rp = root().join("target/ui-report.txt");
    std::fs::create_dir_all(rp.parent().unwrap())?;
    std::fs::write(&rp, &rep)?;
    println!("wrote {}", rp.display());
    Ok(())
}

/// Checks each captured reference menu's height against the items Vector shows in it:
/// 22 px per item, 7 px per separator, 3 px border top and bottom.
fn report_heights(g: &Gen, rep: &mut String) -> Result<()> {
    writeln!(rep, "# Menu heights (reference rectangle vs. items shown)\n")?;
    let doc = g.index[&key(MENU_MODE_DOC)];
    let nodoc = g.index[&key(MENU_MODE_NO_DOC)];
    let (mut ok, mut bad) = (0, 0);
    for (i, ctx) in CONTEXTS.iter().enumerate() {
        let p = reference().join(format!(r"90-relatorios\menus_{ctx}.json"));
        let Ok(text) = std::fs::read_to_string(&p) else { continue };
        let v: serde_json::Value = serde_json::from_str(&text)?;
        for e in v.as_array().into_iter().flatten() {
            let path = e["path"].as_str().unwrap_or_default();
            let Some(rect) = e["rects"].as_array().and_then(|r| r.last()).and_then(|r| r.as_array()) else { continue };
            let h = rect[3].as_i64().unwrap_or(0) - rect[1].as_i64().unwrap_or(0);
            // walk the path through the emitted bars
            let mut bar = if *ctx == "sem-documento" { nodoc } else { doc };
            let mut found = true;
            for part in path.split(" > ") {
                match g.bars[bar].items.iter().find(|it| plain(&it.text) == part && it.sub.is_some()) {
                    Some(it) => bar = g.index[it.sub.as_ref().unwrap()],
                    None => {
                        found = false;
                        break;
                    }
                }
            }
            if !found {
                if !path.contains("item dinamico") {
                    writeln!(rep, "  ?? {ctx}: {path} (not found)")?;
                }
                continue;
            }
            let (n, s) = visible_counts(&g.bars[bar].items);
            let n = n - g.bars[bar].items.iter().filter(|i| BY_SELECTION.iter().any(|b| b.eq_ignore_ascii_case(&i.id))).count() as i64 / 2;
            let want = 6 + 22 * n + 7 * s;
            if g.bars[bar].items.iter().any(|i| i.kind == "Placeholder") {
                writeln!(rep, "  ~~ {ctx}: {path}: reference {h} px, Vector {want} px before its run-time list")?;
                continue;
            }
            if (want - h).abs() <= 2 {
                ok += 1;
            } else {
                bad += 1;
                let diff = (h - want) as f32;
                writeln!(rep, "  !! {ctx}: {path}: reference {h} px, Vector {want} px ({} items, {} separators; {:+.1} rows)", n, s, diff / 22.0)?;
            }
            let _ = i;
        }
    }
    writeln!(rep, "\n{ok} menus match, {bad} differ\n")?;
    println!("menu heights: {ok} match, {bad} differ");
    report_context_menus(g, rep)?;
    Ok(())
}

/// The right-click menus of the reference (`07-menus-de-contexto`) next to the bars whose item
/// count gives the same height.
fn report_context_menus(g: &Gen, rep: &mut String) -> Result<()> {
    writeln!(rep, "# Right-click menus (reference height, bars of the same height)\n")?;
    let p = reference().join(r"90-relatorios\menus_contexto.json");
    let Ok(text) = std::fs::read_to_string(&p) else { return Ok(()) };
    let v: serde_json::Value = serde_json::from_str(&text)?;
    for e in v.as_array().into_iter().flatten() {
        let path = e["path"].as_str().unwrap_or_default();
        let Some(rect) = e["rects"].as_array().and_then(|r| r.last()).and_then(|r| r.as_array()) else { continue };
        let h = rect[3].as_i64().unwrap_or(0) - rect[1].as_i64().unwrap_or(0);
        let rows = (h - 6) as f32 / 22.0;
        let same: Vec<String> = g
            .bars
            .iter()
            .filter(|b| !b.name.starts_with("Property Bar") && !b.items.is_empty())
            .filter(|b| {
                let (n, s) = visible_counts(&b.items);
                (6 + 22 * n + 7 * s - h).abs() <= 2
            })
            .map(|b| b.name.clone())
            .take(8)
            .collect();
        writeln!(rep, "  {path}: {h} px (~{rows:.1} rows) -> {}", same.join(" | "))?;
    }
    writeln!(rep)?;
    Ok(())
}

/// Items and separators a menu shows (separators collapse at the ends and when doubled).
fn visible_counts(items: &[OutItem]) -> (i64, i64) {
    let mut n = 0;
    let mut s = 0;
    let mut pending = false;
    for it in items.iter().filter(|i| !i.id.starts_with("vector:") && i.flags & 16 == 0) {
        if it.kind == "Separator" {
            pending = n > 0;
            continue;
        }
        if pending {
            s += 1;
            pending = false;
        }
        n += 1;
    }
    (n, s)
}

fn report_icons(g: &Gen, rep: &mut String) -> Result<()> {
    let mut missing: BTreeMap<String, (usize, Vec<String>)> = BTreeMap::new();
    let mut total = HashSet::new();
    for b in &g.bars {
        for it in &b.items {
            let Some(ci) = &it.corel_icon else { continue };
            let stem = icon_stem(ci);
            total.insert(stem.clone());
            if it.icon.is_empty() {
                let e = missing.entry(stem).or_default();
                e.0 += 1;
                let label = format!("{} › {}", b.name, plain(if it.text.is_empty() { &it.tip } else { &it.text }));
                if e.1.len() < 3 && !e.1.contains(&label) {
                    e.1.push(label);
                }
            }
        }
    }
    // every use of every reference icon, for drawing and mapping them
    let mut tsv = String::from("stem\tvector\tbar\titem\tlabel\tkind\tmenu_icon\tcorel_path\n");
    for b in &g.bars {
        for it in &b.items {
            let Some(ci) = &it.corel_icon else { continue };
            let label = plain(if it.text.is_empty() { &it.tip } else { &it.text });
            writeln!(tsv, "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}", icon_stem(ci), it.icon, b.name, it.id, label, it.kind, it.menu_icon, ci)?;
        }
    }
    std::fs::write(root().join("target/ui-icons.tsv"), tsv)?;
    writeln!(rep, "# Icons without a Vector drawing: {} of {}\n", missing.len(), total.len())?;
    for (stem, (n, labels)) in &missing {
        writeln!(rep, "{stem}\t{n}\t{}", labels.join(" | "))?;
    }
    println!("icons: {} of {} still need a Vector drawing", missing.len(), total.len());
    Ok(())
}
