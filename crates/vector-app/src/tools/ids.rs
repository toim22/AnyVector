//! Tool identifiers, names, icons, shortcuts and toolbox grouping (from the Quick Customize list).

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
    t!(Pick, "Pick", "pick", "", "Click an object to select it; Shift+click to select multiple objects; Alt+click to select hidden objects; Double-click to toggle rotate/skew handles"),
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
    t!(Callout, "3-Point Callout", "callout", "", "Drag to draw the leader, then click to set the shoulder and type the text"),
    t!(StraightConnector, "Straight-Line Connector", "straight_connector", "", "Drag from one object to another to connect them"),
    t!(RightAngleConnector, "Right-Angle Connector", "right_angle_connector", "", "Drag from one object to another to connect them with a right-angle line"),
    t!(RoundedConnector, "Rounded Right-Angle Connector", "rounded_connector", "", "Drag from one object to another to connect them with a rounded right-angle line"),
    t!(EditAnchor, "Edit Anchor", "edit_anchor", "", "Click an object to show its anchors; drag anchors to move them"),
    t!(DropShadow, "Drop Shadow", "drop_shadow", "", "Drag from an object to add a drop shadow"),
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

/// Quick Customize groups: (group title, tools). The toolbox shows one button per group with a flyout.
pub const GROUPS: &[(&str, &[ToolId])] = &[
    ("Pick Tools", &[ToolId::Pick, ToolId::FreehandPick, ToolId::FreeTransform]),
    ("Edit Shape Tools", &[ToolId::Shape, ToolId::Smooth, ToolId::Smear, ToolId::Twirl, ToolId::AttractRepel, ToolId::Smudge, ToolId::Roughen]),
    ("Crop Tools", &[ToolId::Crop, ToolId::Knife, ToolId::VirtualDelete, ToolId::Eraser]),
    ("Zoom Tools", &[ToolId::Zoom, ToolId::Pan]),
    (
        "Curve Tools",
        &[ToolId::Freehand, ToolId::TwoPointLine, ToolId::Bezier, ToolId::Pen, ToolId::BSpline, ToolId::Polyline, ToolId::ThreePointCurve, ToolId::LiveSketch, ToolId::SmartDrawing],
    ),
    ("Artistic Media Tools", &[ToolId::ArtisticMedia]),
    ("Rectangle Tools", &[ToolId::Rectangle, ToolId::Rect3]),
    ("Ellipse Tools", &[ToolId::Ellipse, ToolId::Ellipse3]),
    ("Object Tools", &[ToolId::Polygon, ToolId::Star, ToolId::Spiral, ToolId::GraphPaper, ToolId::CommonShapes]),
    ("Text Tools", &[ToolId::Text, ToolId::Table]),
    ("Dimension Tools", &[ToolId::ParallelDim, ToolId::HvDim, ToolId::AngularDim, ToolId::SegmentDim, ToolId::Callout]),
    ("Connector Tools", &[ToolId::StraightConnector, ToolId::RightAngleConnector, ToolId::RoundedConnector, ToolId::EditAnchor]),
    (
        "Interactive Tools",
        &[ToolId::DropShadow, ToolId::InnerShadow, ToolId::BlockShadow, ToolId::Contour, ToolId::Blend, ToolId::Distort, ToolId::Envelope, ToolId::Extrude],
    ),
    ("Transparency Tools", &[ToolId::Transparency]),
    ("Eyedropper Tools", &[ToolId::ColorEyedropper, ToolId::AttributesEyedropper]),
    ("Interactive Fill Tools", &[ToolId::InteractiveFill, ToolId::MeshFill, ToolId::SmartFill]),
    ("Painterly Brush Tools", &[ToolId::PainterlyBrush]),
];

/// Separators in the toolbox appear after these group indices (as in the screenshot).
pub const SEPARATOR_AFTER: &[usize] = &[1, 3, 8, 9, 11, 13, 15, 16];

pub fn group_of(id: ToolId) -> usize {
    GROUPS.iter().position(|(_, t)| t.contains(&id)).unwrap_or(0)
}

/// Tools that need a document to be usable.
pub fn needs_doc(_id: ToolId) -> bool {
    true
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
