//! What AnyVector does with each item of the reference interface: the command it runs, and the
//! state (checked, enabled, caption) that depends on the open document or the settings.
//!
//! Items are matched by the first 8 characters of their id (unique across the data, checked by
//! a test). Items without a command here are drawn as in the reference; choosing one says the
//! feature is not available yet.

use crate::app::{Toolbar, AnyVectorApp};
use crate::commands::{Cmd, OptionsPage};
use crate::dockers::DockerId;
use crate::docview::ViewMode as Vm;
use crate::settings::ButtonSize as Bs;
use crate::tools::{NodeOp as No, ToolId};
use vector_core::ops::{Align as Al, Order as Ord, Shaping as Sh};
use vector_core::ObjKind;

fn key(id: &str) -> String {
    id.get(..8).unwrap_or(id).to_ascii_lowercase()
}

/// The docker an item opens or closes.
pub fn docker(id: &str) -> Option<DockerId> {
    use DockerId::*;
    Some(match key(id).as_str() {
        "b9c96a22" => Properties,
        "2f0b4656" => Objects,
        "1f481838" => Pages,
        "817b2ab9" => Comments,
        "0de570eb" => ObjectData,
        "41607d8d" => ObjectStyles,
        "90e01b9d" => Symbols,
        "58fc6ba1" => FindReplace,
        "da2559c6" => Export,
        "d5596b3a" => AiGenerate,
        "4f467f4a" => Transform,
        "40d6bf17" => Coordinates,
        "73958d0f" => LiveGuides,
        "a9c6428a" => Guidelines,
        "40e1a4d1" => AlignDistribute,
        "fc849af3" => FitObjectsToPath,
        "39692b96" => StepRepeat,
        "bc9ee4f2" => Shape,
        "939a8f9c" => Corners,
        "ce73460f" => JoinCurves,
        "9d4d5201" => Text,
        "035e2905" => FontSampler,
        "00c72ae6" => Glyphs,
        "5fc0becf" => ArtisticMedia,
        "6268eac5" => BitmapMask,
        "03687a21" => PhotoCocktail,
        "d509273d" => Pointillizer,
        "91062f47" => Bevel,
        "bda9bd40" => Blend,
        "0a57d894" => Contour,
        "e08bc376" => Envelope,
        "952e4cc8" => Extrude,
        "1c86e69f" => Lens,
        "99b8c7af" => Color,
        "c1db12ea" => Palettes,
        "2e897ace" => ColorStyles,
        "6495e877" => ColorProofing,
        "e8221acf" => Assets,
        "17714be3" => Tray,
        "d7fe25eb" => LinksRollovers,
        "2972bbb3" => Sources,
        "22cc7130" => Views,
        "dc1afc8c" => History,
        "13d17830" => Scripts,
        _ => return None,
    })
}

/// The tool an item selects (toolbox and Create Object menu).
pub fn tool(id: &str) -> Option<ToolId> {
    match key(id).as_str() {
        // the Common Shape button on the Smart Drawing property bar
        "5e1f702f" => Some(ToolId::CommonShapes),
        _ => crate::tools::item_tools(id).first().copied(),
    }
}

/// The command an item runs, if AnyVector has it.
pub fn cmd(id: &str) -> Option<Cmd> {
    use Cmd::*;
    match id {
        "vector:report-bug-new" => return Some(ReportBug),
        "vector:view-bugs" => return Some(ViewReportedBugs),
        _ => {}
    }
    if let Some(d) = docker(id) {
        return Some(Docker(d));
    }
    if let Some(t) = tool(id) {
        return Some(Tool(t));
    }
    let k = key(id);
    if crate::propbar::handles(&k) {
        return u32::from_str_radix(&k, 16).ok().map(BarOption);
    }
    Some(match k.as_str() {
        // File
        "fa65d0c1" => New,
        "f8179d03" => NewFromTemplate,
        "b3a1c7e2" => QuickAiGenerate,
        "f5a570a9" | "2d9c749f" => Open,
        "eba0aaa3" => Close,
        "19d68220" => CloseAll,
        "3c4ccfbb" => Save,
        "b8d0e1d8" => SaveAs,
        "8a1ee996" => SaveAsTemplate,
        "90c536e4" => Revert,
        "2e3cde6c" => SelectTwain,
        "493dda8f" => SelectWia,
        "1d9958bc" => Acquire,
        "785c273f" => Import,
        "d430d269" => Export,
        "cd42798b" => ExportForOffice,
        "4b3e61ff" => ExportForWeb,
        "07592ecb" => ExportForWordPress,
        "b089d276" => ExportDockerAddSelection,
        "badeac4f" => ExportDockerAddPage,
        "ea2d4e6b" => ExportDockerAddAllPages,
        "8f29b2d6" => SendToMail,
        "f5f7cfa8" => PublishPdf,
        "ea816d5e" => PrintMergeCreate,
        "9838344c" => PrintMergeEdit,
        "21dfd614" => PrintMergePerform,
        "46c3dff5" => PrintMergeNewDoc,
        "e3f5366a" => Print,
        "51771ba9" => PrintPreview,
        "3324df3f" => DocumentProperties,
        "92260708" => Exit,
        // Edit
        "3237eadd" | "fc9524bc" => Undo,
        "0bfbf159" | "ddc87e9c" => Redo,
        "50dc2600" => Repeat,
        "0eb6f64d" => Cut,
        "cd550ee5" => Copy,
        "f16b149b" => CopyPropertiesFrom,
        "89088287" => Paste,
        "b52abcd3" => PasteSpecial,
        "c5b2ffa4" => PasteInView,
        "6f43fff0" => BringIntoFocus,
        "26889cbf" => ExitFocusMode,
        "19ae1afb" => Delete,
        "80f991a4" => Duplicate,
        "ae84ae30" => Clone,
        "a604992c" => SelectAll,
        "f87906ed" => SelectAllText,
        "15a91b82" => SelectAllGuides,
        "7f0874a1" => SelectAllNodes,
        // View
        "c67f5323" => ViewMode(Vm::Wireframe),
        "844cb5e2" => ViewMode(Vm::Normal),
        "e09c1ea0" => ViewMode(Vm::Enhanced),
        "6f9db850" => ViewMode(Vm::Pixels),
        "141c5d5e" => ViewMode(Vm::PatternPreview),
        "e279d745" => ToggleSimulateOverprints,
        "4b726144" => PreviewSelectedOnly,
        "2273acde" => ToggleMultipageView,
        "66dd7512" => ToggleProofColors,
        "b3f2d00d" => ToggleRasterizeEffects,
        "f1d540fc" => TogglePrintableArea,
        "99b1ef61" => ToggleBaselineGrid,
        "de9c3db5" => ToggleAlignmentGuides,
        "540eaeee" => ToggleDynamicGuides,
        "c7533564" => ToggleDocNavigator,
        "0a9a07c2" => ToggleScrollbars,
        "e0c557dc" => SnapToPixels,
        "317a9b45" => SnapToBaseline,
        "5387eaac" => FullScreenPreview,
        "073c3fba" => ZoomIn,
        "729045ed" => ZoomOut,
        "21443d9c" => ZoomToAll,
        "29c6b577" => ZoomToSelected,
        "5a73b632" => ZoomToPage,
        "5dccc3ed" => ZoomToWidth,
        "074d3aa0" => ZoomToHeight,
        "77f7f9eb" => TogglePageBorder,
        "1e0034de" => ToggleBleed,
        "543d2d90" => ToggleGrid,
        "64c626a4" => TogglePixelGrid,
        "4a490617" => ToggleRulers,
        "fc6531ef" => ToggleGuidelines,
        "b10c10b5" => SnapToGrid,
        "77754a6b" => SnapToGuides,
        "d8a02807" => SnapToObjects,
        "6e762121" => SnapToPage,
        "000f3257" => SnapOff,
        // Layout
        "4b29cd5e" => InsertPage,
        "5f56e9cf" => DuplicatePage,
        "d3db5c96" => InsertPageAfter,
        "ec5a9872" => InsertPageBefore,
        "cfc681b7" => GridSetup,
        "a67a81b1" => RulerSetup,
        "88ad5cd5" => GuidelinesSetup,
        "d9c0197f" => RenamePage,
        "774e822b" => DeletePage,
        "58bdbfc1" => DeletePageNow,
        "c34fb01f" => GoToPage,
        "88ee85c6" => SwitchOrientation,
        "0c273c3c" => PageSetup,
        "0ccf3b2f" => PageLayout,
        "da0c84ba" | "ba382370" => PageNumberSettings,
        "0738bc2c" => InsertPageNumber(0),
        "11530b48" => InsertPageNumber(1),
        "2b6b5aa4" => InsertPageNumber(2),
        "dbaae0c2" => InsertPageNumber(3),
        "eeb7f2b5" => BorderGrommet,
        "d1080c69" => PageBackground,
        // Object
        "da1efe7f" | "3f1c813c" => PowerClipInside,
        "6ad732d1" => PowerClipExtract,
        "cd3ae51f" => PowerClipEdit,
        "22eabb92" | "1b67872b" => PowerClipFinish,
        "be9422c1" => NewSymbol,
        "34a99e8e" => RevertSymbol,
        // the right-click menu of the Shape tool
        "0a4c11b0" => Node(No::Add),
        "31e25c04" => Node(No::Delete),
        "441b7ac9" => Node(No::Join),
        "10554e40" => Node(No::Break),
        "7625dd3a" => Node(No::ToLine),
        "d88304f5" => Node(No::ToCurve),
        "76786bce" => Node(No::Cusp),
        "31a5283a" => Node(No::Smooth),
        "8a110507" => Node(No::Symmetric),
        "3a691ea5" => Node(No::Reverse),
        "4d6dfca4" => Node(No::Extract),
        "0c57f0fd" => Node(No::Close),
        "856ccfda" => Node(No::ExtendClose),
        "b714bc06" => Node(No::Reduce),
        "29579c22" => Node(No::Elastic),
        "9388e24a" => ClearTransformations,
        "655a2e5f" => ClearEffect,
        "6dda0e64" => FlattenEffects,
        "2660e67c" => Align(Al::Left),
        "ef51f807" => Align(Al::Right),
        "d31c41f7" => Align(Al::Top),
        "abd75c09" => Align(Al::Bottom),
        "69bf9eec" => Align(Al::CenterH),
        "6aa736bd" => Align(Al::CenterV),
        "af85ad65" => CenterToPage,
        "d4ecae41" => AlignTo(Al::CenterH),
        "6ccb1937" => AlignTo(Al::CenterV),
        "87a885a9" => AlignPixelGrid,
        "e54d3dbe" => OrderInFrontOf,
        "ce2cdd81" => OrderBehind,
        "edd19eb3" => ShowObjects,
        "1651a600" => OverprintFill,
        "a86ade3d" => OverprintOutline,
        "cb0ffd87" => OverprintBitmap,
        "12c0a980" => ObjectHinting,
        "25f351b0" => PowerClipEmptyFrame,
        "6ea9b15a" => PowerClipLock,
        "273d95de" => PowerClipPlace(0),
        "a369e3ae" => PowerClipPlace(1),
        "56a4aae7" => PowerClipPlace(2),
        "8ab3326a" => PowerClipPlace(3),
        "b9be1ce5" => CopyPowerClipFrom,
        "fc31d658" => CopyEffectFrom(0),
        "e0f04f36" => CopyEffectFrom(1),
        "42c808a4" => CopyEffectFrom(2),
        "6d0d907d" => CopyEffectFrom(3),
        "8228969f" => CopyEffectFrom(4),
        "bdc3960c" => CopyEffectFrom(5),
        "5b6706b4" => CopyEffectFrom(6),
        "04b054a8" => CopyEffectFrom(7),
        "441d7582" => CloneEffectFrom(2),
        "48ec026e" => CloneEffectFrom(3),
        "ffc8be1d" => CloneEffectFrom(4),
        "2762b176" => CloneEffectFrom(6),
        "1431ed04" => Order(Ord::ToFront),
        "94db9b5a" => Order(Ord::ToBack),
        "6a26f5e7" => Order(Ord::ToFront),
        "8132c321" => Order(Ord::ToBack),
        "de23b084" => Order(Ord::ForwardOne),
        "2b47a59d" => Order(Ord::BackOne),
        "999dc04b" => ReverseOrder,
        "5e4f94a0" => Group,
        "76aa0c75" => Ungroup,
        "4a31123e" => UngroupAll,
        "6154a087" => HideObject,
        "16720e53" => ShowAllObjects,
        "bfbb358f" => Lock,
        "2cc02de0" => Unlock,
        "a5367ce2" => UnlockAll,
        "416c9d7d" => Shaping(Sh::Weld),
        "1d425d4a" => Shaping(Sh::Trim),
        "e5b41ae3" => Shaping(Sh::Intersect),
        "7da36c72" => Shaping(Sh::Simplify),
        "49d4605c" => Shaping(Sh::FrontMinusBack),
        "f64b0582" => Shaping(Sh::BackMinusFront),
        "b0491566" => Shaping(Sh::Boundary),
        "91774a85" => Combine,
        "d5669070" => BreakApart,
        "e6644135" => MirrorH,
        "6ae897fd" => MirrorV,
        "00f16820" => AddPerspective,
        "68000526" => ConvertToCurves,
        "c44919b9" => ConvertToBitmap,
        "e64806d9" => OutlineToObject,
        // Bitmaps
        "f9d61ad8" => AutoAdjust,
        "f85cb8c1" => ResampleBitmap,
        "d207991b" => QuickTrace,
        "465261a5" => OutlineTrace(0),
        "f7b939cd" => OutlineTrace(1),
        "21129fee" => OutlineTrace(2),
        "af57ed80" => OutlineTrace(3),
        "55a3ae58" => OutlineTrace(4),
        "3298c91b" => OutlineTrace(5),
        "1044c10b" => CenterlineTrace(0),
        "5b036050" => CenterlineTrace(1),
        "0258983a" => RepeatLastTrace,
        "726c629a" => OpenLastTraceSettings,
        "632996de" => StraightenImage,
        "1aaa4018" => ImageAdjustmentLab,
        "18ae168e" => RemoveJpegArtifacts,
        "8c5788af" => EditBitmap,
        "dd03eeb6" => RemoveBackground,
        "b2f431a3" => CropBitmap,
        "256c8079" => BitmapMode(0),
        "e27f34c1" => BitmapMode(1),
        "fa741d10" => BitmapMode(2),
        "1c3e5c32" => BitmapMode(3),
        "b2394c63" => BitmapMode(4),
        "e540b312" => BitmapMode(5),
        "6ec34929" => BitmapMode(6),
        "aae3b767" => AutoInflateBitmap,
        "0a2d0181" => ManualInflateBitmap,
        "4917fa89" => BitmapBreakLink,
        "118cd7c2" => BitmapUpdateLink,
        // Text
        "d14b05d4" => EditText,
        "7f650d0c" => ConvertText,
        "8c2ac89e" => FitTextToPath,
        "0ce0b6c1" => StraightenText,
        "067b5be7" => WritingTools(0),
        "e80117b8" => WritingTools(1),
        "72106e09" => WritingTools(2),
        "2edc8b8b" => QuickCorrect,
        "947e5baa" => TextLanguage,
        "f33ec567" => WritingToolsSettings,
        "9f68d8b2" => TextTabs,
        "11acb7c1" => TextColumns,
        "ed641ec9" => TextBullets,
        "4349fb4c" => TextDropCap,
        "bf3bd8fe" => TextStatistics,
        "ddb00511" => InsertFormatCode(0),
        "25874021" => InsertFormatCode(1),
        "c56686e2" => InsertFormatCode(2),
        "c548b8e9" => InsertFormatCode(3),
        "8ba7e63d" => InsertFormatCode(4),
        "cef94dde" => InsertFormatCode(5),
        "6b13bb31" => InsertFormatCode(6),
        "2539b9fc" => InsertFormatCode(7),
        "9445e179" => InsertFormatCode(8),
        "f70cbc29" => InsertFormatCode(9),
        "217e7e25" => CustomHyphens,
        "964c2574" => ToggleNonPrinting,
        "2c6d197a" => ToggleTextFrames,
        "e1b67386" => FitTextToFrame,
        "4aa2e1ed" => InsertPlaceholder,
        "886e12c9" => CreateEmptyFrame,
        "14753345" => LinkFrames,
        "a2286835" => UnlinkFrames,
        "01f7dc01" => AlignToBaseline,
        "38f80f45" => AlignToBaselineGrid,
        "ecda8c51" => ToggleHyphenation,
        "0e3eb2f9" => ChangeCaseDialog,
        "e4f6d170" => MakeWebCompatible,
        "63992d01" => EncodeText,
        // Table
        "ff44aa22" => CreateTable,
        "60122522" => TableInsertRow(true),
        "a26c7f0d" => TableInsertRow(false),
        "8c5fe6dd" => TableInsertCol(true),
        "5a6f9ad7" => TableInsertCol(false),
        "00966d39" => TableDeleteRow,
        "789f579f" => TableDeleteCol,
        "f5950e3b" => TableDistributeRows,
        "e5a43d87" => TableDistributeCols,
        "af65c0be" => TextToTable,
        "00557394" => TableToText,
        "68bea082" => TableInsertMany(false),
        "3ecca95c" => TableInsertMany(true),
        "3843ac65" => TableSelect(0),
        "dc1cf549" => TableSelect(1),
        "f06723e2" => TableSelect(2),
        "a187e17d" => TableSelect(3),
        "beeca38b" => TableDeleteTable,
        "38ae2fe7" => TableMerge,
        "ca1afb09" => TableUnmerge,
        "524b07d8" => TableSplit(false),
        "b60f4415" => TableSplit(true),
        // Object
        "73d5fcfa" => CreateArrowhead,
        "8d3cb8e7" => CreatePattern,
        "8a371e4d" => InsertQrCode,
        "c22f267c" => InsertBarcode,
        "ae803157" => ValidateBarcode,
        "a0776aad" => AiGuided(0),
        "2daeddaf" => AiGuided(1),
        "7e547078" => AiGuided(2),
        "9c0ccd48" => AiMode(0),
        "4bf3be7a" => AiMode(1),
        "8ed410c6" => AiMode(2),
        "b480a210" => SymbolEditMaster,
        "f9d79c11" => SymbolFinish,
        "fc7aec5f" => SymbolBreakLink,
        "3705f3b2" => SymbolUpdateLink,
        "1b8a3585" => SymbolExportLibrary,
        "2ee87e4a" => SymmetryCreate,
        "e5c0b36e" => SymmetryEdit,
        "755c36ec" | "25ebff93" => SymmetryFinish,
        "b8a1181c" => SymmetryRemove,
        "2250f74a" => SymmetryBreak,
        "46d38bcb" => PerspectiveDraw,
        "70453c54" => PerspectiveEdit,
        "d6bcdbfe" => MoveToPlane(0),
        "a1dea8f7" => MoveToPlane(1),
        "5b960863" => MoveToPlane(2),
        "8b1a3ea7" => MoveToPlane(3),
        "2ea909ed" => MoveToPlane(4),
        "724461d8" => InsertObject,
        "91dbd3be" => RolloverCreate,
        "9af6ee00" => RolloverExtract,
        "54cac40e" => RolloverEdit,
        "7b203f56" => RolloverFinish,
        "9377c70c" => RolloverLivePreview,
        "83501505" => RolloverDuplicateState,
        "217155e9" => RolloverDeleteState,
        "7f2204b5" => ShowHotspots,
        // Tools
        "e72e265e" => Options(OptionsPage::App),
        "ab216fdb" => Options(OptionsPage::Customization),
        "886df349" => Options(OptionsPage::Tools),
        "9b473401" => Options(OptionsPage::Global),
        "6ab6cfbd" => Options(OptionsPage::Workspaces),
        "c90df3e4" => Options(OptionsPage::Document),
        "e6024234" => SaveSettingsAsDefault,
        "5a54114e" => ColorManagement,
        "28e16db6" => ScriptEditor,
        "4d559de1" => RunScript,
        "f9215fff" => VsEditor,
        "9ea04cb9" => StartRecording,
        "972ee5c4" => PauseRecording,
        "655c0118" => StopRecording,
        "6009ed52" => CancelRecording,
        "706bee31" => RecordTemporary,
        "a3f0aa6d" => RunTemporary,
        // Window
        "ee06028c" => NewWindow,
        "24045d4b" => RefreshWindow,
        "74d4c886" => CloseWindow,
        "0f66a2f5" => Cascade,
        "a52e08a7" => TileH,
        "1b2d92c5" => TileV,
        "00498564" => CombineWindows,
        "8912c026" => DockWindow,
        "869a8eec" => WorkspaceNew,
        "d7574a40" => WorkspaceDelete,
        "1f3512de" => WorkspaceImport,
        "fc9a2b1b" => WorkspaceExport,
        "8645ceb3" | "4d1c6968" => ButtonSize(Bs::Small),
        "d4b11d74" | "2ac5afdf" => ButtonSize(Bs::Medium),
        "d447478a" | "9c2db071" => ButtonSize(Bs::Large),
        // a command bar's and a toolbar item's Properties…
        "224f44f6" => CommandBarsSetup,
        "0b3f627c" => CommandsSetup,
        "0e19571d" => ToggleToolbar(Toolbar::MenuBar),
        "00ee6da6" => ToggleToolbar(Toolbar::StatusBar),
        "d2fdc0d9" => ToggleToolbar(Toolbar::Toolbox),
        "24c9cf0c" => ToggleToolbar(Toolbar::PropertyBar),
        "7b90fb1f" => LockToolbars,
        "3f8a6b23" => AddColorsFromSelection,
        "fbc9febd" => AddColorsFromDocument,
        "3fb0838f" => CreatePaletteFromSelection,
        "eda72fb9" => CreatePaletteFromDocument,
        "7f5025f6" | "d1b116ab" => OpenPalette,
        "30f49925" => CloseAllPalettes,
        "20248253" => PaletteEditor,
        // Help
        "9ffda614" => ProductHelp,
        "77dc1224" => WelcomeScreen,
        "4eaca129" => QuickStart,
        "7ac8a763" => UserGuide,
        "e59e82df" => WhatsNew,
        "6e7e871a" => Updates,
        "08fa5bc1" => MessageSettings,
        "fafd28e8" => Feedback,
        "1eaa019d" => About,
        // Object ▸ Object Styles
        "13ac75cb" => NewStyleSetFrom,
        "9d6638c8" => ApplyToStyle,
        "d0987a09" => RevertToStyle,
        "61d08529" => BreakStyleLink,
        // Effects ▸ the bitmap effects (their table names the menu items)
        _ => return crate::fx::by_guid(&k).map(|(i, _)| BitmapEffect(i as u8)),
    })
}

fn doc_setting(app: &AnyVectorApp, f: fn(&vector_core::document::DocSettings) -> bool) -> Option<bool> {
    Some(app.doc().map(|d| f(&d.doc.settings)).unwrap_or(false))
}

/// Whether a toolbar is shown.
pub fn toolbar_on(app: &AnyVectorApp, tb: Toolbar) -> bool {
    let s = &app.settings;
    match tb {
        Toolbar::MenuBar => s.show_menu_bar,
        Toolbar::StatusBar => s.show_status_bar,
        Toolbar::Standard => s.show_standard,
        Toolbar::PropertyBar => s.show_property_bar,
        Toolbar::Toolbox => s.show_toolbox,
        _ => s.extra_toolbars.iter().any(|x| x == tb.name()),
    }
}

/// Whether a check or radio item is on.
pub fn checked(app: &AnyVectorApp, id: &str) -> Option<bool> {
    if let Some((_, c)) = crate::propbar::option_state(app, &key(id)) {
        return Some(c);
    }
    match key(id).as_str() {
        // Customize ▸ … ▸ Default / Image Only: how AnyVector shows toolbar buttons and items
        "ed8ad191" | "b412a0c2" | "8ba33eda" | "8f5a59d4" => return Some(true),
        "56b47cbb" | "192b2542" | "b5671d24" | "af261898" => return Some(false),
        _ => {}
    }
    // Lock Contents to PowerClip: AnyVector's PowerClip contents always move with the frame
    if key(id) == "6ea9b15a" {
        return Some(app.doc().is_some_and(|d| matches!(d.selection.as_slice(), [o] if d.doc.find(*o).is_some_and(|o| matches!(o.kind, ObjKind::PowerClip { .. }) && !o.powerclip_unlocked))));
    }
    if let Some(d) = docker(id) {
        return Some(app.dockers.open.contains(&d));
    }
    let tools = crate::tools::item_tools(id);
    if !tools.is_empty() {
        return Some(tools.contains(&app.tool));
    }
    if let Some(t) = tool(id) {
        return Some(app.tool == t);
    }
    match cmd(id)? {
        Cmd::ViewMode(m) => Some(app.doc().map(|d| d.view_mode == m).unwrap_or(false)),
        Cmd::TogglePageBorder => doc_setting(app, |s| s.show_page_border),
        Cmd::ToggleBleed => doc_setting(app, |s| s.show_bleed),
        Cmd::ToggleGrid => doc_setting(app, |s| s.show_grid),
        Cmd::TogglePixelGrid => doc_setting(app, |s| s.pixel_grid),
        Cmd::ToggleRulers => doc_setting(app, |s| s.show_rulers),
        Cmd::ToggleGuidelines => doc_setting(app, |s| s.show_guides),
        Cmd::SnapToGrid => doc_setting(app, |s| s.snap_to_grid),
        Cmd::SnapToGuides => doc_setting(app, |s| s.snap_to_guides),
        Cmd::SnapToObjects => doc_setting(app, |s| s.snap_to_objects),
        Cmd::SnapToPage => doc_setting(app, |s| s.snap_to_page),
        Cmd::SnapOff => doc_setting(app, |s| s.snap_off),
        Cmd::OverprintFill => Some(first_selected(app).is_some_and(|o| o.style.overprint_fill)),
        Cmd::OverprintOutline => Some(first_selected(app).is_some_and(|o| o.style.overprint_outline)),
        Cmd::OverprintBitmap => Some(first_selected(app).is_some_and(|o| o.style.overprint_bitmap)),
        Cmd::ObjectHinting => Some(first_selected(app).is_some_and(|o| o.hinting)),
        Cmd::SnapToPixels => doc_setting(app, |s| s.snap_to_pixels),
        Cmd::SnapToBaseline => doc_setting(app, |s| s.snap_to_baseline),
        Cmd::ToggleMultipageView => doc_setting(app, |s| s.multipage_view),
        Cmd::TogglePrintableArea => doc_setting(app, |s| s.show_printable_area),
        Cmd::ToggleBaselineGrid => doc_setting(app, |s| s.show_baseline_grid),
        Cmd::ToggleProofColors => Some(app.settings.proof_colors),
        Cmd::ToggleSimulateOverprints => Some(app.doc().is_some_and(|d| d.simulate_overprint)),
        Cmd::ToggleRasterizeEffects => Some(app.settings.rasterize_complex_effects),
        Cmd::ToggleAlignmentGuides => Some(app.settings.live_guides),
        Cmd::ToggleDynamicGuides => Some(app.settings.dynamic_guides),
        Cmd::ToggleDocNavigator => Some(app.settings.show_doc_navigator),
        Cmd::ToggleScrollbars => Some(app.settings.show_scrollbars),
        Cmd::ButtonSize(b) => Some(app.settings.button_size == b),
        Cmd::Node(No::Elastic) => Some(app.ts.elastic),
        Cmd::Node(No::Close) => Some(selected_path(app).is_some_and(|p| !p.subpaths.is_empty() && p.subpaths.iter().all(|s| s.closed))),
        Cmd::ToggleToolbar(tb) => Some(toolbar_on(app, tb)),
        Cmd::LockToolbars => Some(app.settings.lock_toolbars),
        Cmd::PauseRecording => Some(app.recording.as_ref().map(|r| r.paused).unwrap_or(false)),
        Cmd::RolloverLivePreview => Some(app.rollover_live),
        Cmd::ShowHotspots => Some(app.settings.show_hotspots),
        Cmd::ToggleNonPrinting => Some(app.settings.show_nonprinting),
        Cmd::ToggleTextFrames => Some(app.settings.show_text_frames),
        Cmd::ToggleHyphenation => Some(app.doc().is_some() && crate::dialogs::text::first_text(app).hyphenate),
        Cmd::AlignToBaselineGrid => Some(app.doc().is_some() && crate::dialogs::text::first_text(app).align_grid),
        Cmd::MakeWebCompatible => Some(app.doc().is_some() && crate::dialogs::text::first_text(app).web),
        // Bitmaps ▸ Mode: the mode of the selected bitmap; Inflate Bitmap ▸ Auto Inflate
        Cmd::BitmapMode(m) => Some(crate::dialogs::bitmaps::selected_mode(app) == Some(m)),
        Cmd::AutoInflateBitmap => Some(app.doc().is_some_and(|d| !d.doc.bitmap_no_inflate)),
        _ => None,
    }
}

/// Enabled state that depends on more than the selection type: undo history, clipboard,
/// page count, recording state, and so on. `None` keeps the reference state.
pub fn enabled(app: &AnyVectorApp, id: &str) -> Option<bool> {
    let d = app.doc();
    let pages = d.map(|d| d.doc.pages.len()).unwrap_or(0);
    let sel = d.map(|d| d.selection.len()).unwrap_or(0);
    let rec = app.recording.is_some();
    let one = d.and_then(|d| match d.selection.as_slice() {
        [o] => d.doc.find(*o),
        _ => None,
    });
    let one_is = |f: fn(&ObjKind) -> bool| one.is_some_and(|o| f(&o.kind));
    if let Some((e, _)) = crate::propbar::option_state(app, &key(id)) {
        return Some(e);
    }
    // items that are no command of their own, or whose state is not the command's
    match key(id).as_str() {
        // Object ▸ Object Styles
        "13ac75cb" => return Some(sel > 0),
        "9d6638c8" | "d0987a09" | "61d08529" => return Some(crate::dockers::ostyles::linked_style(app).is_some()),
        // PowerClip ▸ Fit Contents, Lock Contents to PowerClip, Extract Contents, Edit PowerClip
        "78030281" | "6ea9b15a" | "6ad732d1" | "cd3ae51f" | "273d95de" | "a369e3ae" | "56a4aae7" | "8ab3326a" => return Some(one_is(|k| matches!(k, ObjKind::PowerClip { .. }))),
        // Hide ▸ Show: hidden objects in the selection
        "edd19eb3" => return Some(d.is_some_and(|d| d.selection.iter().any(|id| d.doc.find(*id).is_some_and(|o| !o.visible)))),
        // Finish Editing PowerClip (the right-click and the toolbar ones)
        "22eabb92" | "1b67872b" => return Some(d.is_some_and(|d| !d.focus.is_empty() && d.symbol_edit.is_none() && d.rollover_edit.is_none())),
        // Pixel Grid and Snap To ▸ Pixels work in Pixels view only
        "64c626a4" | "e0c557dc" => return Some(d.is_some_and(|d| d.view_mode == Vm::Pixels)),
        // Align to Baseline Grid: paragraph text and tables
        "38f80f45" => return Some(d.is_some() && sel == 0 || one_is(|k| matches!(k, ObjKind::Table(_)) || matches!(k, ObjKind::Text(t) if matches!(t.kind, vector_core::text::TextKind::Paragraph { .. })))),
        // Insert Placeholder Text: tables and empty paragraph frames
        "4aa2e1ed" => return Some(one_is(|k| matches!(k, ObjKind::Table(_)) || matches!(k, ObjKind::Text(t) if matches!(t.kind, vector_core::text::TextKind::Paragraph { .. }) && t.char_count() == 0))),
        // Wrap Paragraph Text, Wrap Connector Line: not for dimension lines
        "55f2e8c4" | "c40ff90e" if one_is(|k| matches!(k, ObjKind::Dimension(_))) => return Some(false),
        // PowerClip Inside…: needs another object to be the container
        "da1efe7f" | "3f1c813c" => return Some(sel > 0 && d.is_some_and(|d| d.doc.pages.get(d.page).into_iter().flat_map(|p| p.layers.iter()).flat_map(|l| l.objects.iter()).any(|o| !d.selection.contains(&o.id)))),
        _ => {}
    }
    Some(match cmd(id)? {
        Cmd::Undo => d.is_some_and(|d| d.history.can_undo()),
        Cmd::WorkspaceDelete => crate::workspaces::is_custom(&app.settings, &app.settings.workspace),
        Cmd::Redo => d.is_some_and(|d| d.history.can_redo()),
        Cmd::Repeat => d.is_some() && app.last_cmd.is_some(),
        Cmd::Revert => d.is_some_and(|d| d.history.is_modified() && d.path.is_some()),
        Cmd::Paste | Cmd::PasteSpecial | Cmd::PasteInView => d.is_some() && (!app.clip.objects.is_empty() || crate::exec::system_clipboard_has_content()),
        Cmd::BringIntoFocus | Cmd::PreviewSelectedOnly => sel > 0,
        Cmd::ExitFocusMode => d.is_some_and(|d| !d.focus.is_empty()),
        Cmd::CopyPropertiesFrom => sel > 0,
        Cmd::Save => d.is_some_and(|d| d.history.is_modified() || d.path.is_none()),
        Cmd::ZoomToSelected => sel > 0,
        Cmd::Cut | Cmd::Copy | Cmd::Delete => sel > 0,
        Cmd::DeletePage | Cmd::DeletePageNow | Cmd::GoToPage => pages > 1,
        // Merge: a block of more than one cell; Unmerge: merged cells in the block; Split: one cell
        Cmd::TableMerge => crate::dialogs::table::selected_range(app).is_some_and(|(t, g)| t.anchors_in(g).len() > 1),
        Cmd::TableUnmerge => crate::dialogs::table::selected_range(app).is_some_and(|(t, g)| t.anchors_in(g).iter().any(|(r, c)| t.cell(*r, *c).rspan > 1 || t.cell(*r, *c).cspan > 1)),
        Cmd::TableSplit(_) => crate::dialogs::table::selected_range(app).is_some_and(|(t, g)| t.anchors_in(g).len() == 1),
        Cmd::TableSelect(_) => crate::dialogs::table::selected_range(app).is_some(),
        Cmd::TextToTable => one_is(|k| matches!(k, ObjKind::Text(_))),
        Cmd::TableToText | Cmd::TableDeleteTable => one_is(|k| matches!(k, ObjKind::Table(_))),
        // Object
        Cmd::CreateArrowhead => one.is_some_and(|o| o.bounds().width() > 0.0) && !one_is(|k| matches!(k, ObjKind::Image(_) | ObjKind::Table(_))),
        Cmd::CreatePattern | Cmd::InsertQrCode | Cmd::InsertBarcode => d.is_some(),
        Cmd::ValidateBarcode => d.is_some_and(|d| d.selection.iter().filter_map(|id| d.doc.find(*id)).any(|o| matches!(o.special, Some(vector_core::codes::Special::Barcode { .. })))),
        Cmd::AiGuided(_) => sel > 0,
        // Text
        Cmd::TextTabs | Cmd::TextColumns | Cmd::TextBullets | Cmd::TextDropCap | Cmd::ToggleHyphenation => d.is_some() && !one_is(is_artistic),
        Cmd::TextStatistics | Cmd::CustomHyphens | Cmd::ToggleNonPrinting | Cmd::ToggleTextFrames | Cmd::WritingTools(2) | Cmd::QuickCorrect | Cmd::WritingToolsSettings => d.is_some(),
        Cmd::InsertFormatCode(_) => d.is_some_and(|d| d.text_edit.is_some()),
        Cmd::EditText | Cmd::TextLanguage | Cmd::ChangeCaseDialog | Cmd::EncodeText => text_or_table(app),
        Cmd::WritingTools(_) => text_or_table(app) || one_is(|k| matches!(k, ObjKind::Dimension(_))),
        Cmd::FitTextToFrame => one_is(|k| matches!(k, ObjKind::Table(_)) || is_paragraph(k)),
        Cmd::MakeWebCompatible => one_is(is_paragraph) || d.is_some_and(|d| d.text_edit.as_ref().is_some_and(|te| d.doc.find(te.obj).is_some_and(|o| is_paragraph(&o.kind)))),
        Cmd::CreateEmptyFrame => one_is(|k| matches!(k, ObjKind::Rect(_) | ObjKind::Ellipse(_) | ObjKind::Polygon(_) | ObjKind::Path(_) | ObjKind::PowerClip { .. })),
        Cmd::LinkFrames => d.is_some_and(|d| {
            let objs: Vec<_> = d.selection.iter().filter_map(|id| d.doc.find(*id)).collect();
            objs.len() >= 2 && objs.iter().all(|o| is_paragraph(&o.kind)) && objs.iter().skip(1).all(|o| matches!(&o.kind, ObjKind::Text(t) if t.link_prev.is_none()))
        }),
        Cmd::UnlinkFrames => d.is_some_and(|d| d.selection.iter().filter_map(|id| d.doc.find(*id)).any(|o| matches!(&o.kind, ObjKind::Text(t) if t.link_next.is_some() || t.link_prev.is_some()))),
        Cmd::AlignToBaseline => one_is(|k| matches!(k, ObjKind::Text(_))),
        Cmd::SymbolEditMaster => d.is_some_and(|d| d.symbol_edit.is_none()) && crate::dialogs::object::selected_symbol(app).is_some(),
        Cmd::SymbolFinish => d.is_some_and(|d| d.symbol_edit.is_some()),
        Cmd::SymmetryCreate => d.is_some_and(|d| d.symmetry_edit.is_none() && d.persp_edit.is_none()) && !one.is_some_and(crate::dialogs::symmetry::is_symmetry),
        Cmd::SymmetryEdit | Cmd::SymmetryBreak => d.is_some_and(|d| d.symmetry_edit.is_none()) && crate::dialogs::symmetry::selected_symmetry(app).is_some(),
        Cmd::SymmetryFinish => d.is_some_and(|d| d.symmetry_edit.is_some()),
        Cmd::SymmetryRemove => d.is_some_and(|d| d.symmetry_edit.is_some()) || crate::dialogs::symmetry::selected_symmetry(app).is_some(),
        Cmd::PerspectiveDraw => d.is_some_and(|d| d.persp_edit.is_none() && d.symmetry_edit.is_none()),
        Cmd::PerspectiveEdit => d.is_some_and(|d| d.persp_edit.is_none()) && crate::dialogs::symmetry::selected_perspective(app).is_some(),
        Cmd::PerspectiveFinish => d.is_some_and(|d| d.persp_edit.is_some()),
        Cmd::MoveToPlane(p) => d.is_some_and(|d| d.persp_edit.as_ref().is_some_and(|pe| p != 4 || pe.kind == 0) && !d.selection.is_empty()),
        Cmd::InsertObject => true,
        Cmd::OleOpen => one.is_some_and(|o| matches!(o.special, Some(vector_core::codes::Special::Ole { .. }))),
        Cmd::SymbolBreakLink | Cmd::SymbolUpdateLink => crate::dialogs::object::symbol_linked(app),
        Cmd::SymbolExportLibrary => d.is_some_and(|d| !d.doc.symbols.is_empty()),
        Cmd::RolloverCreate => sel > 0 && !one.is_some_and(crate::dialogs::object::is_rollover) && d.is_some_and(|d| d.rollover_edit.is_none()),
        Cmd::RolloverExtract => d.is_some_and(|d| d.rollover_edit.is_none()) && crate::dialogs::object::selected_rollover(app).is_some(),
        Cmd::RolloverEdit => d.is_some_and(|d| d.rollover_edit.is_none()) && crate::dialogs::object::selected_rollover(app).is_some(),
        Cmd::RolloverFinish => d.is_some_and(|d| d.rollover_edit.is_some()),
        Cmd::RolloverLivePreview => d.is_some(),
        Cmd::RolloverDuplicateState => crate::dialogs::object::active_state(app).is_some_and(|s| s > 0),
        Cmd::RolloverDeleteState => crate::dialogs::object::active_state(app).is_some(),
        Cmd::InsertPageNumber(3) => pages > 1,
        Cmd::Group | Cmd::Combine => sel > 1,
        Cmd::StartRecording => d.is_some() && !rec,
        Cmd::PauseRecording | Cmd::StopRecording | Cmd::CancelRecording => rec,
        Cmd::RunTemporary => d.is_some() && !app.temp_script.is_empty(),
        Cmd::WelcomeScreen | Cmd::About | Cmd::Feedback => true,
        Cmd::ViewMode(crate::docview::ViewMode::PatternPreview) => d.is_some_and(|d| d.pattern.is_some()),
        Cmd::Node(op) => node_enabled(app, op),
        Cmd::PrintMergeEdit => d.is_some_and(|d| d.doc.print_merge.is_some()),
        // Bitmaps
        Cmd::RepeatLastTrace | Cmd::OpenLastTraceSettings => one_is(|k| matches!(k, ObjKind::Image(_))) && app.dialogs.bm.last_trace.is_some(),
        Cmd::CropBitmap => sel == 1 && crate::dialogs::bitmaps::is_cropped(app),
        Cmd::BitmapBreakLink | Cmd::BitmapUpdateLink => sel == 1 && crate::dialogs::bitmaps::link_of(app).is_some(),
        Cmd::BitmapMode(m) => sel == 1 && crate::dialogs::bitmaps::selected_mode(app).is_some_and(|c| c != m),
        Cmd::AutoInflateBitmap => d.is_some(),
        Cmd::PrintMergePerform | Cmd::PrintMergeNewDoc => d.is_some_and(|d| d.doc.print_merge.as_ref().is_some_and(|m| m.records.iter().any(|r| r.selected))),
        _ => return None,
    })
}

fn is_artistic(k: &ObjKind) -> bool {
    matches!(k, ObjKind::Text(t) if t.kind == vector_core::text::TextKind::Artistic)
}

fn is_paragraph(k: &ObjKind) -> bool {
    matches!(k, ObjKind::Text(t) if matches!(t.kind, vector_core::text::TextKind::Paragraph { .. }))
}

/// One text or table selected, or text being edited.
fn text_or_table(app: &AnyVectorApp) -> bool {
    app.doc().is_some_and(|d| {
        d.text_edit.is_some()
            || match d.selection.as_slice() {
                [o] => d.doc.find(*o).is_some_and(|o| matches!(o.kind, ObjKind::Text(_) | ObjKind::Table(_))),
                _ => false,
            }
    })
}

/// The first selected object.
fn first_selected(app: &AnyVectorApp) -> Option<&vector_core::Object> {
    let d = app.doc()?;
    d.doc.find(*d.selection.first()?)
}

/// The one curve selected, if that is the selection.
fn selected_path(app: &AnyVectorApp) -> Option<&vector_core::path::NodePath> {
    let d = app.doc()?;
    match d.selection.as_slice() {
        [o] => match &d.doc.find(*o)?.kind {
            ObjKind::Path(p) => Some(p),
            _ => None,
        },
        _ => None,
    }
}

/// Whether a node command applies to the selected nodes. To Line and To Curve act on the segment
/// that leaves a node, as AnyVector's Shape tool does.
fn node_enabled(app: &AnyVectorApp, op: No) -> bool {
    use vector_core::path::NodeKind;
    let (Some(d), Some(path)) = (app.doc(), selected_path(app)) else {
        return op == No::Elastic;
    };
    let nodes: Vec<(usize, usize)> = d.node_sel.iter().filter(|(id, ..)| d.selection.first() == Some(id)).map(|(_, s, n)| (*s, *n)).collect();
    let node = |s: usize, n: usize| path.subpaths.get(s).and_then(|sp| sp.nodes.get(n));
    // the segment leaving a node is a line (None: the last node of an open subpath has none)
    let line_out = |s: usize, n: usize| path.subpaths.get(s).filter(|sp| sp.closed || n + 1 < sp.nodes.len()).and_then(|sp| sp.nodes.get(n)).map(|x| x.line_out);
    let open_end = |s: usize, n: usize| path.subpaths.get(s).is_some_and(|sp| !sp.closed && (n == 0 || n + 1 == sp.nodes.len()));
    let kind_is_not = |k: NodeKind| nodes.iter().any(|&(s, n)| node(s, n).is_some_and(|x| x.kind != k));
    match op {
        No::Reverse | No::Close | No::Reduce | No::SelectAll | No::Elastic => true,
        No::Join => nodes.len() == 2 && nodes.iter().all(|&(s, n)| open_end(s, n)),
        No::ToLine => nodes.iter().any(|&(s, n)| line_out(s, n) == Some(false)),
        No::ToCurve => nodes.iter().any(|&(s, n)| line_out(s, n) == Some(true)),
        No::Cusp => kind_is_not(NodeKind::Cusp),
        No::Smooth => kind_is_not(NodeKind::Smooth),
        No::Symmetric => kind_is_not(NodeKind::Symmetric),
        No::Extract => !nodes.is_empty() && path.subpaths.len() > 1,
        No::Add | No::Delete | No::Break | No::ExtendClose => !nodes.is_empty(),
    }
}

/// The menu caption of a command, without mnemonic or trailing dots ("Duplicate").
pub fn label_of(c: Cmd) -> Option<String> {
    let it = super::bars().iter().flat_map(|b| b.items.iter()).find(|it| !it.text.is_empty() && cmd(it.id) == Some(c))?;
    Some(super::plain(it.text).trim_end_matches('.').trim_end_matches('…').to_string())
}

/// Captions set from the document: `%s` filled in ("Undo Delete"), and the ones the reference
/// words after the selection ("Convert To Paragraph Text").
/// Whether an item with the run-time visibility flag shows now.
pub fn visible(app: &AnyVectorApp, id: &str) -> bool {
    match cmd(id) {
        // View ▸ Pattern Preview: in pattern documents
        Some(Cmd::ViewMode(crate::docview::ViewMode::PatternPreview)) => app.doc().is_some_and(|d| d.pattern.is_some()),
        _ => true,
    }
}

pub fn caption(app: &AnyVectorApp, it: &super::Item) -> Option<String> {
    let d = app.doc();
    let one = d.and_then(|d| match d.selection.as_slice() {
        [o] => d.doc.find(*o),
        _ => None,
    });
    match key(it.id).as_str() {
        // Text ▸ Convert
        "7f650d0c" => {
            return match one.map(|o| &o.kind) {
                Some(ObjKind::Text(t)) => Some(match t.kind {
                    vector_core::text::TextKind::Artistic => "Con&vert To Paragraph Text".into(),
                    vector_core::text::TextKind::Paragraph { .. } => "Con&vert To Artistic Text".into(),
                }),
                _ => None,
            }
        }
        // Break Apart: a dimension line breaks apart into its lines and text
        "d5669070" if one.is_some_and(|o| matches!(o.kind, ObjKind::Dimension(_))) => return Some("&Break Dimension Apart".into()),
        _ => {}
    }
    if !it.text.contains("%s") {
        return None;
    }
    let what = match cmd(it.id) {
        Some(Cmd::Undo) => d.and_then(|d| d.history.undo_labels().first().cloned()),
        Some(Cmd::Redo) => d.and_then(|d| d.history.redo_labels().first().cloned()),
        Some(Cmd::Repeat) => app.last_cmd.and_then(label_of),
        _ => None,
    };
    Some(it.text.replace("%s", what.as_deref().unwrap_or("")).trim_end().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn id_prefixes_are_unique() {
        let mut seen = std::collections::HashMap::new();
        for b in super::super::bars() {
            for it in b.items {
                if it.id.len() < 36 {
                    continue;
                }
                let k = key(it.id);
                if let Some(prev) = seen.insert(k.clone(), it.id) {
                    assert!(prev.eq_ignore_ascii_case(it.id), "{prev} and {} share {k}", it.id);
                }
            }
        }
    }

    #[test]
    fn bound_ids_exist() {
        // every command binding names an item that is in the data
        for b in super::super::bars() {
            for it in b.items {
                let _ = cmd(it.id);
            }
        }
        assert_eq!(cmd("fa65d0c1-879b-4ef5-9465-af09e00e91ab"), Some(Cmd::New));
        assert_eq!(cmd("CD42798B-A81F-4eba-9CCE-7C035B5D26C1"), Some(Cmd::ExportForOffice));
    }

    /// Writes `target/unbound.txt`: every menu item (by menu path) that has no command yet.
    /// Run with `cargo test -p anyvector unbound_report -- --ignored`.
    #[test]
    #[ignore]
    fn unbound_report() {
        use super::super::{bar, bar_named, plain, Kind, NO_SUB};
        fn walk(b: u16, path: &str, out: &mut Vec<String>, depth: usize) {
            let Some(bar) = bar(b) else { return };
            for it in bar.items {
                let name = format!("{path} > {}", plain(super::super::label(it)));
                match it.kind {
                    Kind::Separator | Kind::Placeholder => {}
                    Kind::Flyout | Kind::ButtonFlyout if it.sub != NO_SUB && depth < 4 => walk(it.sub, &name, out, depth + 1),
                    _ if cmd(it.id).is_none() => out.push(format!("{name}	{}", it.id)),
                    _ => {}
                }
            }
        }
        let mut out = Vec::new();
        let (menu, _) = bar_named("Menu Bar").unwrap();
        for it in bar(menu).unwrap().items {
            walk(it.sub, &plain(it.text), &mut out, 0);
        }
        let n = out.len();
        std::fs::write(concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/unbound.txt"), out.join("
") + "
").unwrap();
        println!("{n} menu items without a command");
    }
}
