//! Tool identifiers, names, icons and shortcuts, and the reference toolbox item of each tool.

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug, serde::Serialize, serde::Deserialize)]
pub enum ToolId {
    Pick,
    FreehandPick,
    FreeTransform,
    Shape,
    Smooth,
    Smear,
    Twirl,
    AttractRepel,
    Smudge,
    Roughen,
    Crop,
    Knife,
    VirtualDelete,
    Eraser,
    Zoom,
    Pan,
    Freehand,
    TwoPointLine,
    Bezier,
    Pen,
    BSpline,
    Polyline,
    ThreePointCurve,
    LiveSketch,
    SmartDrawing,
    ArtisticMedia,
    Rectangle,
    Rect3,
    Ellipse,
    Ellipse3,
    Polygon,
    Star,
    Spiral,
    GraphPaper,
    CommonShapes,
    Text,
    Table,
    ParallelDim,
    HvDim,
    AngularDim,
    SegmentDim,
    Callout,
    StraightConnector,
    RightAngleConnector,
    RoundedConnector,
    EditAnchor,
    DropShadow,
    InnerShadow,
    BlockShadow,
    Contour,
    Blend,
    Distort,
    Envelope,
    Extrude,
    Transparency,
    ColorEyedropper,
    AttributesEyedropper,
    InteractiveFill,
    MeshFill,
    SmartFill,
    PainterlyBrush,
}

pub struct ToolInfo {
    pub id: ToolId,
    pub name: &'static str,
    pub icon: &'static str,
    pub shortcut: &'static str,
    pub hint: &'static str,
}

macro_rules! t {
    ($id:ident, $name:expr, $icon:expr, $sc:expr, $hint:expr) => {
        ToolInfo { id: ToolId::$id, name: $name, icon: $icon, shortcut: $sc, hint: $hint }
    };
}

pub const TOOLS: &[ToolInfo] = &[
    t!(Pick, "Pick", "pick", "", "Click an object twice for rotating/skewing; dbl-clicking tool selects all objects; Shift+click multi-selects; Alt+click digs; Ctrl+click selects in a group"),
    t!(FreehandPick, "Freehand Pick", "freehand_pick", "", "Drag around objects to select them with a freehand marquee"),
    t!(FreeTransform, "Free Transform", "free_transform", "", "Drag to rotate, reflect, scale or skew the selection freely"),
    t!(Shape, "Shape", "shape", "F10", "Click a curve object to select it; drag nodes to reshape; double-click a curve to add a node"),
    t!(Smooth, "Smooth", "smooth", "", "Drag along the edge of a curve object to smooth it"),
    t!(Smear, "Smear", "smear", "", "Drag along the outline to smear (distort) it"),
    t!(Twirl, "Twirl", "twirl", "", "Click and hold over the edge of an object to add twirls"),
    t!(AttractRepel, "Attract and Repel", "attract_repel", "", "Click and hold to attract nodes; hold Ctrl to repel"),
    t!(Smudge, "Smudge", "smudge", "", "Drag across the edge of a curve to smudge it"),
    t!(Roughen, "Roughen", "roughen", "", "Drag along the outline of a curve to roughen it"),
    t!(Crop, "Crop", "crop", "", "Drag to define the area to keep; double-click inside or press Enter to crop"),
    t!(Knife, "Knife", "knife", "", "Click or drag across an object to split it"),
    t!(VirtualDelete, "Virtual Segment Delete", "virtual_delete", "", "Click a segment between intersections to delete it"),
    t!(Eraser, "Eraser", "eraser", "X", "Drag over a selected object to erase parts of it"),
    t!(Zoom, "Zoom", "zoom", "Z", "Click to zoom in; right-click to zoom out; drag to zoom to an area"),
    t!(Pan, "Pan", "pan", "H", "Drag to pan the drawing window"),
    t!(Freehand, "Freehand", "freehand", "F5", "Drag to draw a freehand curve; click start and end points to draw a straight line"),
    t!(TwoPointLine, "2-Point Line", "two_point_line", "", "Drag to draw a straight line between two points"),
    t!(Bezier, "Bézier", "bezier", "", "Click to add straight segments; drag to add curved segments; double-click or press Enter to finish"),
    t!(Pen, "Pen", "pen", "", "Click to add points, drag to create curves; double-click to finish"),
    t!(BSpline, "B-Spline", "bspline", "", "Click to add control points; double-click to finish"),
    t!(Polyline, "Polyline", "polyline", "", "Click to add straight segments; drag to draw freehand; double-click to finish"),
    t!(ThreePointCurve, "3-Point Curve", "three_point_curve", "", "Drag to set the start and end points, then click to set the curvature"),
    t!(LiveSketch, "LiveSketch", "livesketch", "", "Sketch strokes; nearby strokes are merged into smooth curves"),
    t!(SmartDrawing, "Smart Drawing", "smart_drawing", "Shift+S", "Draw a shape freehand; it is recognized and converted into a basic shape"),
    t!(ArtisticMedia, "Artistic Media", "artistic_media", "I", "Drag to draw a brush, sprayer, calligraphic or pressure stroke"),
    t!(Rectangle, "Rectangle", "rectangle", "F6", "Drag to draw a rectangle; Ctrl constrains to a square; Shift draws from the center"),
    t!(Rect3, "3-Point Rectangle", "rect3", "", "Drag to set the baseline, then click to set the height"),
    t!(Ellipse, "Ellipse", "ellipse", "F7", "Drag to draw an ellipse; Ctrl constrains to a circle; Shift draws from the center"),
    t!(Ellipse3, "3-Point Ellipse", "ellipse3", "", "Drag to set the center line, then click to set the height"),
    t!(Polygon, "Polygon", "polygon", "Y", "Drag to draw a polygon; Ctrl constrains; Shift draws from the center"),
    t!(Star, "Star", "star", "", "Drag to draw a star; Ctrl constrains; Shift draws from the center"),
    t!(Spiral, "Spiral", "spiral", "A", "Drag to draw a spiral"),
    t!(GraphPaper, "Graph Paper", "graph_paper", "D", "Drag to draw a grid of rectangles"),
    t!(CommonShapes, "Common Shapes", "common_shapes", "", "Choose a shape on the property bar and drag to draw it"),
    t!(Text, "Text", "text", "F8", "Click to add artistic text; drag to add paragraph text"),
    t!(Table, "Table", "table", "", "Drag to draw a table"),
    t!(ParallelDim, "Parallel Dimension", "parallel_dim", "", "Drag to set the start and end points, then click to place the dimension line"),
    t!(HvDim, "Horizontal or Vertical Dimension", "hv_dim", "", "Drag to set the start and end points, then click to place the dimension line"),
    t!(AngularDim, "Angular Dimension", "angular_dim", "", "Click the vertex, then the two arms, then place the arc"),
    t!(SegmentDim, "Segment Dimension", "segment_dim", "", "Click a segment to measure it"),
    t!(Callout, "2-Leg Callout", "callout", "", "Drag to draw the leader, then click to set the shoulder and type the text"),
    t!(StraightConnector, "Connector", "straight_connector", "", "Drag from one object to another to connect them"),
    t!(RightAngleConnector, "Right-Angle Connector", "right_angle_connector", "", "Drag from one object to another to connect them with a right-angle line"),
    t!(RoundedConnector, "Rounded Right-Angle Connector", "rounded_connector", "", "Drag from one object to another to connect them with a rounded right-angle line"),
    t!(EditAnchor, "Anchor Editing", "edit_anchor", "", "Click an object to show its anchors; drag anchors to move them"),
    t!(DropShadow, "Shadow", "drop_shadow", "", "Drag from an object to add a drop shadow"),
    t!(InnerShadow, "Inner Shadow", "inner_shadow", "", "Drag inside an object to add an inner shadow"),
    t!(BlockShadow, "Block Shadow", "block_shadow", "", "Drag from an object to add a block shadow"),
    t!(Contour, "Contour", "contour", "", "Drag from the center to the edge, or from the edge out, to add a contour"),
    t!(Blend, "Blend", "blend", "", "Drag from one object to another to create a blend"),
    t!(Distort, "Distort", "distort", "", "Drag on an object to distort it (Push and Pull, Zipper or Twister)"),
    t!(Envelope, "Envelope", "envelope", "", "Click an object, then drag the envelope nodes to shape it"),
    t!(Extrude, "Extrude", "extrude", "", "Drag from an object to extrude it"),
    t!(Transparency, "Transparency", "transparency", "", "Click an object, then choose a transparency type on the property bar; drag to set a gradient transparency"),
    t!(ColorEyedropper, "Color Eyedropper", "eyedropper", "E", "Click to sample a color; then click an object to apply it (Shift+click applies to the outline)"),
    t!(AttributesEyedropper, "Attributes Eyedropper", "attributes_eyedropper", "", "Click an object to copy its properties, then click other objects to apply them"),
    t!(InteractiveFill, "Interactive Fill", "interactive_fill", "G", "Click an object to select it; drag to set a gradient fill"),
    t!(MeshFill, "Mesh Fill", "mesh_fill", "M", "Click an object to create a mesh; click a patch and choose a color from the palette"),
    t!(SmartFill, "Smart Fill", "smart_fill", "", "Click inside an enclosed area to create a filled object"),
    t!(PainterlyBrush, "Painterly Brush", "painterly", "", "Drag to paint with realistic brush strokes"),
];

pub fn info(id: ToolId) -> &'static ToolInfo {
    TOOLS.iter().find(|t| t.id == id).unwrap()
}

/// The reference toolbox item (first 8 characters of its id) of each Vector tool. Where one
/// reference tool stands for several Vector tools (the Connector and Shadow tools choose the kind
/// on their property bar) the first one is the tool the item starts with.
pub const TOOL_ITEMS: &[(&str, &[ToolId])] = &[
    ("f1aee54d", &[ToolId::Pick]),
    ("fb9a380a", &[ToolId::FreehandPick]),
    ("a7119a39", &[ToolId::FreeTransform]),
    ("ae9f4ba1", &[ToolId::Shape]),
    ("6f4dfa2b", &[ToolId::Smooth]),
    ("3faedeeb", &[ToolId::Smear]),
    ("c5dce714", &[ToolId::Twirl]),
    ("98797683", &[ToolId::AttractRepel]),
    ("3ceaf4f4", &[ToolId::Smudge]),
    ("0666b943", &[ToolId::Roughen]),
    ("c3157ceb", &[ToolId::Crop]),
    ("f2edc14d", &[ToolId::Knife]),
    ("11e3cea8", &[ToolId::VirtualDelete]),
    ("3eca6bde", &[ToolId::Eraser]),
    ("5f1c9019", &[ToolId::Zoom]),
    ("a34b8f68", &[ToolId::Pan]),
    ("a4561afd", &[ToolId::Freehand]),
    ("7c391635", &[ToolId::TwoPointLine]),
    ("9462a0c7", &[ToolId::Bezier]),
    ("6a131f1c", &[ToolId::Pen]),
    ("b0afd160", &[ToolId::BSpline]),
    ("faf6d5d5", &[ToolId::Polyline]),
    ("39243caf", &[ToolId::ThreePointCurve]),
    ("6307aedb", &[ToolId::PainterlyBrush]),
    ("5d09b650", &[ToolId::ArtisticMedia]),
    ("c63176a6", &[ToolId::LiveSketch]),
    ("66816bbe", &[ToolId::SmartDrawing]),
    ("1a105015", &[ToolId::Rectangle]),
    ("7593c3e1", &[ToolId::Rect3]),
    ("1810af9f", &[ToolId::Ellipse]),
    ("0b4cd8b0", &[ToolId::Ellipse3]),
    ("8a8221f7", &[ToolId::Polygon]),
    ("dda23c6e", &[ToolId::Star]),
    ("98987710", &[ToolId::Spiral]),
    ("1c4173b2", &[ToolId::CommonShapes]),
    ("0cedd43c", &[ToolId::GraphPaper]),
    ("927a2b18", &[ToolId::Text]),
    ("5507c778", &[ToolId::Table]),
    ("f1b16e8a", &[ToolId::ParallelDim]),
    ("f0a49225", &[ToolId::HvDim]),
    ("7e764ff0", &[ToolId::AngularDim]),
    ("4618f6d1", &[ToolId::SegmentDim]),
    ("71ed077c", &[ToolId::Callout]),
    ("2e3be6a6", &[ToolId::StraightConnector, ToolId::RightAngleConnector, ToolId::RoundedConnector]),
    ("60f47f20", &[ToolId::EditAnchor]),
    ("e65d1597", &[ToolId::DropShadow, ToolId::InnerShadow]),
    ("441d6b88", &[ToolId::Contour]),
    ("d72c98e8", &[ToolId::Blend]),
    ("f42edc69", &[ToolId::Distort]),
    ("1f16cb33", &[ToolId::Envelope]),
    ("c5555cfc", &[ToolId::Extrude]),
    ("249b7791", &[ToolId::BlockShadow]),
    ("b4b2ecec", &[ToolId::Transparency]),
    ("c109a053", &[ToolId::ColorEyedropper]),
    ("87bf2821", &[ToolId::AttributesEyedropper]),
    ("2c6e4e57", &[ToolId::InteractiveFill]),
    ("af052244", &[ToolId::SmartFill]),
    ("8f3b40f1", &[ToolId::MeshFill]),
];

/// The Vector tools a reference tool item stands for (empty where Vector has no such tool yet).
pub fn item_tools(id: &str) -> &'static [ToolId] {
    let k = id.get(..8).unwrap_or(id).to_ascii_lowercase();
    TOOL_ITEMS.iter().find(|(i, _)| *i == k).map(|(_, t)| *t).unwrap_or(&[])
}

/// The reference toolbox item (first 8 characters of its id) of a Vector tool.
pub fn tool_item(t: ToolId) -> &'static str {
    TOOL_ITEMS.iter().find(|(_, ts)| ts.contains(&t)).map(|(i, _)| *i).unwrap_or("")
}

impl ToolId {
    pub fn name(self) -> &'static str {
        info(self).name
    }
    pub fn icon(self) -> &'static str {
        info(self).icon
    }
    pub fn is_curve_creation(self) -> bool {
        matches!(
            self,
            ToolId::Freehand | ToolId::TwoPointLine | ToolId::Bezier | ToolId::Pen | ToolId::BSpline | ToolId::Polyline | ToolId::ThreePointCurve | ToolId::LiveSketch | ToolId::SmartDrawing | ToolId::ArtisticMedia
        )
    }
}
