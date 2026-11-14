//! Application commands (menus, toolbars, shortcuts and scripts all go through `Cmd`).

use crate::dockers::DockerId;
use crate::tools::ToolId;
use vector_core::ops::{Align, Distribute, Order, Shaping};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash, serde::Serialize, serde::Deserialize)]
pub enum OptionsPage {
    Document,
    App,
    Customization,
    Tools,
    Global,
    Workspaces,
}

#[derive(Clone, Copy, PartialEq, Debug, serde::Serialize, serde::Deserialize)]
pub enum Cmd {
    // File
    New,
    NewFromTemplate,
    QuickAiGenerate,
    Open,
    OpenFromCloud,
    OpenRecent(usize),
    /// Object ▸ Object Styles.
    NewStyleSetFrom,
    NewStyleFrom(u8),
    ApplyObjectStyle(usize),
    EditObjectStyle(usize),
    ApplyToStyle,
    RevertToStyle,
    BreakStyleLink,
    ClearRecent,
    Close,
    CloseAll,
    Save,
    SaveAs,
    SaveToCloud,
    SaveAsTemplate,
    Revert,
    SelectTwain,
    SelectWia,
    Acquire,
    Import,
    Export,
    ExportForWeb,
    ExportForOffice,
    ExportForWordPress,
    ExportDockerOpen,
    ExportDockerAddSelection,
    ExportDockerAddPage,
    ExportDockerAddAllPages,
    SendToMail,
    SendToDesktop,
    SendToDocuments,
    PublishPdf,
    PrintMergeCreate,
    PrintMergeEdit,
    PrintMergePerform,
    /// Print Merge ▸ Create Merged Document.
    PrintMergeNewDoc,
    Print,
    PrintPreview,
    DocumentProperties,
    Exit,
    // Edit
    Undo,
    Redo,
    UndoTo(usize),
    RedoTo(usize),
    Repeat,
    Cut,
    Copy,
    Paste,
    PasteSpecial,
    PasteInView,
    BringIntoFocus,
    ExitFocusMode,
    Delete,
    Duplicate,
    Clone,
    StepAndRepeat,
    CopyPropertiesFrom,
    SelectAll,
    SelectAllText,
    SelectAllGuides,
    SelectAllNodes,
    Deselect,
    FindReplace,
    // View
    ViewMode(crate::docview::ViewMode),
    FullScreenPreview,
    PageSorterView,
    ToggleRulers,
    ToggleGrid,
    TogglePixelGrid,
    ToggleGuidelines,
    TogglePageBorder,
    ToggleBleed,
    SnapOff,
    SnapToGrid,
    SnapToGuides,
    SnapToObjects,
    SnapToPage,
    SnapToPixels,
    SnapToBaseline,
    PreviewSelectedOnly,
    ToggleMultipageView,
    ToggleProofColors,
    ToggleSimulateOverprints,
    ToggleRasterizeEffects,
    TogglePrintableArea,
    ToggleBaselineGrid,
    ToggleAlignmentGuides,
    ToggleDynamicGuides,
    ToggleDocNavigator,
    ToggleScrollbars,
    ZoomIn,
    ZoomOut,
    ZoomToSelected,
    ZoomToAll,
    ZoomToPage,
    ZoomToWidth,
    ZoomToHeight,
    ZoomActual,
    ZoomSet(f64),
    // Layout
    InsertPage,
    /// Insert one page after / before the current page (page tab right-click menu).
    InsertPageAfter,
    InsertPageBefore,
    DuplicatePage,
    RenamePage,
    DeletePage,
    /// The page tab's Delete Page: deletes the page without a dialog.
    DeletePageNow,
    GoToPage,
    NextPage,
    PrevPage,
    SwitchOrientation,
    PageSetup,
    PageLayout,
    PageNumberSettings,
    /// 0 active layer, 1 all pages, 2 odd pages, 3 even pages.
    InsertPageNumber(u8),
    BorderGrommet,
    CenterToPage,
    AlignPixelGrid,
    OrderInFrontOf,
    OrderBehind,
    ShowObjects,
    OverprintFill,
    OverprintOutline,
    OverprintBitmap,
    ObjectHinting,
    PowerClipEmptyFrame,
    PowerClipLock,
    /// 0 Center, 1 Fit Proportionally, 2 Fill Proportionally, 3 Stretch to Fill.
    PowerClipPlace(u8),
    CopyPowerClipFrom,
    /// Copy Effect / Clone Effect: 0 Perspective, 1 Envelope, 2 Blend, 3 Extrude, 4 Contour,
    /// 5 Lens, 6 Shadow, 7 Distortion.
    CloneEffectFrom(u8),
    PageBackground,
    Landscape(bool),
    /// A property bar option button (the first 8 hex digits of its reference id), run by
    /// `crate::propbar::press`.
    BarOption(u32),
    /// Ruler right-click menu: the Grid, Rulers and Guidelines pages of Options.
    GridSetup,
    /// Options ▸ Customization ▸ Command Bars (a command bar's Properties…).
    CommandBarsSetup,
    /// Options ▸ Customization ▸ Commands (a toolbar item's Properties…).
    CommandsSetup,
    RulerSetup,
    GuidelinesSetup,
    // Object
    Group,
    Ungroup,
    UngroupAll,
    Combine,
    BreakApart,
    Lock,
    Unlock,
    UnlockAll,
    HideObject,
    ShowAllObjects,
    Order(Order),
    ReverseOrder,
    Align(Align),
    AlignTo(Align),
    Distribute(Distribute),
    Shaping(Shaping),
    ConvertToCurves,
    /// A Shape tool node command (Add, Delete, To Line, …) on the selected nodes.
    Node(crate::tools::NodeOp),
    OutlineToObject,
    /// Object ▸ Create Cut Contour: a CutContour hairline around the selection (print-and-cut).
    CreateCutContour,
    JoinCurves,
    PowerClipInside,
    PowerClipExtract,
    PowerClipEdit,
    PowerClipFinish,
    MirrorH,
    MirrorV,
    Rotate(f64),
    ClearTransformations,
    NewSymbol,
    RevertSymbol,
    ObjectProperties,
    ClearEffect,
    CopyEffectFrom(u8),
    BreakBlend,
    AddPerspective,
    ClearPerspective,
    BevelEffect,
    LensEffect,
    // Bitmaps
    ConvertToBitmap,
    QuickTrace,
    /// Bitmaps ▸ Outline Trace ▸ a profile: 0 Line art, 1 Logo, 2 Detailed logo, 3 Clipart,
    /// 4 Low quality image, 5 High quality image.
    OutlineTrace(u8),
    /// Bitmaps ▸ Centerline Trace ▸ 0 Technical Illustration, 1 Line drawing.
    CenterlineTrace(u8),
    RepeatLastTrace,
    OpenLastTraceSettings,
    StraightenImage,
    ImageAdjustmentLab,
    RemoveJpegArtifacts,
    EditBitmap,
    RemoveBackground,
    CropBitmap,
    /// Bitmaps ▸ Mode ▸ `crate::bitmaps::MODES[i]`.
    BitmapMode(u8),
    /// Bitmaps ▸ Inflate Bitmap ▸ Auto Inflate Bitmap (a setting).
    AutoInflateBitmap,
    ManualInflateBitmap,
    /// Bitmaps ▸ Break Link / Update from Link (linked bitmaps).
    BitmapBreakLink,
    BitmapUpdateLink,
    /// Adds the effect `crate::fx::FX[i]` to the selection and shows its page.
    BitmapEffect(u8),
    /// Effects ▸ Flatten Effects: the selected objects with bitmap effects become bitmaps.
    FlattenEffects,
    AutoAdjust,
    ResampleBitmap,
    // Text
    EditText,
    FitTextToPath,
    StraightenText,
    ConvertText,
    InsertGlyph,
    ChangeCase(u8),
    TextTabs,
    TextColumns,
    TextBullets,
    TextDropCap,
    TextStatistics,
    /// Insert Formatting Code: 0 Em Space, 1 En Space, 2 ¼ Em Space, 3 Non-breaking Space, 4 Tab,
    /// 5 Column/Frame Break, 6 Em Dash, 7 En Dash, 8 Non-breaking Hyphen, 9 Optional Hyphen.
    InsertFormatCode(u8),
    CustomHyphens,
    ToggleNonPrinting,
    ToggleTextFrames,
    FitTextToFrame,
    InsertPlaceholder,
    CreateEmptyFrame,
    LinkFrames,
    UnlinkFrames,
    AlignToBaseline,
    AlignToBaselineGrid,
    ToggleHyphenation,
    /// Writing Tools: 0 Spell Check, 1 Grammatik, 2 Thesaurus.
    WritingTools(u8),
    QuickCorrect,
    WritingToolsSettings,
    TextLanguage,
    ChangeCaseDialog,
    MakeWebCompatible,
    EncodeText,
    SpellCheck,
    // Table
    CreateTable,
    TableInsertRow(bool),
    TableInsertCol(bool),
    TableDeleteRow,
    TableDeleteCol,
    TableDistributeRows,
    TableDistributeCols,
    TextToTable,
    TableToText,
    /// Insert Rows... (false) and Insert Columns... (true).
    TableInsertMany(bool),
    /// Select ▸ Cell, Row, Column, Table.
    TableSelect(u8),
    TableDeleteTable,
    TableMerge,
    TableUnmerge,
    /// Split into Rows... (false) and Split into Columns... (true).
    TableSplit(bool),
    // Object
    CreateArrowhead,
    CreatePattern,
    InsertQrCode,
    InsertBarcode,
    ValidateBarcode,
    /// AI Generate guided by the selection: 0 vector graphic, 1 image, 2 remix.
    AiGuided(u8),
    /// AI Generate ▸ Create vector graphic, Create image, Remix image: the docker on that tab.
    AiMode(u8),
    SymbolEditMaster,
    SymbolFinish,
    SymbolBreakLink,
    SymbolUpdateLink,
    SymbolExportLibrary,
    RolloverCreate,
    RolloverExtract,
    RolloverEdit,
    RolloverFinish,
    RolloverLivePreview,
    RolloverDuplicateState,
    RolloverDeleteState,
    ShowHotspots,
    SymmetryCreate,
    SymmetryEdit,
    SymmetryFinish,
    SymmetryRemove,
    SymmetryBreak,
    PerspectiveDraw,
    PerspectiveEdit,
    PerspectiveFinish,
    /// Move to Plane: 0 orthographic, 1 top, 2 left, 3 right, 4 side.
    MoveToPlane(u8),
    InsertObject,
    /// Open an inserted object (OLE) in its program.
    OleOpen,
    // Tools
    Options(OptionsPage),
    SaveSettingsAsDefault,
    ColorManagement,
    ScriptEditor,
    RunScript,
    VsEditor,
    StartRecording,
    PauseRecording,
    StopRecording,
    CancelRecording,
    RecordTemporary,
    RunTemporary,
    // Window
    NewWindow,
    RefreshWindow,
    CloseWindow,
    CloseAllWindows,
    Cascade,
    TileH,
    TileV,
    CombineWindows,
    DockWindow,
    Workspace(u8),
    WorkspaceNew,
    WorkspaceDelete,
    WorkspaceImport,
    WorkspaceExport,
    Docker(DockerId),
    ToggleToolbar(crate::app::Toolbar),
    ButtonSize(crate::settings::ButtonSize),
    LockToolbars,
    AddColorsFromSelection,
    AddColorsFromDocument,
    CreatePaletteFromSelection,
    CreatePaletteFromDocument,
    OpenPalette,
    CloseAllPalettes,
    PaletteEditor,
    ToggleDefaultPalette,
    ToggleDocumentPalette,
    ToggleColorStylesPalette,
    WindowList(usize),
    // Help
    ProductHelp,
    WelcomeScreen,
    VideoTutorials,
    QuickStart,
    UserGuide,
    WhatsNew,
    Updates,
    MessageSettings,
    Community,
    Support,
    Feedback,
    ReportBug,
    ViewReportedBugs,
    RefreshAccount,
    AccountSettings,
    About,
    // Misc
    Tool(ToolId),
    Nudge(f64, f64),
    AiGenerate,
    NewFromClipboard,
    ShowHome,
    /// The Transform docker on a page: 0 Position, 1 Rotate, 2 Scale and Mirror, 3 Size, 4 Skew.
    TransformTab(u8),
    /// Toggle Pick State (Ctrl+Space): the Pick tool and back to the previous tool.
    TogglePickState,
    /// Shape tool keys on the selected nodes: 0 Toggle Cusp and Smooth (C), 1 Toggle Smooth and
    /// Symmetrical (S), 2 Toggle Line (L).
    NodeToggle(u8),
}

/// Where a context shortcut table applies (the reference's tables other than the global one).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum KeyContext {
    /// Text being edited (artistic or paragraph).
    Text,
    /// The Shape tool with nodes selected.
    Nodes,
    /// The Table tool with cells selected.
    TableCells,
}

/// Default keyboard shortcuts (text shown in menus and parsed for input). The chords are the
/// reference's global table (`DrawUI.xml`, the same in every UI language); the spelling follows
/// its menus ("Shift+PgUp"). More chords for the same command: [`alt_shortcuts`].
pub fn default_shortcut(cmd: Cmd) -> Option<&'static str> {
    use crate::dockers::DockerId as D;
    use vector_core::ops::{Align as Al, Distribute as Di, Order as Or};
    use Cmd::*;
    Some(match cmd {
        // File
        New => "Ctrl+N",
        Open => "Ctrl+O",
        Save => "Ctrl+S",
        SaveAs => "Ctrl+Shift+S",
        Import => "Ctrl+I",
        Export => "Ctrl+E",
        Print => "Ctrl+P",
        Exit => "Alt+F4",
        // Edit
        Undo => "Ctrl+Z",
        Redo => "Ctrl+Shift+Z",
        Repeat => "Ctrl+R",
        Cut => "Ctrl+X",
        Copy => "Ctrl+C",
        Paste => "Ctrl+V",
        PasteInView => "Ctrl+Shift+V",
        Delete => "Delete",
        Duplicate => "Ctrl+D",
        StepAndRepeat => "Ctrl+Shift+D",
        FindReplace => "Ctrl+F",
        SelectAll => "Ctrl+A",
        ExitFocusMode => "Shift+Esc",
        // View
        FullScreenPreview => "F9",
        ZoomToSelected => "Shift+F2",
        ZoomToAll => "F4",
        ZoomToPage => "Shift+F4",
        ZoomOut => "F3",
        ZoomIn => "Ctrl++",
        ToggleRulers => "Alt+Shift+R",
        SnapOff => "Alt+Q",
        SnapToGrid => "Alt+Y",
        SnapToObjects => "Alt+Z",
        ToggleAlignmentGuides => "Alt+Shift+A",
        ToggleDynamicGuides => "Alt+Shift+D",
        // Layout
        NextPage => "PgDn",
        PrevPage => "PgUp",
        // Object
        Group => "Ctrl+G",
        Ungroup => "Ctrl+U",
        Combine => "Ctrl+L",
        BreakApart => "Ctrl+K",
        ConvertToCurves => "Ctrl+Q",
        OutlineToObject => "Ctrl+Shift+Q",
        JoinCurves => "Ctrl+Shift+J",
        Order(Or::ToFront) => "Shift+PgUp",
        Order(Or::ToBack) => "Shift+PgDn",
        Order(Or::ForwardOne) => "Ctrl+PgUp",
        Order(Or::BackOne) => "Ctrl+PgDn",
        Align(Al::Left) => "L",
        Align(Al::Right) => "R",
        Align(Al::Top) => "T",
        Align(Al::Bottom) => "B",
        // the reference's menu: Align Centers Horizontally C, Align Centers Vertically E (its
        // icons: C lines the centers up on a vertical line, E on a horizontal one)
        Align(Al::CenterH) => "C",
        Align(Al::CenterV) => "E",
        CenterToPage => "P",
        Distribute(Di::Left) => "Shift+L",
        Distribute(Di::CenterH) => "Shift+E",
        Distribute(Di::Right) => "Shift+R",
        Distribute(Di::SpacingH) => "Shift+P",
        Distribute(Di::Top) => "Shift+T",
        Distribute(Di::CenterV) => "Shift+C",
        Distribute(Di::Bottom) => "Shift+B",
        Distribute(Di::SpacingV) => "Shift+A",
        SymmetryCreate => "Alt+S",
        SymmetryRemove => "Alt+Shift+S",
        SymmetryBreak => "Alt+X",
        SymmetryEdit => "Alt+Shift+Y",
        SymmetryFinish => "Alt+Shift+E",
        TransformTab(1) => "Alt+F8",
        TransformTab(2) => "Alt+F9",
        TransformTab(3) => "Alt+F10",
        ObjectProperties => "Alt+Enter",
        // Effects and Bitmaps: Adjust ▸ Light, Color Balance, Hue/Saturation/Lightness
        BitmapEffect(i) => match crate::fx::FX.get(i as usize).and_then(|f| f.guid.get(..8)) {
            Some("b6f2f97d") => "Ctrl+B",
            Some("2f08faa9") => "Ctrl+Shift+B",
            Some("aee88264") => "Ctrl+Shift+U",
            _ => return None,
        },
        // Text
        EditText => "Ctrl+Shift+T",
        ConvertText => "Ctrl+F8",
        AlignToBaseline => "Alt+F12",
        WritingTools(0) => "Ctrl+F12",
        // Text ▸ Bullets and the mesh fill's Sample Color (property bar items)
        BarOption(0xfa26e7e3) => "Ctrl+M",
        BarOption(0x0d1dd68e) => "Ctrl+Shift+E",
        // Tools
        Options(OptionsPage::App) => "Ctrl+J",
        ScriptEditor => "Alt+F11",
        VsEditor => "Alt+Shift+F12",
        StopRecording => "Ctrl+Shift+O",
        RecordTemporary => "Ctrl+Shift+R",
        RunTemporary => "Ctrl+Shift+P",
        // Window
        RefreshWindow => "Ctrl+W",
        CloseWindow => "Ctrl+F4",
        TogglePickState => "Ctrl+Space",
        Docker(D::Lens) => "Alt+F3",
        Docker(D::Contour) => "Ctrl+F9",
        Docker(D::Envelope) => "Ctrl+F7",
        Docker(d) => return crate::dockers::shortcut(d),
        Tool(t) => {
            let s = crate::tools::info(t).shortcut;
            if s.is_empty() {
                return None;
            }
            s
        }
        _ => return None,
    })
}

/// The reference's other chords for a command (it binds some commands to several keys).
pub fn alt_shortcuts(cmd: Cmd) -> &'static [&'static str] {
    use vector_core::ops::Order as Or;
    use Cmd::*;
    match cmd {
        Delete => &["Backspace"],
        Undo => &["Alt+Backspace"],
        Cut => &["Shift+Delete"],
        Copy => &["Ctrl+Insert"],
        Paste => &["Shift+Insert"],
        ZoomIn => &["Ctrl+Num +", "Ctrl+="],
        ZoomOut => &["Ctrl+-", "Ctrl+Num -", "Ctrl+_"],
        // To Front / To Back of Page (AnyVector orders within the layer for both)
        Order(Or::ToFront) => &["Ctrl+Home"],
        Order(Or::ToBack) => &["Ctrl+End"],
        _ => &[],
    }
}

/// The reference's context tables: chords that mean something else while the context is
/// active. `None` = the chord is the reference's but AnyVector has no command for it (or the
/// text editor handles the key itself): the global command must not run.
pub fn context_shortcuts(c: KeyContext) -> &'static [(&'static str, Option<Cmd>)] {
    use Cmd::*;
    match c {
        KeyContext::Text => &[
            ("Ctrl+B", Some(BarOption(0xbbf8b8dd))),
            ("Ctrl+I", Some(BarOption(0xa2e29751))),
            ("Ctrl+U", Some(BarOption(0x4e23866f))),
            ("Ctrl+L", Some(BarOption(0x808ee536))),
            ("Ctrl+E", Some(BarOption(0xb8f147f2))),
            ("Ctrl+R", Some(BarOption(0x40a3ad42))),
            ("Ctrl+J", Some(BarOption(0xeed5a12c))),
            ("Ctrl+H", Some(BarOption(0x4edf17b6))),
            ("Ctrl+N", Some(BarOption(0x6fca7424))),
            ("Ctrl+Shift+D", Some(BarOption(0x6123599f))),
            ("Shift+F3", Some(ChangeCaseDialog)),
            ("Ctrl+Shift+C", Some(ToggleNonPrinting)),
            ("Ctrl+Shift+M", Some(InsertFormatCode(0))),
            ("Ctrl+Shift+N", Some(InsertFormatCode(1))),
            ("Ctrl+Alt+Space", Some(InsertFormatCode(2))),
            ("Ctrl+Shift+Space", Some(InsertFormatCode(3))),
            ("Ctrl+Enter", Some(InsertFormatCode(5))),
            ("Alt+_", Some(InsertFormatCode(6))),
            ("Alt+-", Some(InsertFormatCode(7))),
            ("Ctrl+Shift+-", Some(InsertFormatCode(8))),
            ("Ctrl+_", Some(InsertFormatCode(8))),
            ("Ctrl+-", Some(InsertFormatCode(9))),
            // no command yet: Font List, Font Size, Style List, Font Style, Small Capitals, the
            // HTML font size list, the Text docker (Ctrl+F10), Find and Replace Text
            ("Ctrl+Shift+F", None),
            ("Ctrl+Shift+P", None),
            ("Ctrl+Shift+S", None),
            ("Ctrl+Shift+W", None),
            ("Ctrl+Shift+K", None),
            ("Ctrl+Shift+H", None),
            ("Ctrl+F10", None),
            ("Alt+F3", None),
            // caret movement and selection: the text editor
            ("Ctrl+A", None),
            ("Delete", None),
            ("Ctrl+Delete", None),
            ("Ctrl+Left", None),
            ("Ctrl+Right", None),
            ("Ctrl+Up", None),
            ("Ctrl+Down", None),
            ("Ctrl+Shift+Left", None),
            ("Ctrl+Shift+Right", None),
            ("Ctrl+Shift+Up", None),
            ("Ctrl+Shift+Down", None),
            ("PgUp", None),
            ("PgDn", None),
            ("Shift+PgUp", None),
            ("Shift+PgDn", None),
            ("Ctrl+PgUp", None),
            ("Ctrl+PgDn", None),
            ("Ctrl+Shift+PgUp", None),
            ("Ctrl+Shift+PgDn", None),
            ("Ctrl+Home", None),
            ("Ctrl+End", None),
            ("Ctrl+Shift+Home", None),
            ("Ctrl+Shift+End", None),
        ],
        KeyContext::Nodes => &[("C", Some(NodeToggle(0))), ("S", Some(NodeToggle(1))), ("L", Some(NodeToggle(2)))],
        KeyContext::TableCells => &[("Ctrl+M", Some(TableMerge)), ("Ctrl+A", None)],
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Shortcut {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: egui::Key,
}

/// Parses "Ctrl+Shift+S", also the reference spellings: "Shift+PgUp", "Ctrl++", "Ctrl+Num +",
/// "Ctrl+_" (Shift and the minus key), "Ctrl+Num 8".
pub fn parse_shortcut(s: &str) -> Option<Shortcut> {
    let mut sc = Shortcut { ctrl: false, shift: false, alt: false, key: egui::Key::A };
    let s = s.trim();
    // the key itself may be "+" ("Ctrl++", "Ctrl+Num +")
    let (mods, last) = match s.strip_suffix("++") {
        Some(m) => (m, "+"),
        None => match s.rfind('+') {
            Some(i) if s.ends_with("Num +") => (s[..i.saturating_sub(4)].trim_end_matches('+'), "+"),
            Some(i) => (&s[..i], &s[i + 1..]),
            None => ("", s),
        },
    };
    for part in mods.split('+').filter(|p| !p.trim().is_empty()) {
        match part.trim() {
            "Ctrl" => sc.ctrl = true,
            "Shift" => sc.shift = true,
            "Alt" => sc.alt = true,
            _ => return None,
        }
    }
    let k = last.trim();
    let k = k.strip_prefix("Num ").unwrap_or(k);
    sc.key = match k {
        "PgUp" | "Page Up" => egui::Key::PageUp,
        "PgDn" | "Page Down" => egui::Key::PageDown,
        "Esc" => egui::Key::Escape,
        "_" => {
            sc.shift = true;
            egui::Key::Minus
        }
        _ => egui::Key::from_name(k)?,
    };
    Some(sc)
}

/// All commands that have default shortcuts, for dispatch. The most specific chords come first:
/// egui matches a chord with extra Shift or Alt held too, so "Ctrl+S" must not be tried before
/// "Ctrl+Shift+S", nor "L" before "Shift+L".
pub fn shortcut_commands() -> Vec<Cmd> {
    use vector_core::ops::{Align as Al, Distribute as Di, Order as Or};
    use Cmd::*;
    let mut v = vec![
        New, Open, Save, SaveAs, Import, Export, Print, Undo, Redo, Repeat, Cut, Copy, Paste, PasteInView, Delete, Duplicate, SelectAll, ExitFocusMode,
        FullScreenPreview, ZoomToSelected, ZoomToAll, ZoomToPage, ZoomOut, ZoomIn, Group, Ungroup, Combine, BreakApart, ConvertToCurves, OutlineToObject, JoinCurves,
        Order(Or::ToFront), Order(Or::ToBack), Order(Or::ForwardOne), Order(Or::BackOne),
        Align(Al::Left), Align(Al::Right), Align(Al::Top), Align(Al::Bottom), Align(Al::CenterH), Align(Al::CenterV), CenterToPage,
        Distribute(Di::Left), Distribute(Di::CenterH), Distribute(Di::Right), Distribute(Di::SpacingH), Distribute(Di::Top), Distribute(Di::CenterV), Distribute(Di::Bottom), Distribute(Di::SpacingV),
        SymmetryCreate, SymmetryRemove, SymmetryBreak, SymmetryEdit, SymmetryFinish, TransformTab(1), TransformTab(2), TransformTab(3),
        EditText, ConvertText, AlignToBaseline, WritingTools(0), BarOption(0xfa26e7e3), BarOption(0x0d1dd68e),
        Options(OptionsPage::App), ScriptEditor, VsEditor, StopRecording, RecordTemporary, RunTemporary, RefreshWindow, CloseWindow, TogglePickState, NextPage, PrevPage,
        ToggleRulers, SnapOff, SnapToGrid, SnapToObjects, ToggleAlignmentGuides, ToggleDynamicGuides,
    ];
    for g in ["b6f2f97d", "2f08faa9", "aee88264"] {
        if let Some((i, _)) = crate::fx::by_guid(g) {
            v.push(BitmapEffect(i as u8));
        }
    }
    for d in crate::dockers::ALL {
        if default_shortcut(Docker(*d)).is_some() {
            v.push(Docker(*d));
        }
    }
    for t in crate::tools::TOOLS {
        if default_shortcut(Tool(t.id)).is_some() {
            v.push(Tool(t.id));
        }
    }
    v.sort_by_key(|c| std::cmp::Reverse(default_shortcut(*c).and_then(parse_shortcut).map(|s| s.ctrl as u8 + s.shift as u8 + s.alt as u8).unwrap_or(0)));
    v
}

#[cfg(test)]
mod tests {
    use super::*;

    fn same(a: &str, b: &str) -> bool {
        parse_shortcut(a).is_some() && parse_shortcut(a) == parse_shortcut(b)
    }

    #[test]
    fn parses_reference_spellings() {
        let k = |s: &str| parse_shortcut(s).map(|s| (s.ctrl, s.shift, s.alt, s.key));
        use egui::Key;
        assert_eq!(k("Ctrl++"), Some((true, false, false, Key::Plus)));
        assert_eq!(k("Ctrl+Num +"), Some((true, false, false, Key::Plus)));
        assert_eq!(k("Ctrl+Num -"), Some((true, false, false, Key::Minus)));
        assert_eq!(k("Ctrl+-"), Some((true, false, false, Key::Minus)));
        assert_eq!(k("Ctrl+_"), Some((true, true, false, Key::Minus)));
        assert_eq!(k("Alt+_"), Some((false, true, true, Key::Minus)));
        assert_eq!(k("Ctrl+="), Some((true, false, false, Key::Equals)));
        assert_eq!(k("Shift+PgUp"), Some((false, true, false, Key::PageUp)));
        assert_eq!(k("Ctrl+PgDn"), Some((true, false, false, Key::PageDown)));
        assert_eq!(k("Shift+Esc"), Some((false, true, false, Key::Escape)));
        assert_eq!(k("Ctrl+Alt+Space"), Some((true, false, true, Key::Space)));
        assert_eq!(k("Ctrl+Num 8"), Some((true, false, false, Key::Num8)));
        assert_eq!(k("Alt+Shift+F12"), Some((false, true, true, Key::F12)));
        assert_eq!(k("Ctrl+Shift+S"), Some((true, true, false, Key::S)));
        assert_eq!(k("Delete"), Some((false, false, false, Key::Delete)));
        assert_eq!(k("Ctrl+Foo"), None);
    }

    #[test]
    fn menu_keys_match_dispatch() {
        // every chord the menus show for an item runs that item's command
        for b in crate::ui::bars() {
            for it in b.items {
                let Some(c) = crate::ui::bind::cmd(it.id) else { continue };
                for k in it.keys {
                    let ok = default_shortcut(c).is_some_and(|d| same(d, k)) || alt_shortcuts(c).iter().any(|a| same(a, k));
                    assert!(ok, "{} ({}) shows {k} but {c:?} has {:?} {:?}", it.text, it.id, default_shortcut(c), alt_shortcuts(c));
                }
            }
        }
    }

    #[test]
    fn dispatch_chords_are_unique_and_ordered() {
        let v: Vec<(Cmd, Shortcut)> = shortcut_commands().into_iter().map(|c| (c, default_shortcut(c).and_then(parse_shortcut).unwrap_or_else(|| panic!("{c:?}")))).collect();
        for (i, (a, sa)) in v.iter().enumerate() {
            for (b, sb) in &v[i + 1..] {
                assert!(sa != sb, "{a:?} and {b:?} share a chord");
                // egui matches a chord with extra Shift/Alt held: a less specific chord tried
                // first would take the other one
                let takes = sa.key == sb.key && sa.ctrl == sb.ctrl && (!sa.shift || sb.shift) && (!sa.alt || sb.alt);
                assert!(!takes, "{a:?} would take {b:?}'s chord");
            }
        }
        for c in [KeyContext::Text, KeyContext::Nodes, KeyContext::TableCells] {
            for (k, _) in context_shortcuts(c) {
                assert!(parse_shortcut(k).is_some(), "{k}");
            }
        }
        for c in shortcut_commands() {
            for k in alt_shortcuts(c) {
                assert!(parse_shortcut(k).is_some(), "{k}");
            }
        }
    }
}
