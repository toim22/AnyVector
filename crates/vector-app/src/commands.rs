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
    GoToPage,
    NextPage,
    PrevPage,
    SwitchOrientation,
    PageSetup,
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
    CopyEffectFrom,
    BreakBlend,
    AddPerspective,
    ClearPerspective,
    BevelEffect,
    LensEffect,
    // Bitmaps
    ConvertToBitmap,
    QuickTrace,
    OutlineTrace,
    BitmapEffect(u8),
    AutoAdjust,
    ResampleBitmap,
    // Text
    EditText,
    FitTextToPath,
    StraightenText,
    ConvertText,
    InsertGlyph,
    ChangeCase(u8),
    SpellCheck,
    // Table
    CreateTable,
    TableInsertRow(bool),
    TableInsertCol(bool),
    TableDeleteRow,
    TableDeleteCol,
    TableDistributeRows,
    TableDistributeCols,
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
}

/// Default keyboard shortcuts (text shown in menus and parsed for input).
pub fn default_shortcut(cmd: Cmd) -> Option<&'static str> {
    use Cmd::*;
    Some(match cmd {
        New => "Ctrl+N",
        Open => "Ctrl+O",
        Save => "Ctrl+S",
        SaveAs => "Ctrl+Shift+S",
        Import => "Ctrl+I",
        Export => "Ctrl+E",
        Print => "Ctrl+P",
        Exit => "Alt+F4",
        Undo => "Ctrl+Z",
        Redo => "Ctrl+Shift+Z",
        Repeat => "Ctrl+R",
        Cut => "Ctrl+X",
        Copy => "Ctrl+C",
        Paste => "Ctrl+V",
        Delete => "Delete",
        Duplicate => "Ctrl+D",
        StepAndRepeat => "Ctrl+Shift+D",
        FindReplace => "Ctrl+F",
        SelectAll => "Ctrl+A",
        FullScreenPreview => "F9",
        ZoomToSelected => "Shift+F2",
        ZoomToAll => "F4",
        ZoomToPage => "Shift+F4",
        ZoomOut => "F3",
        ZoomIn => "F2",
        Group => "Ctrl+G",
        Ungroup => "Ctrl+U",
        Combine => "Ctrl+L",
        BreakApart => "Ctrl+K",
        ConvertToCurves => "Ctrl+Q",
        OutlineToObject => "Ctrl+Shift+Q",
        Order(vector_core::ops::Order::ToFront) => "Shift+PageUp",
        Order(vector_core::ops::Order::ToBack) => "Shift+PageDown",
        Order(vector_core::ops::Order::ForwardOne) => "Ctrl+PageUp",
        Order(vector_core::ops::Order::BackOne) => "Ctrl+PageDown",
        Align(vector_core::ops::Align::Left) => "L",
        Align(vector_core::ops::Align::Right) => "R",
        Align(vector_core::ops::Align::Top) => "T",
        Align(vector_core::ops::Align::Bottom) => "B",
        Align(vector_core::ops::Align::CenterH) => "E",
        Align(vector_core::ops::Align::CenterV) => "C",
        Options(OptionsPage::App) => "Ctrl+J",
        ScriptEditor => "Alt+F11",
        VsEditor => "Alt+Shift+F12",
        StopRecording => "Ctrl+Shift+O",
        RecordTemporary => "Ctrl+Shift+R",
        RunTemporary => "Ctrl+Shift+P",
        RefreshWindow => "Ctrl+W",
        CloseWindow => "Ctrl+F4",
        EditText => "Ctrl+Shift+T",
        ObjectProperties => "Alt+Enter",
        NextPage => "PageDown",
        PrevPage => "PageUp",
        ToggleRulers => "Alt+Shift+R",
        SnapOff => "Alt+Q",
        SnapToGrid => "Alt+Y",
        SnapToObjects => "Alt+Z",
        ToggleAlignmentGuides => "Alt+Shift+A",
        ToggleDynamicGuides => "Alt+Shift+D",
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

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Shortcut {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
    pub key: egui::Key,
}

pub fn parse_shortcut(s: &str) -> Option<Shortcut> {
    let mut sc = Shortcut { ctrl: false, shift: false, alt: false, key: egui::Key::A };
    let mut key = None;
    for part in s.split('+') {
        match part.trim() {
            "Ctrl" => sc.ctrl = true,
            "Shift" => sc.shift = true,
            "Alt" => sc.alt = true,
            k => {
                key = match k {
                    "Delete" => Some(egui::Key::Delete),
                    "PageUp" => Some(egui::Key::PageUp),
                    "PageDown" => Some(egui::Key::PageDown),
                    "Enter" => Some(egui::Key::Enter),
                    _ => egui::Key::from_name(k),
                }
            }
        }
    }
    sc.key = key?;
    Some(sc)
}

/// All commands that have default shortcuts, for dispatch.
pub fn shortcut_commands() -> Vec<Cmd> {
    use Cmd::*;
    let mut v = vec![
        New, Open, Save, SaveAs, Import, Export, Print, Undo, Redo, Repeat, Cut, Copy, Paste, Delete, Duplicate, StepAndRepeat, FindReplace, SelectAll, FullScreenPreview, ZoomToSelected, ZoomToAll, ZoomToPage, ZoomOut, Group, Ungroup, Combine, BreakApart, ConvertToCurves, OutlineToObject,
        Order(vector_core::ops::Order::ToFront), Order(vector_core::ops::Order::ToBack), Order(vector_core::ops::Order::ForwardOne), Order(vector_core::ops::Order::BackOne),
        Align(vector_core::ops::Align::Left), Align(vector_core::ops::Align::Right), Align(vector_core::ops::Align::Top), Align(vector_core::ops::Align::Bottom), Align(vector_core::ops::Align::CenterH), Align(vector_core::ops::Align::CenterV),
        Options(OptionsPage::App), ScriptEditor, StopRecording, RecordTemporary, RunTemporary, RefreshWindow, CloseWindow, EditText, NextPage, PrevPage,
        ToggleRulers, SnapOff, SnapToGrid, SnapToObjects, ToggleAlignmentGuides, ToggleDynamicGuides,
    ];
    for d in crate::dockers::ALL {
        if crate::dockers::shortcut(*d).is_some() {
            v.push(Docker(*d));
        }
    }
    for t in crate::tools::TOOLS {
        if !t.shortcut.is_empty() {
            v.push(Tool(t.id));
        }
    }
    v
}
