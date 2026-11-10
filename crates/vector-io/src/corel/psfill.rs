//! CorelDRAW's PostScript fills. A file keeps only the fill's number in CorelDRAW's library
//! and up to five numbers; CorelDRAW draws the fill with its own PostScript procedure. The
//! regular ones are drawn again here, as a repeating tile, from the same numbers. The others
//! get the tone they have at their default settings.

use tiny_skia as sk;
use vector_core::color::Color;

/// The fill as drawn: a tile (RGBA pixels, size in pixels, width in points, angle in degrees)
/// or a plain tone.
pub(super) enum Drawn {
    Tile { rgba: Vec<u8>, w: u32, h: u32, width: f64, angle: f64 },
    Tone(Color),
}

/// The fill's number, from the one a file stores. CorelDRAW 12 and older add 4 and set the top
/// two bits.
pub(super) fn number(stored: u16) -> u16 {
    if stored & 0xc000 == 0xc000 {
        (stored & 0x3fff).saturating_sub(4)
    } else {
        stored
    }
}

/// The library's first fill (Archimedes); the others follow in the library's order.
const FIRST: u16 = 28;

/// The tone of each fill at its default settings, measured from CorelDRAW's own drawing.
const TONES: [[u8; 3]; 52] = [
    [182, 182, 182], [202, 202, 203], [158, 158, 158], [124, 124, 124], [180, 180, 180], [247, 247, 247], [168, 168, 168],
    [143, 143, 143], [141, 141, 141], [230, 230, 230], [215, 215, 215], [164, 164, 164], [186, 186, 186], [127, 127, 127],
    [201, 201, 201], [195, 195, 195], [164, 164, 164], [204, 204, 204], [207, 207, 207], [187, 187, 187], [231, 231, 231],
    [177, 178, 178], [111, 111, 111], [199, 199, 199], [220, 220, 220], [201, 201, 201], [196, 196, 196], [248, 248, 248],
    [150, 150, 150], [227, 227, 227], [238, 238, 238], [191, 191, 191], [197, 197, 197], [188, 188, 188], [10, 10, 10],
    [213, 213, 213], [92, 92, 93], [233, 233, 233], [183, 183, 183], [224, 224, 224], [183, 183, 183], [217, 217, 217],
    [251, 251, 252], [220, 233, 241], [208, 203, 206], [224, 226, 226], [145, 164, 145], [230, 228, 225], [33, 180, 33],
    [123, 115, 120], [101, 113, 112], [90, 86, 80],
];

const BRICKS: u16 = 32;
const CROSSHATCHING: u16 = 39;
const FISHSCALE: u16 = 43;
const HATCHING: u16 = 45;
const HEXAGONS: u16 = 46;
const SQUARES: u16 = 60;
const TRIANGLE: u16 = 68;
const WAVES: u16 = 69;

/// Pixels per inch of the tiles.
const DPI: f64 = 150.0;

pub(super) fn draw(number: u16, p: &[i32]) -> Drawn {
    let arg = |i: usize, d: i32| p.get(i).copied().unwrap_or(d) as f64;
    // gray in percent of black
    let gray = |g: f64| {
        let v = (255.0 * (1.0 - g.clamp(0.0, 100.0) / 100.0)).round() as u8;
        Color::rgb(v, v, v)
    };
    match number {
        HATCHING | CROSSHATCHING => {
            // lines a random distance apart, between the minimum and maximum (thousandths of
            // an inch); drawn evenly, as dense as CorelDRAW's lines look on average
            let (max, min, lw, angle) = (arg(0, 75), arg(1, 0), arg(2, 5), arg(3, 45));
            let d = (0.85 * max).max(min).max(lw + 1.0).max(4.0) / 1000.0;
            let cross = number == CROSSHATCHING;
            tile(d, d, lw / 1000.0, Color::BLACK, Color::WHITE, -angle, |pb, w, h| {
                pb.line(0.0, h / 2.0, w, h / 2.0);
                if cross {
                    pb.line(w / 2.0, 0.0, w / 2.0, h);
                }
            })
        }
        // frequency (cells per inch), line width (thousandths of an inch), foreground and
        // background gray
        SQUARES | BRICKS | TRIANGLE | HEXAGONS | FISHSCALE | WAVES => {
            let c = 1.0 / arg(0, 8).clamp(0.5, 100.0);
            let (lw, fg, bg) = (arg(1, 5) / 1000.0, gray(arg(2, 100)), gray(arg(3, 0)));
            match number {
                SQUARES => tile(c, c, lw, fg, bg, 0.0, |pb, w, h| {
                    pb.line(0.0, 0.0, w, 0.0);
                    pb.line(0.0, 0.0, 0.0, h);
                }),
                // bricks a cell long and half a cell high, every other row shifted by half
                BRICKS => tile(c, c, lw, fg, bg, 0.0, |pb, w, h| {
                    pb.line(0.0, 0.0, w, 0.0);
                    pb.line(0.0, h / 2.0, w, h / 2.0);
                    pb.line(0.0, 0.0, 0.0, h / 2.0);
                    pb.line(w / 2.0, h / 2.0, w / 2.0, h);
                }),
                // equilateral triangles one cell wide
                TRIANGLE => tile(c, c * 3f64.sqrt(), lw, fg, bg, 0.0, |pb, w, h| {
                    pb.line(0.0, 0.0, w, 0.0);
                    pb.line(0.0, h / 2.0, w, h / 2.0);
                    pb.line(0.0, 0.0, w, h);
                    pb.line(w, 0.0, 0.0, h);
                }),
                // flat-topped hexagons with sides two thirds of a cell: two columns per tile
                HEXAGONS => {
                    let side = 2.0 * c / 3.0;
                    tile(3.0 * side, side * 3f64.sqrt(), lw, fg, bg, 0.0, |pb, w, h| {
                        let (s, m) = (w / 3.0, h / 2.0);
                        // one hexagon centered at (s, m), and the edge joining it to the next column
                        pb.0.move_to((s / 2.0) as f32, 0.0);
                        for (px, py) in [(1.5 * s, 0.0), (2.0 * s, m), (1.5 * s, h), (0.5 * s, h), (0.0, m), (0.5 * s, 0.0)] {
                            pb.0.line_to(px as f32, py as f32);
                        }
                        pb.line(2.0 * s, m, w, m);
                    })
                }
                // overlapping half circles a cell wide, rows half a cell apart
                FISHSCALE => tile(c, c, lw, fg, bg, 0.0, |pb, w, h| {
                    for (cx, cy) in [(w / 2.0, 0.0), (0.0, h / 2.0), (w, h / 2.0), (w / 2.0, h)] {
                        pb.half_circle(cx, cy, w / 2.0);
                    }
                }),
                // waves a cell long, rows a cell apart times the spacing
                _ => {
                    let spacing = arg(4, 100).clamp(10.0, 1000.0) / 100.0;
                    tile(c, c * spacing, lw, fg, bg, 0.0, move |pb, w, h| {
                        let (n, a) = (32, 0.15 * w);
                        pb.0.move_to(0.0, (h / 2.0) as f32);
                        for i in 1..=n {
                            let x = w * i as f64 / n as f64;
                            pb.0.line_to(x as f32, (h / 2.0 + a * (x / w * std::f64::consts::TAU).sin()) as f32);
                        }
                    })
                }
            }
        }
        _ => {
            let t = number.checked_sub(FIRST).and_then(|i| TONES.get(i as usize)).copied().unwrap_or([200, 200, 200]);
            Drawn::Tone(Color::rgb(t[0], t[1], t[2]))
        }
    }
}

/// A seamless tile `w` × `h` inches: the lines `shape` draws (in pixels, y down), stroked `lw`
/// inches wide, also drawn shifted by a tile each way so that lines across the edges join.
fn tile(w: f64, h: f64, lw: f64, fg: Color, bg: Color, angle: f64, shape: impl Fn(&mut Path, f64, f64)) -> Drawn {
    let pw = (w * DPI).round().clamp(2.0, 1024.0) as u32;
    let ph = (h * DPI).round().clamp(2.0, 1024.0) as u32;
    let Some(mut pm) = sk::Pixmap::new(pw, ph) else { return Drawn::Tone(Color::lerp(fg, bg, 0.5)) };
    pm.fill(sk::Color::from_rgba8(bg.r, bg.g, bg.b, 255));
    let mut pb = Path(sk::PathBuilder::new());
    shape(&mut pb, pw as f64, ph as f64);
    if let Some(path) = pb.0.finish() {
        let mut paint = sk::Paint::default();
        paint.set_color_rgba8(fg.r, fg.g, fg.b, 255);
        paint.anti_alias = true;
        let stroke = sk::Stroke { width: ((lw * DPI) as f32).max(1.5), ..Default::default() };
        for dx in [-1.0, 0.0, 1.0] {
            for dy in [-1.0, 0.0, 1.0] {
                pm.stroke_path(&path, &paint, &stroke, sk::Transform::from_translate(dx * pw as f32, dy * ph as f32), None);
            }
        }
    }
    Drawn::Tile { rgba: pm.data().to_vec(), w: pw, h: ph, width: pw as f64 / DPI * 72.0, angle }
}

struct Path(sk::PathBuilder);

impl Path {
    fn line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) {
        self.0.move_to(x0 as f32, y0 as f32);
        self.0.line_to(x1 as f32, y1 as f32);
    }
    /// The lower half of a circle (y grows downward).
    fn half_circle(&mut self, cx: f64, cy: f64, r: f64) {
        let n = 24;
        for i in 0..=n {
            let a = std::f64::consts::PI * i as f64 / n as f64;
            let (x, y) = ((cx + r * a.cos()) as f32, (cy + r * a.sin()) as f32);
            if i == 0 {
                self.0.move_to(x, y);
            } else {
                self.0.line_to(x, y);
            }
        }
    }
}
