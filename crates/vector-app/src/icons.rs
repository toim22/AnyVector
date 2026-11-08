//! Line-art icon set (24×24 grid), drawn to match the meaning and look of the reference
//! screenshots. Icons are SVG fragments rasterized with resvg into egui textures on demand.

use egui::{Color32, TextureHandle, TextureOptions};
use std::collections::HashMap;

const G: &str = "#c4c4c4"; // default line color
const GREEN: &str = "#36b24a";
const BLUE: &str = "#3d8fe6";
const RED: &str = "#e5484d";
const YELLOW: &str = "#f2c94c";

/// SVG body for an icon name (without the outer <svg> element).
pub fn body(name: &str) -> Option<String> {
    let s: &str = match name {
        "blank" => "",
        // ---------------------------------------------------------------- standard toolbar
        "new" => r##"<path d="M6 3.5h7l4 4V20.5H6z"/><path d="M13 3.5v4h4"/><rect x="13" y="1.5" width="9" height="9" fill="GREEN" stroke="none"/><path d="M17.5 3.3v5.4M14.8 6h5.4" stroke="#fff" stroke-width="1.6"/>"##,
        "open" => r##"<path d="M3 6.5h6l2 2h9.5v10.5H3z"/><path d="M3 10.5h17.5"/>"##,
        "save" => r##"<path d="M4 4h13l3 3v13H4z"/><path d="M8 4v5h7V4"/><rect x="7.5" y="13" width="9" height="7"/>"##,
        "open_cloud" => r##"<path d="M3 5.5h6l2 2h9.5v4"/><path d="M3 5.5V19h7"/><path d="M3 9.5h17.5"/><path d="M12.5 20.5h8a2.6 2.6 0 0 0 .3-5.2 3.6 3.6 0 0 0-6.9-.8 2.9 2.9 0 0 0-1.4 6z" fill="#c4c4c4"/>"##,
        "save_cloud" => r##"<path d="M4 3.5h12l3 3v6"/><path d="M4 3.5V19.5h6"/><path d="M7.5 3.5v4.5h6.5V3.5"/><path d="M12.5 21h8a2.6 2.6 0 0 0 .3-5.2 3.6 3.6 0 0 0-6.9-.8 2.9 2.9 0 0 0-1.4 6z" fill="#c4c4c4"/>"##,
        "print" => r##"<path d="M7 9V3.5h10V9"/><rect x="3.5" y="9" width="17" height="8" rx="1.5"/><path d="M7 14.5h10v6H7z"/>"##,
        "cut" => r##"<path d="M5 3.5h9.5l3 3V12"/><path d="M5 3.5v15"/><circle cx="9.5" cy="18" r="2.2"/><circle cx="16.5" cy="18" r="2.2"/><path d="M11 16.3l7-7.3M15 16.3l-7-7.3"/>"##,
        "copy" => r##"<rect x="8" y="7.5" width="11" height="13" rx="1"/><path d="M5 16.5V3.5h10"/>"##,
        "paste" => r##"<rect x="4.5" y="4.5" width="12" height="16" rx="1"/><path d="M8 4.5V3h5v1.5"/><rect x="11" y="10" width="9" height="11" rx="1" fill="#262626"/>"##,
        "undo" => r##"<path d="M8 6L4 10l4 4"/><path d="M4 10h11a5 5 0 0 1 0 10h-3"/>"##,
        "redo" => r##"<path d="M16 6l4 4-4 4"/><path d="M20 10H9a5 5 0 0 0 0 10h3"/>"##,
        "import" => r##"<path d="M4 13v7h16v-7"/><path d="M12 3v11M8 10l4 4 4-4"/>"##,
        "export" => r##"<path d="M4 13v7h16v-7"/><path d="M12 15V4M8 8l4-4 4 4"/>"##,
        "pdf" => r##"<path d="M5 2.5h8l4 4v4"/><path d="M5 2.5v9"/><rect x="2.5" y="13" width="19" height="8.5" rx="1" fill="#8a8a8a" stroke="none"/><text x="12" y="20" font-family="Segoe UI" font-weight="bold" font-size="7" fill="#262626" stroke="none" text-anchor="middle">PDF</text><path d="M13.5 3.5h7v7h-7z" fill="#5a5a5a" stroke="none"/><path d="M17 9V5M15.3 6.7L17 5l1.7 1.7" stroke="#d8d8d8"/>"##,
        "fullscreen" => r##"<path d="M4 8V4h4M16 4h4v4M20 16v4h-4M8 20H4v-4"/><rect x="7" y="7" width="10" height="10" fill="#c4c4c4"/>"##,
        "rulers" => r##"<path d="M4 4v16h16v-4H8V4z"/><path d="M4 8h2M4 12h2M12 18v2M16 18v2"/><ellipse cx="17" cy="7" rx="4" ry="2.5" fill="#c4c4c4" stroke="none"/><circle cx="17" cy="7" r="1.1" fill="#262626" stroke="none"/>"##,
        "grid" => r##"<path d="M4 9h.01M8 9h.01M12 9h.01M4 13h.01M8 13h.01M12 13h.01M16 13h.01M4 17h.01M8 17h.01M12 17h.01M16 17h.01M20 17h.01M4 21h.01M8 21h.01M12 21h.01M16 21h.01M20 21h.01" stroke-width="2"/><ellipse cx="17" cy="6" rx="4" ry="2.5" fill="#c4c4c4" stroke="none"/><circle cx="17" cy="6" r="1.1" fill="#262626" stroke="none"/>"##,
        "guidelines" => r##"<path d="M9 4v17M3 14h18" stroke-dasharray="2 2"/><ellipse cx="17" cy="6" rx="4" ry="2.5" fill="#c4c4c4" stroke="none"/><circle cx="17" cy="6" r="1.1" fill="#262626" stroke="none"/>"##,
        "snap_off" => r##"<path d="M5 21v-9a7 7 0 0 1 14 0v9h-4v-9a3 3 0 0 0-6 0v9z"/><path d="M5 17h4M15 17h4"/><path d="M16 2.5l5 5M21 2.5l-5 5" stroke="RED"/>"##,
        "snap" => r##"<path d="M5 21v-9a7 7 0 0 1 14 0v9h-4v-9a3 3 0 0 0-6 0v9z"/><path d="M5 17h4M15 17h4"/>"##,
        "ai" => r##"<path d="M10 4l1.6 4.4L16 10l-4.4 1.6L10 16l-1.6-4.4L4 10l4.4-1.6z" fill="#e8e8e8" stroke="none"/><path d="M18 3l.8 2.2L21 6l-2.2.8L18 9l-.8-2.2L15 6l2.2-.8z" fill="#e8e8e8" stroke="none"/><path d="M17 15l.6 1.4 1.4.6-1.4.6L17 19l-.6-1.4L15 17l1.4-.6z" fill="#e8e8e8" stroke="none"/>"##,
        "gear" => r##"<path d="M12 2.8l1.6.2.5 2.2 1.6.8 2-1.1 1.2 1.1 1.1 1.2-1.1 2 .7 1.6 2.3.5.2 1.6-.2 1.6-2.3.5-.7 1.6 1.1 2-1.1 1.2-1.2 1.1-2-1.1-1.6.7-.5 2.3-1.6.2-1.6-.2-.5-2.3-1.6-.7-2 1.1-1.2-1.1-1.1-1.2 1.1-2-.7-1.6-2.3-.5L2.8 12l.2-1.6 2.3-.5.7-1.6-1.1-2 1.1-1.2 1.2-1.1 2 1.1 1.6-.8.5-2.2z" fill="#e0e0e0" stroke="none"/><circle cx="12" cy="12" r="3.3" fill="#2b2b2b" stroke="none"/>"##,
        // ---------------------------------------------------------------- property bar
        "page_width" => r##"<path d="M5 3v4M19 3v4M5 5h14"/><path d="M7 10h7l3 3v8H7z"/>"##,
        "page_height" => r##"<path d="M3 6h4M3 21h4M5 6v15"/><path d="M10 8h7l3 3v10H10z"/>"##,
        "portrait" => r##"<path d="M7 3.5h7l4 4v13H7z"/><path d="M14 3.5v4h4"/>"##,
        "landscape" => r##"<path d="M3.5 6.5h13l4 4v8H3.5z"/><path d="M16.5 6.5v4h4"/>"##,
        "all_pages" => r##"<rect x="4" y="3.5" width="8" height="10" rx="1"/><rect x="8" y="7.5" width="8" height="10" rx="1" fill="#2b2b2b"/><rect x="12" y="11" width="8" height="10" rx="1" fill="#2b2b2b"/>"##,
        "current_page" => r##"<rect x="3.5" y="3.5" width="8" height="10" rx="1" stroke="#7a7a7a"/><rect x="12.5" y="3.5" width="8" height="10" rx="1" stroke="#7a7a7a"/><rect x="8" y="10" width="8" height="11" rx="1" fill="#2b2b2b"/>"##,
        "nudge" => r##"<path d="M12 3l-2.5 3h5zM12 21l-2.5-3h5zM3 12l3-2.5v5zM21 12l-3-2.5v5z" fill="#c4c4c4"/><rect x="9" y="9" width="6" height="6"/>"##,
        "dup_x" => r##"<rect x="3.5" y="3.5" width="9" height="10"/><path d="M7.5 17.5v-0M8 17.5H6.5v-0" /><path d="M7.5 13.5v4h4"/><text x="16.5" y="21" font-family="Segoe UI" font-size="9" fill="#c4c4c4" stroke="none">x</text>"##,
        "dup_y" => r##"<rect x="3.5" y="3.5" width="9" height="10"/><path d="M7.5 13.5v4h4"/><text x="16" y="21" font-family="Segoe UI" font-size="9" fill="#c4c4c4" stroke="none">y</text>"##,
        "treat_filled" => r##"<rect x="4" y="4" width="15" height="15" stroke="#ffffff"/><rect x="2.8" y="2.8" width="2.4" height="2.4" fill="#ffffff" stroke="none"/><rect x="17.8" y="2.8" width="2.4" height="2.4" fill="#ffffff" stroke="none"/><rect x="2.8" y="17.8" width="2.4" height="2.4" fill="#ffffff" stroke="none"/><path d="M14 12.5v9l2.2-2 1.6 3.3 1.4-.7-1.6-3.2h3z" fill="#ffffff" stroke="#1f4e8c" stroke-width=".8"/>"##,
        "scale_outline" => r##"<path d="M3 7V3h4M3 17v4h4M17 3h-1M21 17v4h-4" stroke-dasharray="0"/><circle cx="10.5" cy="13.5" r="3.5"/><path d="M13 11l6-6 2 2-6 6z"/>"##,
        "transform_fill" => r##"<path d="M3 7V3h4M3 17v4h4M21 17v4h-4"/><circle cx="10.5" cy="13.5" r="3.5"/><path d="M15 6l2 2 4-4"/>"##,
        "plus" => r##"<path d="M12 4v16M4 12h16" stroke="#e8e8e8" stroke-width="2"/>"##,
        "lock" => r##"<rect x="5" y="11" width="14" height="10" rx="1.5"/><path d="M8 11V8a4 4 0 0 1 8 0v3"/>"##,
        "unlock" => r##"<rect x="5" y="11" width="14" height="10" rx="1.5"/><path d="M8 11V8a4 4 0 0 1 7.5-2"/>"##,
        // ---------------------------------------------------------------- toolbox
        "pick" => r##"<path d="M6 3l12 9.5-5.5.5 3.5 6.5-2.3 1.2-3.4-6.6L6 18z" fill="#c4c4c4"/>"##,
        "freehand_pick" => r##"<path d="M9 4l9 7.5-4.2.4 2.6 5-1.8.9-2.6-5.1L9 15.5z" fill="#c4c4c4"/><path d="M3 20c2-3 4 1 6-2s4 1 6-1" />"##,
        "free_transform" => r##"<path d="M8 3l10 8-4.6.5 2.9 5.4-2 1-2.8-5.4L8 15.6z" fill="#c4c4c4"/><path d="M4 9a9 9 0 0 0 5 11" /><path d="M6.5 20.5L9 20l-.5-2.5"/>"##,
        "shape" => r##"<path d="M4 19C6 10 12 5 21 4"/><rect x="3" y="3" width="4" height="4" fill="#262626"/><path d="M5 7l5 8.5 2-3.5 4 .2z" fill="#c4c4c4"/><path d="M7 5h8"/>"##,
        "smooth" => r##"<path d="M3 18c4-1 3-8 8-8s5 6 10 4"/><path d="M14 3l3 3-6 6-3-1 1-3z"/>"##,
        "smear" => r##"<path d="M3 18c3 0 4-3 7-3s5 3 11 1"/><path d="M8 3c3 0 5 2 5 5s-2 5-5 5"/>"##,
        "twirl" => r##"<path d="M12 12m-1 0a1 1 0 1 0 2 0a3 3 0 1 0-6 0a5 5 0 1 0 10 0a7 7 0 1 0-14 0"/>"##,
        "attract_repel" => r##"<circle cx="12" cy="12" r="8"/><path d="M12 7v10M7 12h10M9.5 9.5l5 5M14.5 9.5l-5 5" stroke-dasharray="1.5 1.5"/>"##,
        "smudge" => r##"<path d="M4 20c3-6 7-9 13-10"/><path d="M14 4l6 6-4 4-6-6z"/>"##,
        "roughen" => r##"<path d="M3 16l2-3 2 3 2-4 2 4 2-3 2 3 2-4 2 4 2-2"/><path d="M14 3l4 4-6 6-3-1 1-3z"/>"##,
        "crop" => r##"<path d="M7 3v14h14"/><path d="M3 7h14v14"/><path d="M17 17l3 3" stroke="#262626"/>"##,
        "knife" => r##"<path d="M4 20L18 6l2 1-12 13z"/><path d="M15 9l-4-4 3-2 4 4"/>"##,
        "virtual_delete" => r##"<path d="M4 18l6-12 6 12"/><path d="M14 4l6 6M20 4l-6 6" stroke="RED"/>"##,
        "eraser" => r##"<path d="M4 16l9-9 6 6-6 6H8z"/><path d="M8.5 11.5l6 6" /><path d="M13 21h7"/>"##,
        "zoom" => r##"<circle cx="10" cy="10" r="6.5"/><path d="M15 15l6 6"/>"##,
        "pan" => r##"<path d="M8 13V5.5a1.5 1.5 0 0 1 3 0V12V4a1.5 1.5 0 0 1 3 0v8V5.5a1.5 1.5 0 0 1 3 0V13v-4a1.5 1.5 0 0 1 3 0v6a6 6 0 0 1-6 6h-1.5c-2 0-3.3-.8-4.5-2.2L4.5 14a1.5 1.5 0 0 1 2.3-1.9z"/>"##,
        "freehand" => r##"<path d="M6 3v6M3 6h6" stroke-width="1.2"/><path d="M5 20c0-4 1-6 2-6s1 6 2.5 6 1-6 2.5-6 1 6 2.5 6 1-6 2.5-6 1.5 3 2 5"/>"##,
        "two_point_line" => r##"<path d="M5 19L19 5"/><rect x="3" y="17" width="4" height="4" fill="#262626"/><rect x="17" y="3" width="4" height="4" fill="#262626"/>"##,
        "bezier" => r##"<path d="M4 20C6 8 14 16 20 4"/><path d="M2 14h8M14 8h8" stroke="#8a8a8a"/><rect x="2.5" y="18.5" width="3" height="3" fill="#262626"/><rect x="18.5" y="2.5" width="3" height="3" fill="#262626"/>"##,
        "pen" => r##"<path d="M12 3l6 9-4 9h-4l-4-9z"/><path d="M12 3v8"/><circle cx="12" cy="12.5" r="1.5"/>"##,
        "bspline" => r##"<path d="M4 18C8 4 16 4 20 18"/><path d="M4 18L8 6h8l4 12" stroke="#8a8a8a" stroke-dasharray="2 1.5"/>"##,
        "polyline" => r##"<path d="M3 18l5-9 5 6 4-10 4 7"/>"##,
        "three_point_curve" => r##"<path d="M4 18C6 6 18 6 20 18"/><circle cx="4" cy="18" r="1.5" fill="#c4c4c4"/><circle cx="20" cy="18" r="1.5" fill="#c4c4c4"/><circle cx="12" cy="9" r="1.5" fill="#c4c4c4"/>"##,
        "livesketch" => r##"<path d="M3 17c3-3 5-3 7-1s4 2 6-1 3-4 5-4" /><path d="M3 13c3-3 5-3 7-1" stroke="#8a8a8a"/><path d="M14 4l5 5" /><path d="M18 3l3 3-9 9h-3v-3z"/>"##,
        "smart_drawing" => r##"<path d="M3 19c2-6 4-2 6-7s4-1 6-5"/><path d="M12 13l5-5 4 4-5 5z" fill="#c4c4c4"/><path d="M12 13l-1 5 5-1"/>"##,
        "artistic_media" => r##"<path d="M3 21c2 0 4-1 5-3l-3-3c-2 1-2 3-2 6z" fill="#c4c4c4"/><path d="M6.5 15.5L19 3l2 2L8.5 17.5"/>"##,
        "rectangle" => r##"<rect x="4" y="5" width="16" height="14"/>"##,
        "rect3" => r##"<path d="M4 15L12 4l8 6-8 11z"/><circle cx="4" cy="15" r="1.5" fill="#c4c4c4"/><circle cx="12" cy="4" r="1.5" fill="#c4c4c4"/><circle cx="20" cy="10" r="1.5" fill="#c4c4c4"/>"##,
        "ellipse" => r##"<circle cx="12" cy="12" r="8.5"/>"##,
        "ellipse3" => r##"<ellipse cx="12" cy="12" rx="9" ry="5.5" transform="rotate(-30 12 12)"/><circle cx="4.5" cy="16.5" r="1.5" fill="#c4c4c4"/><circle cx="19.5" cy="7.5" r="1.5" fill="#c4c4c4"/><circle cx="14.5" cy="16" r="1.5" fill="#c4c4c4"/>"##,
        "polygon" => r##"<path d="M8 4h8l4.5 8-4.5 8H8l-4.5-8z"/>"##,
        "star" => r##"<path d="M12 3l2.6 5.6 6.1.7-4.5 4.2 1.2 6.1L12 16.6l-5.4 3 1.2-6.1-4.5-4.2 6.1-.7z"/>"##,
        "complex_star" => r##"<path d="M12 3l3 15.5L4 8.5h16L9 18.5z"/>"##,
        "spiral" => r##"<path d="M12 12a1.5 1.5 0 0 1 3 0 3 3 0 0 1-6 0 4.5 4.5 0 0 1 9 0 6 6 0 0 1-12 0 7.5 7.5 0 0 1 15 0"/>"##,
        "graph_paper" => r##"<rect x="3.5" y="3.5" width="17" height="17"/><path d="M9.2 3.5v17M14.8 3.5v17M3.5 9.2h17M3.5 14.8h17"/>"##,
        "common_shapes" => r##"<path d="M4 20l5-9 5 9z"/><circle cx="16" cy="8" r="4"/><path d="M12 13h8v7"/>"##,
        "text" => r##"<path d="M4 21L11 3h2l7 18M7 14h10"/>"##,
        "table" => r##"<rect x="3.5" y="4.5" width="17" height="15"/><path d="M3.5 9.5h17M3.5 14.5h17M9 4.5v15M15 4.5v15"/>"##,
        "parallel_dim" => r##"<path d="M5 19L19 5"/><path d="M3 15l4 4M15 3l4 4" stroke="#8a8a8a"/>"##,
        "hv_dim" => r##"<path d="M4 8v12M20 8v12M4 14h16"/><path d="M7 11l-3 3 3 3M17 11l3 3-3 3"/>"##,
        "angular_dim" => r##"<path d="M4 20h16M4 20L16 6"/><path d="M13 20a9 9 0 0 0-2.5-6.3"/>"##,
        "segment_dim" => r##"<path d="M3 18l6-10 6 6 6-9"/><path d="M3 21h12" stroke="#8a8a8a"/>"##,
        "callout" => r##"<path d="M4 20l6-8h10"/><circle cx="4" cy="20" r="1.5" fill="#c4c4c4"/><path d="M13 9h6" stroke="#8a8a8a"/>"##,
        "connector" => r##"<rect x="3" y="14" width="6" height="6" rx="1" fill="#c4c4c4" transform="rotate(-20 6 17)"/><rect x="15" y="4" width="6" height="6" rx="1" fill="#c4c4c4" transform="rotate(-20 18 7)"/><path d="M8 15l9-7" stroke-width="2"/>"##,
        "straight_connector" => r##"<rect x="2.5" y="15.5" width="5" height="5" fill="#262626"/><rect x="16.5" y="3.5" width="5" height="5" fill="#262626"/><path d="M7.5 15.5l9-7"/>"##,
        "right_angle_connector" => r##"<rect x="2.5" y="15.5" width="5" height="5" fill="#262626"/><rect x="16.5" y="3.5" width="5" height="5" fill="#262626"/><path d="M7.5 18H12V6h4.5"/>"##,
        "rounded_connector" => r##"<rect x="2.5" y="15.5" width="5" height="5" fill="#262626"/><rect x="16.5" y="3.5" width="5" height="5" fill="#262626"/><path d="M7.5 18H9a3 3 0 0 0 3-3V9a3 3 0 0 1 3-3h1.5"/>"##,
        "edit_anchor" => r##"<rect x="5" y="5" width="12" height="12"/><circle cx="11" cy="5" r="1.8" fill="#c4c4c4"/><circle cx="17" cy="11" r="1.8" fill="#c4c4c4"/><path d="M14 14l7 3.5-3 .8-.8 3z" fill="#c4c4c4"/>"##,
        "drop_shadow" => r##"<rect x="7" y="7" width="13" height="13" fill="#6a6a6a" stroke="none"/><rect x="4" y="4" width="13" height="13" fill="#262626"/>"##,
        "inner_shadow" => r##"<rect x="4" y="4" width="16" height="16"/><path d="M5 19V5h14" stroke="#7a7a7a" stroke-width="3"/>"##,
        "block_shadow" => r##"<path d="M10 10h10v10H10z" fill="#6a6a6a" stroke="none"/><path d="M4 4l6 6M14 4l6 6M4 14l6 6" stroke="#6a6a6a"/><rect x="4" y="4" width="10" height="10" fill="#262626"/>"##,
        "contour" => r##"<rect x="9" y="9" width="6" height="6"/><rect x="6" y="6" width="12" height="12"/><rect x="3" y="3" width="18" height="18"/>"##,
        "blend" => r##"<circle cx="5.5" cy="17" r="3" fill="#c4c4c4"/><circle cx="10" cy="13.5" r="3"/><circle cx="14" cy="10" r="3"/><rect x="15.5" y="3" width="6" height="6" fill="#c4c4c4"/>"##,
        "distort" => r##"<path d="M4 6c4-3 6 3 8 0s6-3 8 0v12c-2-3-6-3-8 0s-4 3-8 0z"/>"##,
        "envelope" => r##"<path d="M4 4c5 3 11 3 16 0v16c-5-3-11-3-16 0z"/><path d="M4 4h0M20 4h0M4 20h0M20 20h0" stroke-width="3"/>"##,
        "extrude" => r##"<path d="M4 9l5-5h11v11l-5 5"/><path d="M9 4v0M20 4l-5 5"/><rect x="4" y="9" width="11" height="11" fill="#262626"/><path d="M15 9l5-5"/>"##,
        "transparency" => r##"<path d="M4 4h4v4H4zM12 4h4v4h-4zM8 8h4v4H8zM16 8h4v4h-4zM4 12h4v4H4zM12 12h4v4h-4zM8 16h4v4H8zM16 16h4v4h-4z" fill="#c4c4c4" stroke="none"/><rect x="4" y="4" width="16" height="16"/>"##,
        "eyedropper" => r##"<path d="M14 4l6 6-2 2-6-6z" fill="#c4c4c4"/><path d="M13 7l-8.5 8.5L3 21l5.5-1.5L17 11"/>"##,
        "attributes_eyedropper" => r##"<path d="M14 4l6 6-2 2-6-6z" fill="#c4c4c4"/><path d="M13 7l-8.5 8.5L3 21l5.5-1.5L17 11"/><rect x="2" y="2" width="6" height="6" fill="BLUE" stroke="none"/>"##,
        "interactive_fill" => r##"<path d="M5 11l7-7 7 7-7 7z"/><path d="M5 11h14l-7 7z" fill="#c4c4c4"/><path d="M20 15c1 1.5 1.5 2.5 1.5 3.3a1.5 1.5 0 0 1-3 0c0-.8.5-1.8 1.5-3.3z" fill="#c4c4c4"/>"##,
        "mesh_fill" => r##"<path d="M4 4c5 2 11 2 16 0v16c-5-2-11-2-16 0z"/><path d="M12 4.8v14.4M4 12c5-2 11 2 16 0"/>"##,
        "smart_fill" => r##"<path d="M5 11l7-7 7 7-7 7z" fill="#c4c4c4"/><path d="M3 3l3 3M21 3l-3 3" stroke-dasharray="1.5 1.5"/>"##,
        "painterly" => r##"<path d="M16 3c-4 1-9 6-9 11 0 3 2 4 4 4" /><path d="M11 18c-2 0-3 1-4 3h4c2 0 3-1 3-3" fill="#c4c4c4"/><path d="M13 9c2-2 5-3 7-3-1 3-3 6-6 8"/><path d="M8 7l6 6" stroke="#8a8a8a"/>"##,
        // ---------------------------------------------------------------- file / menus
        "new_template" => r##"<path d="M5 3.5h10l4 4V20.5H5z"/><rect x="8" y="9" width="8" height="3"/><path d="M8 14.5h8M8 17h5"/>"##,
        "close_doc" => r##"<path d="M5 3.5h8l4 4v5"/><path d="M5 3.5v17h7"/><path d="M15 15l5 5M20 15l-5 5"/>"##,
        "close_all" => r##"<path d="M4 6.5V3.5h8"/><path d="M7 6.5h8l3 3V20.5H7z"/><path d="M15 14l6 6M21 14l-6 6" stroke="RED" stroke-width="2"/>"##,
        "save_as" => r##"<path d="M4 4h13l3 3v6"/><path d="M4 4v16h7"/><path d="M8 4v5h7V4"/><path d="M14 21l1-3 5-5 2 2-5 5z"/>"##,
        "save_template" => r##"<path d="M4 4h13l3 3v13H4z"/><rect x="7.5" y="12" width="9" height="8"/><path d="M9.5 15h5M9.5 17.5h3"/>"##,
        "revert" => r##"<path d="M6 3.5h7l4 4V20.5H6z"/><path d="M9 13a3.5 3.5 0 1 0 1-2.5L9 11.5"/><path d="M9 9v2.5h2.5"/>"##,
        "acquire" => r##"<rect x="3" y="9" width="18" height="8" rx="1"/><path d="M6 9l2-4h8l2 4"/><path d="M6 13h6"/>"##,
        "export_for" => r##"<path d="M4 13v7h16v-7"/><path d="M12 15V4M8 8l4-4 4 4"/><circle cx="18" cy="6" r="2.5" fill="BLUE" stroke="none"/>"##,
        "print_preview" => r##"<path d="M5 3.5h8l4 4v4"/><path d="M5 3.5v17h6"/><circle cx="15" cy="16" r="3.5"/><path d="M17.5 18.5l3 3"/>"##,
        "doc_props" => r##"<path d="M5 3.5h8l4 4v6"/><path d="M5 3.5v17h7"/><path d="M14.5 21c0-2 1-3 2.5-3s2.5 1 2.5 3M17 16.8a1.6 1.6 0 1 0 0-3.2 1.6 1.6 0 0 0 0 3.2z"/>"##,
        "exit" => r##"<path d="M12 3v8"/><path d="M7.5 6.5a7 7 0 1 0 9 0"/>"##,
        "options" => r##"<path d="M4 7h10M18 7h2M4 17h2M10 17h10"/><circle cx="16" cy="7" r="2"/><circle cx="8" cy="17" r="2"/>"##,
        "save_default" => r##"<path d="M12 3.5l1.3.2.4 1.8 1.3.6 1.6-.9 1 .9.9 1-.9 1.6.6 1.3 1.8.4.2 1.3-.2 1.3-1.8.4-.6 1.3.9 1.6-.9 1-1 .9-1.6-.9-1.3.6-.4 1.8-1.3.2-1.3-.2-.4-1.8-1.3-.6-1.6.9-1-.9-.9-1 .9-1.6-.6-1.3-1.8-.4L3.5 12l.2-1.3 1.8-.4.6-1.3-.9-1.6.9-1 1-.9 1.6.9 1.3-.6.4-1.8z"/><circle cx="12" cy="12" r="2.6"/>"##,
        "color_mgmt" => r##"<circle cx="9" cy="9" r="5"/><circle cx="15" cy="9" r="5"/><circle cx="12" cy="14.5" r="5"/>"##,
        "script" => r##"<path d="M6 3.5h12v17H6z"/><path d="M9 8l-2 2 2 2M15 8l2 2-2 2M13 7l-2 6"/>"##,
        "record" => r##"<circle cx="12" cy="12" r="6" fill="RED" stroke="none"/>"##,
        "pause" => r##"<path d="M9 6v12M15 6v12" stroke-width="2.4"/>"##,
        "stop" => r##"<rect x="7" y="7" width="10" height="10" fill="#c4c4c4"/>"##,
        "run" => r##"<path d="M8 5l11 7-11 7z" fill="GREEN" stroke="none"/>"##,
        "cancel" => r##"<circle cx="12" cy="12" r="8"/><path d="M6.5 17.5l11-11"/>"##,
        "help" => r##"<circle cx="12" cy="12" r="9"/><path d="M9.5 9.5a2.5 2.5 0 1 1 3.5 2.3c-.7.3-1 .9-1 1.6v.6"/><circle cx="12" cy="17" r=".6" fill="#c4c4c4"/>"##,
        "welcome" => r##"<path d="M3 11l9-7 9 7"/><path d="M5.5 9.5V20h13V9.5"/>"##,
        "video" => r##"<rect x="3" y="6" width="13" height="12" rx="1.5"/><path d="M16 10l5-3v10l-5-3z"/>"##,
        "learn" => r##"<path d="M2.5 9L12 4.5 21.5 9 12 13.5z"/><path d="M6 11v5c2 2 10 2 12 0v-5"/><path d="M21.5 9v6"/>"##,
        "book" => r##"<path d="M4 4.5h6a2 2 0 0 1 2 2V20a2 2 0 0 0-2-2H4z"/><path d="M20 4.5h-6a2 2 0 0 0-2 2V20a2 2 0 0 1 2-2h6z"/>"##,
        "whats_new" => r##"<path d="M12 3l2.4 5 5.6.8-4 3.9.9 5.5L12 15.6l-4.9 2.6.9-5.5-4-3.9 5.6-.8z"/>"##,
        "updates" => r##"<path d="M20 12a8 8 0 1 1-2.3-5.7"/><path d="M20 4v4.5h-4.5"/>"##,
        "message" => r##"<path d="M4 5h16v11H9l-5 4z"/>"##,
        "community" => r##"<circle cx="8" cy="8" r="3"/><circle cx="16" cy="8" r="3"/><path d="M2.5 19c0-3 2.5-5 5.5-5s5.5 2 5.5 5M10.5 19c0-3 2.5-5 5.5-5s5.5 2 5.5 5"/>"##,
        "support" => r##"<path d="M4 14v-2a8 8 0 0 1 16 0v2"/><rect x="3" y="13" width="4" height="6" rx="1"/><rect x="17" y="13" width="4" height="6" rx="1"/><path d="M19 19c0 1.5-2 2.5-5 2.5"/>"##,
        "feedback" => r##"<path d="M9 18h6M10 21h4"/><path d="M12 3a6 6 0 0 0-3.5 10.9V16h7v-2.1A6 6 0 0 0 12 3z"/>"##,
        "account" => r##"<circle cx="12" cy="8" r="4"/><path d="M4 21c0-4.5 3.5-7 8-7s8 2.5 8 7"/>"##,
        "refresh" => r##"<path d="M20 11a8 8 0 0 0-14.5-4.5L4 8"/><path d="M4 4v4h4"/><path d="M4 13a8 8 0 0 0 14.5 4.5L20 16"/><path d="M20 20v-4h-4"/>"##,
        // ---------------------------------------------------------------- new document presets
        "preset_print" => r##"<path d="M7 9V3.5h10V9"/><rect x="3" y="9" width="18" height="8.5" rx="1.5"/><path d="M7 14h10v6.5H7z" fill="#262626"/><path d="M9 17h6"/>"##,
        "preset_web" => r##"<circle cx="12" cy="12" r="9"/><ellipse cx="12" cy="12" rx="4" ry="9"/><path d="M3 12h18M4.5 7.5h15M4.5 16.5h15"/>"##,
        "preset_devices" => r##"<rect x="2.5" y="3.5" width="15" height="11" rx="1"/><path d="M7 18.5h6M10 14.5v4"/><rect x="14.5" y="9.5" width="7" height="11.5" rx="1.2" fill="#262626"/><path d="M17.3 19h1.4"/>"##,
        "preset_social" => r##"<path d="M3 4.5h11.5a1 1 0 0 1 1 1v6.5a1 1 0 0 1-1 1H8l-3.5 3v-3H3a1 1 0 0 1-1-1V5.5a1 1 0 0 1 1-1z"/><path d="M9.5 9.5h11a1 1 0 0 1 1 1V17a1 1 0 0 1-1 1H19v3l-3.5-3h-6a1 1 0 0 1-1-1v-6.5a1 1 0 0 1 1-1z" fill="#262626"/><path d="M12.5 13.5h6M12.5 15.5h4"/>"##,
        "preset_custom" => r##"<rect x="4" y="3.5" width="12" height="16" rx="1"/><path d="M14 13l6.5-6.5 2 2L16 15l-2.5.5z" fill="#262626"/>"##,
        "page_single" => r##"<path d="M6.5 3.5h8l3.5 3.5v13.5H6.5z"/><path d="M14.5 3.5V7H18"/>"##,
        "page_multi" => r##"<rect x="3.5" y="3.5" width="9" height="11" rx="1"/><rect x="11.5" y="9.5" width="9" height="11" rx="1" fill="#262626"/>"##,
        "add_circle" => r##"<circle cx="12" cy="12" r="9"/><path d="M12 7.5v9M7.5 12h9"/>"##,
        "remove_circle" => r##"<circle cx="12" cy="12" r="9"/><path d="M7.5 12h9"/>"##,
        "bug" => r##"<ellipse cx="12" cy="14" rx="5" ry="6.5"/><path d="M9 8.5a3 3 0 0 1 6 0"/><path d="M12 9v11.5"/><path d="M7 12H3.5M7 16.5l-3 2M7 9.5 4.5 7.5M17 12h3.5M17 16.5l3 2M17 9.5l2.5-2"/>"##,
        "about" => r##"<circle cx="12" cy="12" r="9"/><path d="M12 11v6"/><circle cx="12" cy="7.5" r=".7" fill="#c4c4c4"/>"##,
        "window_new" => r##"<rect x="3" y="4.5" width="18" height="15" rx="1"/><path d="M3 8.5h18"/><path d="M12 11v6M9 14h6" stroke="GREEN"/>"##,
        "window_close" => r##"<rect x="3" y="4.5" width="18" height="15" rx="1"/><path d="M3 8.5h18"/><path d="M9.5 11.5l5 5M14.5 11.5l-5 5"/>"##,
        "cascade" => r##"<rect x="3" y="3" width="11" height="9"/><rect x="6.5" y="7" width="11" height="9" fill="#262626"/><rect x="10" y="11" width="11" height="9" fill="#262626"/>"##,
        "tile_h" => r##"<rect x="3.5" y="3.5" width="17" height="7.5"/><rect x="3.5" y="13" width="17" height="7.5"/>"##,
        "tile_v" => r##"<rect x="3.5" y="3.5" width="7.5" height="17"/><rect x="13" y="3.5" width="7.5" height="17"/>"##,
        "combine" => r##"<rect x="3.5" y="5.5" width="17" height="14"/><path d="M3.5 9h17M8 5.5V9M12.5 5.5V9"/>"##,
        "dock" => r##"<rect x="3.5" y="3.5" width="17" height="17"/><path d="M14 3.5v17"/>"##,
        "window_list" => r##"<path d="M8 6h12M8 12h12M8 18h12"/><path d="M4 6h.01M4 12h.01M4 18h.01" stroke-width="2.4"/>"##,
        "workspace" => r##"<rect x="3" y="4" width="18" height="16" rx="1"/><path d="M3 8h18M7 8v12M17 8v12"/>"##,
        "toolbars" => r##"<rect x="3" y="4" width="18" height="16" rx="1"/><path d="M3 8h18M6 6h.01M9 6h.01M12 6h.01" />"##,
        "palettes" => r##"<path d="M12 3a9 9 0 1 0 0 18c1.5 0 2-1 2-2s-1-1.5-1-2.5 1-1.5 2-1.5h2a4 4 0 0 0 4-4c0-4.4-4-8-9-8z"/><circle cx="7.5" cy="11" r="1.3" fill="RED" stroke="none"/><circle cx="10" cy="7" r="1.3" fill="YELLOW" stroke="none"/><circle cx="15" cy="7.5" r="1.3" fill="GREEN" stroke="none"/><circle cx="8.5" cy="15.5" r="1.3" fill="BLUE" stroke="none"/>"##,
        "dockers" => r##"<rect x="3.5" y="3.5" width="17" height="17"/><path d="M13 3.5v17M13 9h7.5M13 14h7.5"/>"##,
        "purchase" => r##"<path d="M3 4h2.5l2.5 11h11l2-8H6.5"/><circle cx="9.5" cy="19" r="1.5"/><circle cx="17" cy="19" r="1.5"/>"##,
        // ---------------------------------------------------------------- dockers / objects
        "properties" => r##"<rect x="4" y="3.5" width="16" height="17" rx="1"/><path d="M7.5 8h9M7.5 12h9M7.5 16h5"/>"##,
        "objects" => r##"<path d="M12 4l9 4.5-9 4.5-9-4.5z"/><path d="M3 12.5l9 4.5 9-4.5M3 16.5l9 4.5 9-4.5"/>"##,
        "pages" => r##"<rect x="4" y="3.5" width="10" height="13"/><rect x="10" y="7.5" width="10" height="13" fill="#262626"/>"##,
        "comments" => r##"<path d="M4 4.5h16v11h-8l-5 4v-4H4z"/><path d="M8 8.5h8M8 11.5h5"/>"##,
        "object_data" => r##"<rect x="3.5" y="4.5" width="17" height="15"/><path d="M3.5 9h17M3.5 13.5h17M9 4.5v15"/>"##,
        "object_styles" => r##"<rect x="4" y="4" width="7" height="7" fill="BLUE" stroke="none"/><circle cx="16.5" cy="7.5" r="3.5"/><path d="M4 20l3.5-6 3.5 6z"/><rect x="13" y="13" width="7" height="7"/>"##,
        "symbols" => r##"<path d="M12 3l2.5 5h5.5l-4.5 3.5 2 5.5L12 13.5 6.5 17l2-5.5L4 8h5.5z"/><path d="M5 21h14"/>"##,
        "find" => r##"<circle cx="10" cy="10" r="6"/><path d="M14.5 14.5l6 6"/><path d="M7.5 10h5"/>"##,
        "transform" => r##"<rect x="4" y="8" width="12" height="12" stroke-dasharray="2 1.5"/><rect x="8" y="4" width="12" height="12"/>"##,
        "coordinates" => r##"<path d="M4 4v16h16"/><path d="M4 20l7-7"/><circle cx="14" cy="10" r="2.5"/><path d="M14 4v2M14 14v2M20 10h-2M10 10h-2" stroke="#8a8a8a"/>"##,
        "live_guides" => r##"<path d="M12 3v18M3 12h18" stroke-dasharray="2 2"/><rect x="7" y="7" width="10" height="10"/>"##,
        "align" => r##"<path d="M4 3v18"/><rect x="4" y="5" width="13" height="5" fill="#c4c4c4"/><rect x="4" y="14" width="8" height="5"/>"##,
        "align_left" => r##"<path d="M4 3v18"/><rect x="4" y="5" width="13" height="5" fill="#c4c4c4"/><rect x="4" y="14" width="8" height="5"/>"##,
        "align_hcenter" => r##"<path d="M12 3v18"/><rect x="5" y="5" width="14" height="5" fill="#c4c4c4"/><rect x="8" y="14" width="8" height="5"/>"##,
        "align_right" => r##"<path d="M20 3v18"/><rect x="7" y="5" width="13" height="5" fill="#c4c4c4"/><rect x="12" y="14" width="8" height="5"/>"##,
        "align_top" => r##"<path d="M3 4h18"/><rect x="5" y="4" width="5" height="13" fill="#c4c4c4"/><rect x="14" y="4" width="5" height="8"/>"##,
        "align_vcenter" => r##"<path d="M3 12h18"/><rect x="5" y="5" width="5" height="14" fill="#c4c4c4"/><rect x="14" y="8" width="5" height="8"/>"##,
        "align_bottom" => r##"<path d="M3 20h18"/><rect x="5" y="7" width="5" height="13" fill="#c4c4c4"/><rect x="14" y="12" width="5" height="8"/>"##,
        "fit_path" => r##"<path d="M3 18C7 6 17 6 21 18"/><rect x="5" y="9" width="3" height="3" fill="#c4c4c4"/><rect x="10.5" y="5.5" width="3" height="3" fill="#c4c4c4"/><rect x="16" y="9" width="3" height="3" fill="#c4c4c4"/>"##,
        "step_repeat" => r##"<rect x="3" y="3" width="7" height="7"/><rect x="8.5" y="8.5" width="7" height="7" fill="#262626"/><rect x="14" y="14" width="7" height="7" fill="#262626"/>"##,
        "corners" => r##"<path d="M4 20V11a7 7 0 0 1 7-7h9"/><path d="M8 20v-8a4 4 0 0 1 4-4h8" stroke="#8a8a8a"/>"##,
        "join_curves" => r##"<path d="M3 17c3-8 6-8 8-4M13 11c2 4 5 4 8-4"/><circle cx="12" cy="12" r="1.8" fill="BLUE" stroke="none"/>"##,
        "text_docker" => r##"<path d="M5 6V4h14v2M12 4v16M9 20h6"/>"##,
        "font_sampler" => r##"<text x="3" y="17" font-family="Times New Roman" font-size="14" fill="#c4c4c4" stroke="none">A</text><text x="12" y="19" font-family="Segoe UI" font-size="11" fill="#c4c4c4" stroke="none">a</text>"##,
        "glyphs" => r##"<rect x="3.5" y="3.5" width="17" height="17"/><path d="M3.5 12h17M12 3.5v17"/><text x="5" y="10.5" font-family="Segoe UI" font-size="6" fill="#c4c4c4" stroke="none">A</text><text x="14" y="10.5" font-family="Segoe UI" font-size="6" fill="#c4c4c4" stroke="none">&amp;</text><text x="5" y="19" font-family="Segoe UI" font-size="6" fill="#c4c4c4" stroke="none">%</text><text x="14" y="19" font-family="Segoe UI" font-size="6" fill="#c4c4c4" stroke="none">©</text>"##,
        "effects" => r##"<path d="M4 20L15 9"/><path d="M15 4v3M15 11v3M11 7.5h3M17 7.5h3M18 4.5l-1.5 1.5M18 10.5l-1.5-1.5" />"##,
        "color" => r##"<circle cx="12" cy="12" r="8.5"/><path d="M12 3.5v8.5l7.4 4.2" /><path d="M12 12L4.6 16.2"/>"##,
        "color_styles" => r##"<rect x="3.5" y="4" width="7" height="7" fill="RED" stroke="none"/><rect x="13.5" y="4" width="7" height="7" fill="BLUE" stroke="none"/><rect x="3.5" y="13" width="7" height="7" fill="YELLOW" stroke="none"/><rect x="13.5" y="13" width="7" height="7" fill="GREEN" stroke="none"/>"##,
        "color_proofing" => r##"<path d="M2.5 12s3.5-6 9.5-6 9.5 6 9.5 6-3.5 6-9.5 6-9.5-6-9.5-6z"/><circle cx="12" cy="12" r="3" fill="BLUE" stroke="none"/>"##,
        "assets" => r##"<rect x="3.5" y="3.5" width="7.5" height="7.5"/><rect x="13" y="3.5" width="7.5" height="7.5"/><rect x="3.5" y="13" width="7.5" height="7.5"/><rect x="13" y="13" width="7.5" height="7.5"/>"##,
        "tray" => r##"<path d="M3 13l3-8h12l3 8v6H3z"/><path d="M3 13h5l1 2h6l1-2h5"/>"##,
        "links" => r##"<path d="M10 14a4 4 0 0 0 5.7 0l3-3a4 4 0 0 0-5.7-5.7l-1.5 1.5"/><path d="M14 10a4 4 0 0 0-5.7 0l-3 3a4 4 0 0 0 5.7 5.7l1.5-1.5"/>"##,
        "sources" => r##"<path d="M4 6h16M4 12h10M4 18h12"/><circle cx="19" cy="15" r="2.5"/>"##,
        "views" => r##"<rect x="3" y="5" width="18" height="14" rx="1"/><circle cx="12" cy="12" r="3"/>"##,
        "history" => r##"<path d="M4 12a8 8 0 1 0 2.3-5.7L4 8.5"/><path d="M4 4v4.5h4.5"/><path d="M12 8v4l3 2"/>"##,
        "trace" => r##"<path d="M4 18c3-8 6-12 9-12s4 4 7 2"/><rect x="3" y="3" width="8" height="8" fill="#6a6a6a" stroke="none"/>"##,
        "bitmap" => r##"<rect x="3.5" y="4.5" width="17" height="15" rx="1"/><path d="M3.5 16l5-5 4 4 3-3 5 5"/><circle cx="15.5" cy="8.5" r="1.5"/>"##,
        "layer" => r##"<path d="M12 4l9 4.5-9 4.5-9-4.5z"/><path d="M3 13l9 4.5 9-4.5"/>"##,
        "layer_new" => r##"<path d="M12 4l9 4.5-9 4.5-9-4.5z"/><path d="M3 13l9 4.5 3-1.5"/><path d="M19 14v6M16 17h6" stroke="GREEN"/>"##,
        "page_new" => r##"<path d="M5 3.5h8l4 4v4"/><path d="M5 3.5v17h8"/><path d="M18 14v7M14.5 17.5h7" stroke="GREEN"/>"##,
        "delete" => r##"<path d="M4 6.5h16M9 6.5V4h6v2.5M6 6.5l1 14h10l1-14"/><path d="M10 10v7M14 10v7"/>"##,
        "eye" => r##"<path d="M2.5 12s3.5-6 9.5-6 9.5 6 9.5 6-3.5 6-9.5 6-9.5-6-9.5-6z"/><circle cx="12" cy="12" r="2.8"/>"##,
        "eye_off" => r##"<path d="M2.5 12s3.5-6 9.5-6 9.5 6 9.5 6-3.5 6-9.5 6-9.5-6-9.5-6z" stroke="#6a6a6a"/><path d="M4 4l16 16"/>"##,
        "printer_small" => r##"<path d="M7 9V4h10v5"/><rect x="4" y="9" width="16" height="7" rx="1"/><path d="M7 14h10v6H7z"/>"##,
        "no_print" => r##"<path d="M7 9V4h10v5"/><rect x="4" y="9" width="16" height="7" rx="1" stroke="#6a6a6a"/><path d="M4 4l16 16"/>"##,
        "group" => r##"<rect x="3" y="3" width="8" height="8"/><circle cx="16" cy="16" r="4.5"/><path d="M2 2h1M21 2h1M2 22h1M21 22h1" stroke-width="2.5"/>"##,
        "ungroup" => r##"<rect x="3" y="3" width="8" height="8"/><circle cx="16" cy="16" r="4.5"/>"##,
        "combine_obj" => r##"<path d="M3 3h11v6h7v12H10v-6H3z"/>"##,
        "break_apart" => r##"<rect x="3" y="3" width="8" height="8"/><rect x="13" y="13" width="8" height="8"/><path d="M14 4l6 6M20 4l-6 6" stroke="RED"/>"##,
        "to_curves" => r##"<path d="M4 18C6 8 18 16 20 6"/><rect x="2.5" y="16.5" width="3" height="3" fill="#262626"/><rect x="18.5" y="4.5" width="3" height="3" fill="#262626"/><rect x="10.5" y="10.5" width="3" height="3" fill="#262626"/>"##,
        "to_front" => r##"<rect x="3" y="3" width="10" height="10" stroke="#6a6a6a"/><rect x="9" y="9" width="12" height="12" fill="#c4c4c4"/>"##,
        "to_back" => r##"<rect x="9" y="9" width="12" height="12" stroke="#6a6a6a"/><rect x="3" y="3" width="10" height="10" fill="#c4c4c4"/>"##,
        "forward_one" => r##"<rect x="3" y="3" width="10" height="10"/><rect x="9" y="9" width="12" height="12" fill="#c4c4c4" stroke="none"/><path d="M17 5v-0" />"##,
        "back_one" => r##"<rect x="9" y="9" width="12" height="12"/><rect x="3" y="3" width="10" height="10" fill="#c4c4c4" stroke="none"/>"##,
        "weld" => r##"<path d="M3 9a6 6 0 0 1 11.6-2.1A6 6 0 1 1 9.4 17 6 6 0 0 1 3 9z" fill="#c4c4c4"/>"##,
        "trim" => r##"<circle cx="9" cy="10" r="6" stroke="#6a6a6a"/><path d="M14.6 7.9A6 6 0 1 1 9.5 16c3.2-.3 5.6-3 5.6-6.2a6 6 0 0 0-.5-1.9z" fill="#c4c4c4"/>"##,
        "intersect" => r##"<circle cx="9" cy="10" r="6"/><circle cx="15" cy="14" r="6"/><path d="M14.6 7.9a6 6 0 0 1-5.2 8.1 6 6 0 0 1 5.2-8.1z" fill="#c4c4c4"/>"##,
        "simplify" => r##"<circle cx="9" cy="10" r="6" fill="#c4c4c4"/><path d="M14.6 7.9A6 6 0 1 1 9.5 16" />"##,
        "front_minus_back" => r##"<circle cx="9" cy="10" r="6" stroke="#6a6a6a"/><path d="M15 8a6 6 0 1 1-5.6 8.1A6 6 0 0 0 15 8z" fill="#c4c4c4"/>"##,
        "back_minus_front" => r##"<path d="M9 4a6 6 0 0 1 5.6 3.9A6 6 0 0 0 9.4 16 6 6 0 1 1 9 4z" fill="#c4c4c4"/><circle cx="15" cy="14" r="6" stroke="#6a6a6a"/>"##,
        "boundary" => r##"<path d="M3 9a6 6 0 0 1 11.6-2.1A6 6 0 1 1 9.4 17 6 6 0 0 1 3 9z" stroke-dasharray="2 1.5"/>"##,
        "mirror_h" => r##"<path d="M12 3v18" stroke-dasharray="2 1.5"/><path d="M9 6L3 18h6zM15 6l6 12h-6z"/>"##,
        "mirror_v" => r##"<path d="M3 12h18" stroke-dasharray="2 1.5"/><path d="M6 9L18 3v6zM6 15l12 6v-6z"/>"##,
        "rotate" => r##"<path d="M19 12a7 7 0 1 1-2-4.9"/><path d="M17.5 3v4.5H13"/>"##,
        "powerclip" => r##"<circle cx="12" cy="12" r="8.5"/><rect x="7" y="9" width="10" height="6" fill="#c4c4c4" stroke="none"/>"##,
        "lens" => r##"<circle cx="12" cy="12" r="8"/><path d="M8 9a5 5 0 0 1 4-3"/>"##,
        "bevel" => r##"<rect x="3.5" y="3.5" width="17" height="17"/><rect x="7.5" y="7.5" width="9" height="9"/><path d="M3.5 3.5l4 4M20.5 3.5l-4 4M3.5 20.5l4-4M20.5 20.5l-4-4"/>"##,
        "perspective" => r##"<path d="M6 5h12l3 14H3z"/><path d="M12 5v14M4.5 12h15"/>"##,
        "fill_icon" => r##"<path d="M5 11l7-7 7 7-7 7z"/><path d="M5 11h14l-7 7z" fill="#c4c4c4"/><path d="M20 15c1 1.5 1.5 2.5 1.5 3.3a1.5 1.5 0 0 1-3 0c0-.8.5-1.8 1.5-3.3z" fill="#c4c4c4"/>"##,
        "outline_icon" => r##"<path d="M12 3l6 9-4 9h-4l-4-9z"/><path d="M12 3v8"/><circle cx="12" cy="12.5" r="1.5"/>"##,
        "doc_color" => r##"<rect x="3.5" y="4.5" width="17" height="15" rx="1"/><rect x="6" y="7" width="4" height="10" fill="RED" stroke="none"/><rect x="10" y="7" width="4" height="10" fill="GREEN" stroke="none"/><rect x="14" y="7" width="4" height="10" fill="BLUE" stroke="none"/>"##,
        "home" => r##"<path d="M3.5 11.5L12 4l8.5 7.5"/><path d="M6 9.5V20h4.5v-5h3v5H18V9.5"/>"##,
        "close_x" => r##"<path d="M6 6l12 12M18 6L6 18"/>"##,
        "chevron_down" => r##"<path d="M7 10l5 5 5-5"/>"##,
        "chevron_up" => r##"<path d="M7 14l5-5 5 5"/>"##,
        "chevron_right" => r##"<path d="M10 7l5 5-5 5"/>"##,
        "chevron_left" => r##"<path d="M14 7l-5 5 5 5"/>"##,
        "double_right" => r##"<path d="M6 7l5 5-5 5M13 7l5 5-5 5"/>"##,
        "no_color" => r##"<rect x="3.5" y="3.5" width="17" height="17" fill="#ffffff" stroke="#9a9a9a"/><path d="M4 20L20 4" stroke="RED" stroke-width="1.8"/>"##,
        "wireframe" => r##"<rect x="4" y="4" width="16" height="16" stroke-dasharray="0"/><path d="M4 4l16 16M20 4L4 20" stroke="#8a8a8a"/>"##,
        "zoom_in" => r##"<circle cx="10" cy="10" r="6.5"/><path d="M15 15l6 6M7 10h6M10 7v6"/>"##,
        "zoom_out" => r##"<circle cx="10" cy="10" r="6.5"/><path d="M15 15l6 6M7 10h6"/>"##,
        "zoom_selected" => r##"<circle cx="10" cy="10" r="6.5"/><path d="M15 15l6 6"/><rect x="7.5" y="7.5" width="5" height="5" fill="#c4c4c4" stroke="none"/>"##,
        "zoom_all" => r##"<circle cx="10" cy="10" r="6.5"/><path d="M15 15l6 6"/><path d="M7 8h2M11 8h2M7 12h2M11 12h2" />"##,
        "zoom_page" => r##"<circle cx="10" cy="10" r="6.5"/><path d="M15 15l6 6"/><rect x="7.5" y="6.5" width="5" height="7"/>"##,
        "zoom_width" => r##"<circle cx="10" cy="10" r="6.5"/><path d="M15 15l6 6M6.5 10h7M8 8.5L6.5 10 8 11.5M12 8.5l1.5 1.5-1.5 1.5"/>"##,
        "zoom_height" => r##"<circle cx="10" cy="10" r="6.5"/><path d="M15 15l6 6M10 6.5v7M8.5 8L10 6.5 11.5 8M8.5 12l1.5 1.5 1.5-1.5"/>"##,
        "bold" => r##"<path d="M7 4h6a4 4 0 0 1 0 8H7zM7 12h7a4 4 0 0 1 0 8H7z" stroke-width="2.2"/>"##,
        "italic" => r##"<path d="M10 4h8M6 20h8M14 4l-4 16"/>"##,
        "underline" => r##"<path d="M7 4v7a5 5 0 0 0 10 0V4M5 21h14"/>"##,
        "align_text_left" => r##"<path d="M4 6h16M4 10h10M4 14h16M4 18h10"/>"##,
        "align_text_center" => r##"<path d="M4 6h16M7 10h10M4 14h16M7 18h10"/>"##,
        "align_text_right" => r##"<path d="M4 6h16M10 10h10M4 14h16M10 18h10"/>"##,
        "align_text_justify" => r##"<path d="M4 6h16M4 10h16M4 14h16M4 18h16"/>"##,
        "align_text_none" => r##"<path d="M4 6h16M4 10h10M4 14h16M4 18h10" stroke="#7a7a7a"/><path d="M3 3l18 18"/>"##,
        "bullets" => r##"<path d="M9 6h11M9 12h11M9 18h11"/><circle cx="5" cy="6" r="1.2" fill="#c4c4c4"/><circle cx="5" cy="12" r="1.2" fill="#c4c4c4"/><circle cx="5" cy="18" r="1.2" fill="#c4c4c4"/>"##,
        "drop_cap" => r##"<path d="M13 6h7M13 10h7M4 14h16M4 18h16"/><text x="3" y="12" font-family="Segoe UI" font-weight="bold" font-size="10" fill="#c4c4c4" stroke="none">A</text>"##,
        "text_vertical" => r##"<path d="M17 4v16M14 17l3 3 3-3"/><path d="M4 6h8M8 6v12"/>"##,
        "text_horizontal" => r##"<path d="M4 17h16M17 14l3 3-3 3"/><path d="M6 4h8M10 4v9"/>"##,
        "dashes" => r##"<path d="M3 12h4M10 12h4M17 12h4"/>"##,
        "arrow_start" => r##"<path d="M9 12h12"/><path d="M3 12l6-4v8z" fill="#c4c4c4"/>"##,
        "arrow_end" => r##"<path d="M3 12h12"/><path d="M21 12l-6-4v8z" fill="#c4c4c4"/>"##,
        "node_add" => r##"<path d="M3 18C8 6 16 6 21 18"/><rect x="10.5" y="7.5" width="3" height="3" fill="#262626"/><path d="M17 3v5M14.5 5.5h5" stroke="GREEN"/>"##,
        "node_delete" => r##"<path d="M3 18C8 6 16 6 21 18"/><rect x="10.5" y="7.5" width="3" height="3" fill="#262626"/><path d="M14.5 5.5h5" stroke="RED"/>"##,
        "node_join" => r##"<path d="M3 18l7-8M14 10l7 8"/><rect x="10" y="8.5" width="4" height="4" fill="BLUE" stroke="none"/>"##,
        "node_break" => r##"<path d="M3 18l6-8M15 10l6 8"/><rect x="8" y="8" width="3" height="3" fill="#262626"/><rect x="13" y="8" width="3" height="3" fill="#262626"/>"##,
        "to_line" => r##"<path d="M4 18L20 6"/><rect x="2.5" y="16.5" width="3" height="3" fill="#262626"/><rect x="18.5" y="4.5" width="3" height="3" fill="#262626"/>"##,
        "to_curve" => r##"<path d="M4 18C6 6 14 14 20 6"/><rect x="2.5" y="16.5" width="3" height="3" fill="#262626"/><rect x="18.5" y="4.5" width="3" height="3" fill="#262626"/>"##,
        "cusp" => r##"<path d="M3 18l9-10 9 10"/><rect x="10.5" y="6.5" width="3" height="3" fill="#262626"/><path d="M6 7l6 1.5M18 7l-6 1.5" stroke="#8a8a8a"/>"##,
        "smooth_node" => r##"<path d="M3 18C6 6 18 6 21 18"/><rect x="10.5" y="7.5" width="3" height="3" fill="#262626"/><path d="M5 9h14" stroke="#8a8a8a"/>"##,
        "symmetric" => r##"<path d="M3 18C6 6 18 6 21 18"/><rect x="10.5" y="7.5" width="3" height="3" fill="#262626"/><path d="M7 9h10" stroke="#8a8a8a"/><circle cx="7" cy="9" r="1" fill="#c4c4c4"/><circle cx="17" cy="9" r="1" fill="#c4c4c4"/>"##,
        "opt_document" => r##"<path d="M7 3.5h7.5l3.5 3.5v13.5H7z"/>"##,
        "opt_app" => r##"<rect x="3" y="3" width="18" height="18" rx="3" fill="BLUE" stroke="none"/><path d="M8 17l2.5-7 5-2.5-2.5 5zM10.5 10l3 3M8 17l3.4-3.4" stroke="#fff" stroke-width="1.4"/>"##,
        "opt_custom" => r##"<path d="M3 7h18M3 12h18M3 17h18"/><rect x="6" y="5" width="3" height="4" fill="#c4c4c4"/><rect x="14" y="10" width="3" height="4" fill="#c4c4c4"/><rect x="9" y="15" width="3" height="4" fill="#c4c4c4"/>"##,
        "opt_tools" => r##"<path d="M7 3.5v15l4-4 3 6 2.2-1-3-6h5.3z" fill="#e8e8e8" stroke="none"/>"##,
        "opt_global" => r##"<path d="M12 3a6.5 6.5 0 0 0-6.5 6.5c0 4.5 6.5 11.5 6.5 11.5s6.5-7 6.5-11.5A6.5 6.5 0 0 0 12 3z" fill="#e8e8e8" stroke="none"/>"##,
        "opt_workspaces" => r##"<rect x="3" y="4" width="18" height="12.5" rx="1"/><path d="M12 16.5v3.5M8 20.5h8"/>"##,
        "reverse" => r##"<path d="M4 9h14l-3-3M20 15H6l3 3"/>"##,
        "extend_close" => r##"<path d="M4 18C4 8 18 4 20 14"/><path d="M4 18h16" stroke-dasharray="2 1.5"/>"##,
        "close_curve" => r##"<path d="M4 18C4 8 18 4 20 14L4 18z"/>"##,
        "reduce_nodes" => r##"<path d="M3 16l3-4 3 3 3-6 3 4 3-3 3 2"/><path d="M3 20h18" stroke="#8a8a8a"/>"##,
        "extract" => r##"<path d="M4 18C6 6 18 6 20 18"/><rect x="10.5" y="7.5" width="3" height="3" fill="BLUE" stroke="none"/>"##,
        "elastic" => r##"<path d="M4 16c3-6 5 4 8-2s5 4 8-2"/>"##,
        "select_all_nodes" => r##"<path d="M4 18C6 6 18 6 20 18"/><rect x="2.5" y="16.5" width="3" height="3" fill="#c4c4c4"/><rect x="10.5" y="7.5" width="3" height="3" fill="#c4c4c4"/><rect x="18.5" y="16.5" width="3" height="3" fill="#c4c4c4"/>"##,
        _ => return None,
    };
    Some(
        s.replace("\"GREEN\"", &format!("\"{GREEN}\""))
            .replace("\"BLUE\"", &format!("\"{BLUE}\""))
            .replace("\"RED\"", &format!("\"{RED}\""))
            .replace("\"YELLOW\"", &format!("\"{YELLOW}\"")),
    )
}

/// Complete SVG document for an icon.
pub fn svg(name: &str, color: &str) -> Option<String> {
    let b = body(name)?;
    let b = if color != G { b.replace(G, color) } else { b };
    Some(format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24" width="24" height="24" fill="none" stroke="{color}" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">{b}</svg>"#
    ))
}

fn usvg_opts() -> &'static usvg::Options<'static> {
    static O: std::sync::OnceLock<usvg::Options<'static>> = std::sync::OnceLock::new();
    O.get_or_init(|| {
        let mut o = usvg::Options::default();
        let windir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into());
        let f = std::path::Path::new(&windir).join("Fonts");
        let db = o.fontdb_mut();
        for name in ["segoeui.ttf", "segoeuib.ttf", "times.ttf"] {
            let _ = db.load_font_file(f.join(name));
        }
        db.set_sans_serif_family("Segoe UI");
        db.set_serif_family("Times New Roman");
        o
    })
}

/// Rasterize an SVG string to RGBA at `px` pixels square.
pub fn rasterize_svg(svg: &str, w: u32, h: u32) -> Option<egui::ColorImage> {
    let tree = usvg::Tree::from_str(svg, usvg_opts()).ok()?;
    let mut pm = tiny_skia::Pixmap::new(w.max(1), h.max(1))?;
    let sz = tree.size();
    let tf = tiny_skia::Transform::from_scale(w as f32 / sz.width(), h as f32 / sz.height());
    resvg::render(&tree, tf, &mut pm.as_mut());
    Some(egui::ColorImage::from_rgba_premultiplied([w as usize, h as usize], pm.data()))
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Tone {
    Normal,
    Disabled,
    Bright,
}

/// Texture cache for icons.
#[derive(Default)]
pub struct Icons {
    cache: HashMap<(String, u32, Tone), Option<TextureHandle>>,
}

impl Icons {
    /// Texture for `name` at `size` logical points.
    pub fn get(&mut self, ctx: &egui::Context, name: &str, size: f32, tone: Tone) -> Option<egui::load::SizedTexture> {
        let ppp = ctx.pixels_per_point();
        let px = (size * ppp).round().max(1.0) as u32;
        let key = (name.to_string(), px, tone);
        if !self.cache.contains_key(&key) {
            let color = match tone {
                Tone::Normal => G,
                Tone::Disabled => "#808080",
                Tone::Bright => "#f0f0f0",
            };
            let tex = svg(name, color).and_then(|s| rasterize_svg(&s, px, px)).map(|mut img| {
                if tone == Tone::Disabled {
                    // Desaturate colored accents and compress them toward mid-gray, so a
                    // disabled icon reads as clearly dimmed but its shape stays legible on
                    // the dark bars (the gray strokes themselves are drawn in #808080).
                    for p in img.pixels.iter_mut() {
                        let [r, g, b, a] = p.to_array();
                        if a == 0 {
                            continue;
                        }
                        let af = a as f32 / 255.0;
                        // un-premultiply, take luminance, map 0..255 -> 58..198
                        let l = (r as f32 * 0.3 + g as f32 * 0.6 + b as f32 * 0.1) / af;
                        let l = (l * 0.55 + 58.0).min(255.0);
                        let v = (l * af).round().min(a as f32) as u8;
                        *p = Color32::from_rgba_premultiplied(v, v, v, a);
                    }
                }
                ctx.load_texture(format!("icon:{name}:{px}:{tone:?}"), img, TextureOptions::LINEAR)
            });
            self.cache.insert(key.clone(), tex);
        }
        self.cache.get(&key).and_then(|t| t.as_ref()).map(|t| egui::load::SizedTexture::new(t.id(), egui::vec2(size, size)))
    }

    pub fn image(&mut self, ctx: &egui::Context, name: &str, size: f32, enabled: bool) -> Option<egui::Image<'static>> {
        self.get(ctx, name, size, if enabled { Tone::Normal } else { Tone::Disabled }).map(egui::Image::from_texture)
    }
}

pub const ALL: &[&str] = &[
    "new", "open", "save", "open_cloud", "save_cloud", "print", "cut", "copy", "paste", "undo", "redo", "import", "export", "pdf", "fullscreen", "rulers", "grid", "guidelines", "snap_off", "ai", "gear",
    "pick", "shape", "crop", "zoom", "freehand", "artistic_media", "rectangle", "ellipse", "polygon", "text", "parallel_dim", "connector", "drop_shadow", "transparency", "eyedropper", "interactive_fill", "painterly",
];
