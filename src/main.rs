//! Cats Grid — one cat per row, per column, per color, no two touching.
//! Native desktop build for the G760. Optional hints, extra lives, and a
//! win counter that only credits clean (unaided) solves.
//!
//! Everything on screen is drawn procedurally with egui's `Painter`:
//! warm "paper" theme, rounded pastel tiles, a vector cat face, paw lives.

mod puzzle;
mod rules;
mod stats;

use eframe::egui::{
    self, epaint::Shadow, Align2, Color32, CornerRadius, FontFamily, FontId, Painter, Pos2, Rect,
    Sense, Shape, Stroke, StrokeKind, Vec2,
};
use puzzle::{solutions, Puzzle, DEFAULT_N};
use stats::Stats;
use std::f32::consts::PI;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

// ───────────────────────────── theme ─────────────────────────────

const BG: Color32 = Color32::from_rgb(246, 241, 233);
const CARD: Color32 = Color32::from_rgb(255, 255, 255);
const WELL: Color32 = Color32::from_rgb(241, 235, 226);
const HAIRLINE: Color32 = Color32::from_rgb(232, 224, 212);
const INK: Color32 = Color32::from_rgb(43, 47, 56);
const INK_SOFT: Color32 = Color32::from_rgb(104, 109, 124);
const INK_FAINT: Color32 = Color32::from_rgb(160, 158, 160);
const ACCENT: Color32 = Color32::from_rgb(212, 110, 70);
const ACCENT_DEEP: Color32 = Color32::from_rgb(182, 86, 52);
const TONAL: Color32 = Color32::from_rgb(238, 230, 219);
const TONAL_HOVER: Color32 = Color32::from_rgb(229, 218, 203);
const SAGE: Color32 = Color32::from_rgb(98, 150, 104);
const AMBER: Color32 = Color32::from_rgb(206, 146, 52);
const HINT_CARD: Color32 = Color32::from_rgb(255, 248, 231);
const HINT_EDGE: Color32 = Color32::from_rgb(236, 214, 168);
const HINT_TITLE: Color32 = Color32::from_rgb(150, 100, 24);
const HINT_TEXT: Color32 = Color32::from_rgb(64, 50, 26);
const WRONG: Color32 = Color32::from_rgb(222, 64, 58);
const X_INK: Color32 = Color32::from_rgb(58, 40, 52);

// cat
const CAT_INK: Color32 = Color32::from_rgb(50, 45, 44);
const CAT_BODY: Color32 = Color32::from_rgb(64, 58, 56);
const CAT_CREAM: Color32 = Color32::from_rgb(246, 236, 219);
const CAT_EAR: Color32 = Color32::from_rgb(226, 158, 154);
const CAT_EYE: Color32 = Color32::from_rgb(236, 186, 80);
const CAT_PUPIL: Color32 = Color32::from_rgb(28, 25, 24);
const CAT_NOSE: Color32 = Color32::from_rgb(226, 132, 134);
const CAT_WHISKER: Color32 = Color32::from_rgb(158, 148, 140);

/// Ten muted pastels, spread over hue *and* lightness so neighbouring regions
/// stay distinct for colour-blind eyes too. The name is what the hint text says.
const PALETTE: [(&str, [u8; 3]); 10] = [
    ("rose", [244, 170, 180]),
    ("apricot", [248, 192, 142]),
    ("butter", [242, 222, 136]),
    ("sage", [188, 214, 146]),
    ("mint", [140, 216, 190]),
    ("teal", [104, 184, 190]),
    ("sky", [148, 196, 240]),
    ("periwinkle", [140, 152, 218]),
    ("lilac", [198, 174, 234]),
    ("plum", [190, 130, 182]),
];

#[derive(Clone, Copy)]
enum W {
    Regular,
    Semi,
    Bold,
}

fn font(size: f32, w: W) -> FontId {
    let fam = match w {
        W::Regular => FontFamily::Proportional,
        W::Semi => FontFamily::Name("semibold".into()),
        W::Bold => FontFamily::Name("bold".into()),
    };
    FontId::new(size, fam)
}

/// Fira Sans if present; otherwise the named families just alias egui's defaults.
fn install_fonts(ctx: &egui::Context) {
    use egui::{FontData, FontDefinitions};
    use std::sync::Arc;
    let mut defs = FontDefinitions::default();
    let base = defs.families.get(&FontFamily::Proportional).cloned().unwrap_or_default();
    let dir = "/usr/share/fonts/TTF";
    let mut load = |key: &str, file: &str| -> Option<String> {
        let bytes = std::fs::read(format!("{dir}/{file}")).ok()?;
        defs.font_data.insert(key.to_owned(), Arc::new(FontData::from_owned(bytes)));
        Some(key.to_owned())
    };
    let regular = load("fira-regular", "FiraSans-Regular.ttf");
    let semi = load("fira-semibold", "FiraSans-SemiBold.ttf");
    let bold = load("fira-bold", "FiraSans-Bold.ttf").or_else(|| semi.clone());
    let semi = semi.or_else(|| bold.clone());
    let with = |first: Option<String>| {
        let mut v = base.clone();
        if let Some(f) = first {
            v.insert(0, f);
        }
        v
    };
    defs.families.insert(FontFamily::Proportional, with(regular.clone()));
    defs.families.insert(FontFamily::Name("semibold".into()), with(semi.or(regular.clone())));
    defs.families.insert(FontFamily::Name("bold".into()), with(bold.or(regular)));
    ctx.set_fonts(defs);
}

fn install_style(ctx: &egui::Context) {
    use egui::{TextStyle, Theme};
    ctx.set_theme(Theme::Light);
    let mut v = egui::Visuals::light();
    v.panel_fill = BG;
    v.window_fill = CARD;
    v.extreme_bg_color = CARD;
    v.override_text_color = Some(INK);
    v.window_corner_radius = CornerRadius::same(14);
    v.selection.bg_fill = ACCENT.gamma_multiply(0.35);
    ctx.set_visuals_of(Theme::Light, v);
    ctx.all_styles_mut(|s| {
        s.text_styles.insert(TextStyle::Heading, font(30.0, W::Bold));
        s.text_styles.insert(TextStyle::Body, font(15.5, W::Regular));
        s.text_styles.insert(TextStyle::Button, font(13.0, W::Semi));
        s.text_styles.insert(TextStyle::Small, font(12.5, W::Regular));
        s.spacing.item_spacing = Vec2::new(8.0, 6.0);
    });
}

fn cr(r: f32) -> CornerRadius {
    CornerRadius::same(r.round().clamp(0.0, 255.0) as u8)
}

fn rgb(c: [u8; 3]) -> Color32 {
    Color32::from_rgb(c[0], c[1], c[2])
}

fn shade(c: Color32, f: f32) -> Color32 {
    let m = |v: u8| (v as f32 * f).clamp(0.0, 255.0) as u8;
    Color32::from_rgb(m(c.r()), m(c.g()), m(c.b()))
}

fn mix(a: Color32, b: Color32, t: f32) -> Color32 {
    let l = |x: u8, y: u8| (x as f32 + (y as f32 - x as f32) * t).round() as u8;
    Color32::from_rgb(l(a.r(), b.r()), l(a.g(), b.g()), l(a.b(), b.b()))
}

fn ease_out(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    1.0 - (1.0 - t).powi(3)
}

fn ease_out_back(t: f32) -> f32 {
    let t = t.clamp(0.0, 1.0);
    let c1 = 1.9;
    let c3 = c1 + 1.0;
    1.0 + c3 * (t - 1.0).powi(3) + c1 * (t - 1.0).powi(2)
}

fn soft_shadow(offset_y: i8, blur: u8, spread: u8, alpha: u8) -> Shadow {
    Shadow {
        offset: [0, offset_y],
        blur,
        spread,
        color: Color32::from_rgba_unmultiplied(70, 46, 24, alpha),
    }
}

// ───────────────────────────── vector helpers ─────────────────────────────

fn quad(a: Pos2, b: Pos2, c: Pos2, n: usize) -> Vec<Pos2> {
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            Pos2::new(
                u * u * a.x + 2.0 * u * t * b.x + t * t * c.x,
                u * u * a.y + 2.0 * u * t * b.y + t * t * c.y,
            )
        })
        .collect()
}

fn cubic(p: [Pos2; 4], n: usize) -> Vec<Pos2> {
    (0..=n)
        .map(|i| {
            let t = i as f32 / n as f32;
            let u = 1.0 - t;
            let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
            Pos2::new(
                a * p[0].x + b * p[1].x + c * p[2].x + d * p[3].x,
                a * p[0].y + b * p[1].y + c * p[2].y + d * p[3].y,
            )
        })
        .collect()
}

/// A convex polygon with every corner replaced by a small quadratic arc.
fn rounded_poly(pts: &[Pos2], frac: f32) -> Vec<Pos2> {
    let n = pts.len();
    let mut out = Vec::with_capacity(n * 7);
    for i in 0..n {
        let p = pts[i];
        let prev = pts[(i + n - 1) % n];
        let next = pts[(i + 1) % n];
        let a = p + (prev - p) * frac;
        let b = p + (next - p) * frac;
        out.extend(quad(a, p, b, 6));
    }
    out
}

/// Superellipse-ish blob (convex), slightly wider at the bottom like cheeks.
fn head_shape(c: Pos2, rx: f32, ry: f32) -> Vec<Pos2> {
    let steps = 56;
    (0..steps)
        .map(|i| {
            let t = i as f32 / steps as f32 * 2.0 * PI;
            let (s, co) = t.sin_cos();
            let e = 2.0 / 2.35;
            let x = co.signum() * co.abs().powf(e);
            let y = s.signum() * s.abs().powf(e);
            let cheek = 1.0 + 0.06 * s.max(0.0);
            Pos2::new(c.x + rx * x * cheek, c.y + ry * y)
        })
        .collect()
}

fn round_line(p: &Painter, pts: Vec<Pos2>, w: f32, col: Color32) {
    if let (Some(&a), Some(&b)) = (pts.first(), pts.last()) {
        p.circle_filled(a, w * 0.5, col);
        p.circle_filled(b, w * 0.5, col);
    }
    p.line(pts, Stroke::new(w, col));
}

// ───────────────────────────── illustrations ─────────────────────────────

/// Hand-drawn cat face, `k` = tile size, roughly 70 % coverage. Callers clip.
fn draw_cat(p: &Painter, center: Pos2, k: f32, alpha: f32) {
    let a = |c: Color32| c.gamma_multiply(alpha);
    let at = |x: f32, y: f32| center + Vec2::new(x * k, y * k);

    // body / shoulders peeking from the bottom, tail curling out of frame
    p.add(Shape::ellipse_filled(at(0.0, 0.47), Vec2::new(0.27 * k, 0.17 * k), a(CAT_BODY)));
    let tail = cubic([at(0.14, 0.40), at(0.44, 0.42), at(0.58, 0.18), at(0.47, -0.10)], 24);
    round_line(p, tail, 0.075 * k, a(CAT_BODY));

    // ears
    for s in [-1.0f32, 1.0] {
        let outer = [at(0.285 * s, -0.04), at(0.255 * s, -0.355), at(0.055 * s, -0.175)];
        let outer = if s < 0.0 { outer } else { [outer[0], outer[2], outer[1]] };
        p.add(Shape::convex_polygon(rounded_poly(&outer, 0.18), a(CAT_INK), Stroke::NONE));
        let inner = [at(0.235 * s, -0.09), at(0.225 * s, -0.285), at(0.105 * s, -0.17)];
        let inner = if s < 0.0 { inner } else { [inner[0], inner[2], inner[1]] };
        p.add(Shape::convex_polygon(rounded_poly(&inner, 0.22), a(CAT_EAR), Stroke::NONE));
    }

    // head
    p.add(Shape::convex_polygon(head_shape(at(0.0, 0.035), 0.305 * k, 0.255 * k), a(CAT_INK), Stroke::NONE));

    // cream muzzle (two cheeks + chin)
    for s in [-1.0f32, 1.0] {
        p.add(Shape::ellipse_filled(at(0.068 * s, 0.135), Vec2::new(0.098 * k, 0.074 * k), a(CAT_CREAM)));
    }
    p.add(Shape::ellipse_filled(at(0.0, 0.19), Vec2::new(0.07 * k, 0.05 * k), a(CAT_CREAM)));

    // eyes: amber almond, vertical slit, highlight
    for s in [-1.0f32, 1.0] {
        let e = at(0.125 * s, -0.012);
        p.add(Shape::ellipse_filled(e, Vec2::new(0.07 * k, 0.06 * k), a(CAT_EYE)));
        p.add(Shape::ellipse_filled(e, Vec2::new(0.017 * k, 0.05 * k), a(CAT_PUPIL)));
        p.circle_filled(e + Vec2::new(0.022 * k, -0.022 * k), (0.015 * k).max(0.8), a(Color32::WHITE));
    }

    // nose
    let nose = [at(-0.034, 0.078), at(0.034, 0.078), at(0.0, 0.113)];
    p.add(Shape::convex_polygon(rounded_poly(&nose, 0.3), a(CAT_NOSE), Stroke::NONE));

    // w-shaped mouth
    let mw = (0.014 * k).max(1.0);
    let ink = Stroke::new(mw, a(CAT_INK));
    p.line(vec![at(0.0, 0.11), at(0.0, 0.135)], ink);
    for s in [-1.0f32, 1.0] {
        p.line(quad(at(0.0, 0.135), at(0.022 * s, 0.172), at(0.062 * s, 0.142), 10), ink);
    }

    // whiskers, three per side
    let ww = (0.0105 * k).max(0.9);
    for s in [-1.0f32, 1.0] {
        for (i, dy) in [-0.034f32, 0.0, 0.034].iter().enumerate() {
            let tilt = (i as f32 - 1.0) * 0.045;
            let start = at(0.14 * s, 0.128 + dy);
            let ctrl = at(0.31 * s, 0.112 + dy + tilt * 0.5);
            let end = at(0.45 * s, 0.096 + dy * 1.4 + tilt);
            p.line(quad(start, ctrl, end, 10), Stroke::new(ww, a(CAT_WHISKER)));
        }
    }
}

/// Simple paw print, `s` = overall size.
fn draw_paw(p: &Painter, c: Pos2, s: f32, col: Color32) {
    p.add(Shape::ellipse_filled(c + Vec2::new(0.0, 0.17 * s), Vec2::new(0.27 * s, 0.22 * s), col));
    for (x, y) in [(-0.34, -0.10), (-0.13, -0.30), (0.13, -0.30), (0.34, -0.10)] {
        p.add(Shape::ellipse_filled(c + Vec2::new(x * s, y * s), Vec2::new(0.115 * s, 0.135 * s), col));
    }
}

fn draw_x(p: &Painter, rect: Rect, t: f32) {
    // ease-in so the X of a double-click's first click barely shows
    let e = (t / 0.16).clamp(0.0, 1.0);
    let e = e * e * (3.0 - 2.0 * e);
    let k = rect.width();
    let h = 0.275 * k * (0.82 + 0.18 * e);
    let w = (0.085 * k).clamp(2.8, 7.5);
    let col = X_INK.gamma_multiply(0.88 * e);
    let c = rect.center();
    round_line(p, vec![c + Vec2::new(-h, -h), c + Vec2::new(h, h)], w, col);
    round_line(p, vec![c + Vec2::new(h, -h), c + Vec2::new(-h, h)], w, col);
}

fn draw_info_icon(p: &Painter, c: Pos2, r: f32) {
    p.circle_filled(c, r, AMBER);
    p.circle_filled(c + Vec2::new(0.0, -r * 0.45), r * 0.14, Color32::WHITE);
    p.line(
        vec![c + Vec2::new(0.0, -r * 0.12), c + Vec2::new(0.0, r * 0.5)],
        Stroke::new(r * 0.24, Color32::WHITE),
    );
}

// ───────────────────────────── widgets ─────────────────────────────

#[derive(Clone, Copy, PartialEq)]
enum Kind {
    Primary,
    Tonal,
}

fn button_width(ui: &egui::Ui, text: &str) -> f32 {
    ui.painter().layout_no_wrap(text.to_owned(), font(13.0, W::Semi), INK).size().x + 32.0
}

fn pill_button(ui: &mut egui::Ui, text: &str, kind: Kind, enabled: bool) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(text.to_owned(), font(13.0, W::Semi), INK);
    let size = Vec2::new(galley.size().x + 32.0, 36.0);
    let sense = if enabled { Sense::click() } else { Sense::hover() };
    let (rect, resp) = ui.allocate_exact_size(size, sense);
    let hovered = enabled && resp.hovered();
    let pressed = enabled && resp.is_pointer_button_down_on();
    let rect = if pressed { rect.translate(Vec2::new(0.0, 1.0)) } else { rect };
    let p = ui.painter();
    let (fill, text_col) = match kind {
        Kind::Primary => (if hovered { ACCENT_DEEP } else { ACCENT }, Color32::WHITE),
        Kind::Tonal => (if hovered { TONAL_HOVER } else { TONAL }, INK),
    };
    let fade = if enabled { 1.0 } else { 0.45 };
    if kind == Kind::Primary && enabled && !pressed {
        p.add(Shadow {
            offset: [0, 3],
            blur: 10,
            spread: 0,
            color: Color32::from_rgba_unmultiplied(182, 86, 52, 70),
        }
        .as_shape(rect, cr(11.0)));
    }
    p.rect_filled(rect, cr(11.0), fill.gamma_multiply(fade));
    let pos = rect.center() - galley.size() * 0.5;
    p.galley(pos, galley, text_col.gamma_multiply(fade));
    if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    }
}

/// Quiet stat chip: coloured dot, small label, semibold value.
fn chip(ui: &mut egui::Ui, dot: Option<Color32>, label: &str, value: &str) {
    let p = ui.painter();
    let lg = p.layout_no_wrap(label.to_owned(), font(12.5, W::Regular), INK_SOFT);
    let vg = p.layout_no_wrap(value.to_owned(), font(13.5, W::Semi), INK);
    let dot_w = if dot.is_some() { 14.0 } else { 0.0 };
    let w = 12.0 + dot_w + lg.size().x + 6.0 + vg.size().x + 12.0;
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, 28.0), Sense::hover());
    let p = ui.painter();
    p.rect_filled(rect, cr(14.0), CARD);
    p.rect_stroke(rect, cr(14.0), Stroke::new(1.0, HAIRLINE), StrokeKind::Inside);
    let mut x = rect.left() + 12.0;
    if let Some(d) = dot {
        p.circle_filled(Pos2::new(x + 4.0, rect.center().y), 4.0, d);
        x += dot_w;
    }
    let lw = lg.size().x;
    p.galley(Pos2::new(x, rect.center().y - lg.size().y * 0.5), lg, INK_SOFT);
    x += lw + 6.0;
    p.galley(Pos2::new(x, rect.center().y - vg.size().y * 0.5), vg, INK);
}

// ───────────────────────────── app state ─────────────────────────────

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cell {
    Empty,
    Cat,
    X,
}

/// A press on the board that may turn into an X-painting drag.
struct Drag {
    origin: usize,
    last: Pos2,
    painting: bool,
    /// cell the pointer is currently in; every entry into a new cell flips it once
    cur: usize,
}

struct App {
    n: usize,
    puzzle: Option<Puzzle>,
    cells: Vec<Cell>,
    /// egui time of each cell's last change (drives pop / fade animations)
    changed_at: Vec<f64>,
    lives: u8,
    max_lives: u8,
    hint: Option<(usize, usize)>,
    hint_at: Option<f64>,
    /// the spoken deduction behind the current hint
    hint_text: Option<String>,
    /// true as soon as a hint or an extra life is used — kills the clean run
    assisted: bool,
    hints_used: u32,
    extra_lives: u32,
    start: Option<Instant>,
    elapsed: Duration,
    won: bool,
    game_over: bool,
    ended_at: f64,
    message: String,
    last_msg: String,
    last_hint: Option<String>,
    dumped: bool,
    stats: Stats,
    rx: Option<mpsc::Receiver<Option<puzzle::GenReport>>>,
    wrong: Option<((usize, usize), f64)>,
    gen_ms: Option<u128>,
    /// current egui time, refreshed every frame
    now: f64,
    last_press: Option<(usize, f64)>,
    drag: Option<Drag>,
    /// last single click: (cell, state before the click) — undone by a double click
    last_click: Option<(usize, Cell)>,
}

impl App {
    fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);
        install_style(&cc.egui_ctx);
        let stats = Stats::load();
        let mut app = App {
            n: DEFAULT_N,
            puzzle: None,
            cells: vec![Cell::Empty; DEFAULT_N * DEFAULT_N],
            changed_at: vec![-10.0; DEFAULT_N * DEFAULT_N],
            lives: 3,
            max_lives: 3,
            hint: None,
            hint_at: None,
            hint_text: None,
            assisted: false,
            hints_used: 0,
            extra_lives: 0,
            start: None,
            elapsed: Duration::ZERO,
            won: false,
            game_over: false,
            ended_at: 0.0,
            message: String::from("Building a puzzle…"),
            last_msg: String::new(),
            last_hint: None,
            dumped: false,
            stats,
            rx: None,
            wrong: None,
            gen_ms: None,
            now: 0.0,
            last_press: None,
            drag: None,
            last_click: None,
        };
        app.request_new_puzzle();
        app
    }

    fn request_new_puzzle(&mut self) {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0xDEAD_BEEF)
            ^ (self.stats.games_started as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(puzzle::generate(DEFAULT_N, seed));
        });
        self.rx = Some(rx);
        self.puzzle = None;
        self.cells = vec![Cell::Empty; self.n * self.n];
        self.changed_at = vec![-10.0; self.n * self.n];
        self.lives = self.max_lives;
        self.hint = None;
        self.hint_text = None;
        self.assisted = false;
        self.hints_used = 0;
        self.extra_lives = 0;
        self.start = None;
        self.elapsed = Duration::ZERO;
        self.won = false;
        self.game_over = false;
        self.wrong = None;
        self.gen_ms = None;
        self.drag = None;
        self.last_click = None;
        self.message = String::from("Building a puzzle…");
        self.stats.games_started += 1;
        self.stats.save();
    }

    fn poll_generator(&mut self, ctx: &egui::Context) -> bool {
        let mut ready = false;
        if let Some(rx) = &self.rx {
            match rx.try_recv() {
                Ok(Some(rep)) => {
                    self.gen_ms = Some(rep.elapsed.as_millis());
                    self.n = rep.puzzle.n;
                    self.cells = vec![Cell::Empty; self.n * self.n];
                    self.changed_at = vec![-10.0; self.n * self.n];
                    self.puzzle = Some(rep.puzzle);
                    if std::env::var_os("CATS_DEBUG").is_some() {
                        if let Some(p) = &self.puzzle {
                            eprintln!("SOLUTION n={} {:?}", p.n, p.solution);
                        }
                    }
                    self.start = Some(Instant::now());
                    self.rx = None;
                    self.message = String::from("One cat per row, column and color — none may touch.");
                    ready = true;
                }
                Ok(None) => {
                    self.rx = None;
                    self.message = String::from("Generator failed — press New game.");
                    ready = true;
                }
                Err(mpsc::TryRecvError::Empty) => {
                    ctx.request_repaint_after(Duration::from_millis(60));
                }
                Err(mpsc::TryRecvError::Disconnected) => {
                    self.rx = None;
                }
            }
        }
        ready
    }

    fn set_cell(&mut self, idx: usize, v: Cell) {
        if self.cells[idx] != v {
            self.cells[idx] = v;
            self.changed_at[idx] = self.now;
        }
    }

    fn cats(&self) -> Vec<(usize, usize)> {
        let mut v = Vec::new();
        for r in 0..self.n {
            for c in 0..self.n {
                if self.cells[r * self.n + c] == Cell::Cat {
                    v.push((r, c));
                }
            }
        }
        v
    }

    fn xs(&self) -> Vec<(usize, usize)> {
        let mut v = Vec::new();
        for r in 0..self.n {
            for c in 0..self.n {
                if self.cells[r * self.n + c] == Cell::X {
                    v.push((r, c));
                }
            }
        }
        v
    }

    fn conflict(&self, r: usize, c: usize) -> Option<&'static str> {
        if let Some(p) = &self.puzzle {
            let g = p.region_at(r, c);
            for (r2, c2) in self.cats() {
                if r2 == r {
                    return Some("that row already has a cat");
                }
                if c2 == c {
                    return Some("that column already has a cat");
                }
                if p.region_at(r2, c2) == g {
                    return Some("that color already has a cat");
                }
                if r.abs_diff(r2) <= 1 && c.abs_diff(c2) <= 1 {
                    return Some("cats can't touch, even diagonally");
                }
            }
        }
        None
    }

    fn place_cat(&mut self, r: usize, c: usize) {
        if self.won || self.game_over {
            return;
        }
        let Some(p) = self.puzzle.clone() else { return };
        let idx = r * self.n + c;
        if let Some(why) = self.conflict(r, c) {
            self.fail_cell(r, c, why);
            return;
        }
        self.set_cell(idx, Cell::Cat);
        let cats = self.cats();
        // A legal-looking cat that still admits no completion is a wrong cat.
        if solutions(self.n, &p.region, &cats, &[], 1).is_empty() {
            if std::env::var_os("CATS_DEBUG").is_some() {
                eprintln!("REJECT r={} c={} cats={:?} solution={:?}", r, c, cats, p.solution);
            }
            self.set_cell(idx, Cell::X);
            self.fail_cell(r, c, "no solution is left with a cat there");
            return;
        }
        self.hint = None;
        self.hint_text = None;
        if cats.len() == self.n {
            self.won = true;
            self.ended_at = self.now;
            self.elapsed = self.start.map(|s| s.elapsed()).unwrap_or_default();
            let ms = self.elapsed.as_millis() as u64;
            let clean = !self.assisted;
            self.stats.record_win(clean, ms);
            self.message = if clean {
                format!("Solved clean in {} — that one counts.", fmt_time(ms))
            } else {
                String::from("Solved — counted as assisted, not a clean win.")
            };
        } else {
            self.message = format!(
                "{} cat{} placed, {} to go.",
                cats.len(),
                if cats.len() == 1 { "" } else { "s" },
                self.n - cats.len()
            );
        }
    }

    fn fail_cell(&mut self, r: usize, c: usize, why: &str) {
        self.wrong = Some(((r, c), self.now));
        if self.lives > 0 {
            self.lives -= 1;
        }
        self.hint = None;
        self.hint_text = None;
        if self.lives == 0 {
            self.game_over = true;
            self.ended_at = self.now;
            self.elapsed = self.start.map(|s| s.elapsed()).unwrap_or_default();
            self.message = format!("Out of lives — {}.", why);
        } else {
            self.message = format!("Not a cat: {}. Lost a life ({} left).", why, self.lives);
        }
    }

    /// Rule-by-rule X marks available on the current board.
    fn deductions(&self) -> Vec<rules::Deduction> {
        if self.won || self.game_over {
            return Vec::new();
        }
        let Some(p) = self.puzzle.as_ref() else { return Vec::new() };
        let n = self.n;
        let cats = self.cats();
        rules::deductions(
            n,
            &p.region,
            &cats,
            &|g| color_name(g, n),
            &|r, c| self.cells[r * n + c] == Cell::Empty,
        )
    }

    /// Apply one rule's X marks and show the reasoning behind them.
    fn apply_deduction(&mut self, d: &rules::Deduction) {
        for &(r, c) in &d.cells {
            self.set_cell(r * self.n + c, Cell::X);
        }
        self.assisted = true;
        self.message = d.why.clone();
    }

    fn give_hint(&mut self) {
        if self.won || self.game_over {
            return;
        }
        let Some(p) = self.puzzle.clone() else { return };
        let cats = self.cats();
        let xs = self.xs();
        let with_xs = solutions(self.n, &p.region, &cats, &xs, 1);
        let without_xs = solutions(self.n, &p.region, &cats, &[], 1);

        if with_xs.is_empty() && without_xs.is_empty() {
            self.message = String::from("Something you placed is wrong — no completion exists.");
            return;
        }
        if with_xs.is_empty() {
            self.message = String::from("One of your X marks is on the real cat — clear some X's.");
            return;
        }
        let sol = with_xs[0].clone();
        let Some((target, why)) = puzzle::best_hint(self.n, &p.region, &cats, &sol, &|g| color_name(g, self.n))
        else {
            return;
        };
        self.assisted = true;
        self.hints_used += 1;
        self.hint = Some(target);
        self.hint_at = Some(self.now);
        self.hint_text = Some(why);
        self.message = format!("Next cat: row {}, column {}.", target.0 + 1, target.1 + 1);
    }

    fn buy_extra_life(&mut self) {
        if self.won || self.puzzle.is_none() {
            return;
        }
        self.assisted = true;
        self.extra_lives += 1;
        self.lives = (self.lives + 1).min(5);
        if self.game_over {
            self.game_over = false;
            self.start = Some(Instant::now() - self.elapsed);
        }
        self.message = String::from("Extra life added — this run won't count as clean.");
    }

    fn clear_marks(&mut self) {
        if self.won {
            return;
        }
        for i in 0..self.cells.len() {
            self.set_cell(i, Cell::Empty);
        }
        self.hint = None;
        self.hint_text = None;
        self.last_click = None;
        self.message = String::from("Marks cleared.");
    }

    // ── mouse model ──

    /// Single click: toggle an X. Cats are proven correct when placed, so a
    /// single click leaves them alone.
    fn single_click(&mut self, idx: usize) {
        let prev = self.cells[idx];
        match prev {
            Cell::Empty => self.set_cell(idx, Cell::X),
            Cell::X => self.set_cell(idx, Cell::Empty),
            Cell::Cat => {}
        }
        self.last_click = Some((idx, prev));
    }

    /// Double click: egui reports the first click on its own a moment earlier,
    /// so undo that toggle first, then place the cat.
    fn double_click(&mut self, idx: usize) {
        if let Some((li, prev)) = self.last_click.take() {
            if li == idx {
                self.cells[idx] = prev;
            }
        }
        if self.cells[idx] != Cell::Cat {
            // The first press of the double click can also have arrived as a
            // drag-painted X (held a few ms too long), which `last_click` never
            // saw. The cell is about to hold a cat (or stay empty if the cat is
            // rejected), so clear any mark here unconditionally.
            self.cells[idx] = Cell::Empty;
            self.place_cat(idx / self.n, idx % self.n);
        }
    }

    /// Drag stroke: flip a cell once each time the pointer enters it, so a drag
    /// over unmarked cells sets X's and a drag over X's clears them (and a drag
    /// back over a cell undoes it).
    fn drag_flip(&mut self, idx: usize, cur: &mut usize) {
        if *cur == idx {
            return;
        }
        *cur = idx;
        match self.cells[idx] {
            Cell::Empty => self.set_cell(idx, Cell::X),
            Cell::X => self.set_cell(idx, Cell::Empty),
            Cell::Cat => {}
        }
    }
}

fn fmt_time(ms: u64) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

fn region_color(g: u8, _n: usize) -> Color32 {
    rgb(PALETTE[g as usize % PALETTE.len()].1)
}

fn color_name(g: u8, _n: usize) -> &'static str {
    PALETTE[g as usize % PALETTE.len()].0
}

/// Board geometry inside the card.
struct Board {
    inner: Rect,
    tile: f32,
    gap: f32,
    n: usize,
}

impl Board {
    fn rect(&self, r: usize, c: usize) -> Rect {
        let step = self.tile + self.gap;
        Rect::from_min_size(
            self.inner.min + Vec2::new(c as f32 * step, r as f32 * step),
            Vec2::splat(self.tile),
        )
    }

    fn cell_at(&self, p: Pos2) -> Option<usize> {
        if !self.inner.expand(self.gap * 0.5).contains(p) {
            return None;
        }
        let step = self.tile + self.gap;
        let f = |v: f32| ((v / step).floor() as isize).clamp(0, self.n as isize - 1) as usize;
        let c = f(p.x - self.inner.min.x + self.gap * 0.5);
        let r = f(p.y - self.inner.min.y + self.gap * 0.5);
        Some(r * self.n + c)
    }
}

// ───────────────────────────── UI ─────────────────────────────

impl App {
    fn header(&mut self, ui: &mut egui::Ui) {
        let wide = ui.available_width() > 760.0;
        ui.horizontal(|ui| {
            // badge
            let (badge, _) = ui.allocate_exact_size(Vec2::splat(40.0), Sense::hover());
            let p = ui.painter();
            p.add(soft_shadow(2, 8, 0, 40).as_shape(badge, cr(12.0)));
            p.rect_filled(badge, cr(12.0), ACCENT);
            draw_cat(&p.with_clip_rect(badge.shrink(1.0)), badge.center() + Vec2::new(0.0, 1.0), 40.0, 1.0);
            ui.add_space(6.0);
            ui.label(egui::RichText::new("Cats Grid").font(font(30.0, W::Bold)).color(INK));

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if wide {
                    if let Some(best) = self.stats.best_clean_ms {
                        chip(ui, None, "best", &fmt_time(best));
                    }
                }
                chip(ui, Some(AMBER), "assisted", &self.stats.assisted_wins.to_string());
                chip(ui, Some(SAGE), "clean", &self.stats.clean_wins.to_string());
            });
        });
        ui.add_space(4.0);
        ui.horizontal(|ui| {
            ui.add_space(2.0);
            // paws
            let slots = self.lives.max(self.max_lives) as usize;
            let (rect, _) = ui.allocate_exact_size(Vec2::new(slots as f32 * 24.0, 24.0), Sense::hover());
            let p = ui.painter();
            for i in 0..slots {
                let c = Pos2::new(rect.left() + 12.0 + i as f32 * 24.0, rect.center().y);
                let col = if i < self.lives as usize { ACCENT } else { INK_FAINT.gamma_multiply(0.35) };
                draw_paw(p, c, 18.0, col);
            }
            ui.add_space(6.0);
            ui.label(egui::RichText::new("lives").font(font(12.5, W::Regular)).color(INK_SOFT));
            ui.add_space(14.0);
            chip(ui, None, "time", &fmt_time(self.elapsed.as_millis() as u64));
            ui.add_space(4.0);
            // always reserve the chip's footprint so the board never re-flows
            let g = ui.painter().layout_no_wrap("assisted run".into(), font(12.5, W::Semi), AMBER);
            let (r, _) = ui.allocate_exact_size(g.size() + Vec2::new(20.0, 10.0), Sense::hover());
            if self.assisted {
                ui.painter().rect_filled(r, cr(12.0), AMBER.gamma_multiply(0.14));
                ui.painter().galley(r.center() - g.size() * 0.5, g, AMBER);
            }
        });
    }

    fn controls(&mut self, ui: &mut egui::Ui) {
        let playing = !self.won && !self.game_over && self.puzzle.is_some();
        ui.vertical_centered(|ui| {
            ui.label(egui::RichText::new(&self.message).font(font(15.5, W::Regular)).color(INK));
        });
        ui.add_space(8.0);

        if let Some(text) = self.hint_text.clone() {
            let avail = ui.available_width();
            let w = avail.min(760.0);
            ui.horizontal(|ui| {
                ui.add_space(((avail - w) * 0.5).max(0.0));
                ui.vertical(|ui| {
                    ui.set_width(w);
                    egui::Frame::new()
                        .fill(HINT_CARD)
                        .stroke(Stroke::new(1.0, HINT_EDGE))
                        .corner_radius(cr(14.0))
                        .shadow(soft_shadow(2, 6, 0, 26))
                        .inner_margin(egui::Margin::symmetric(14, 11))
                        .show(ui, |ui| {
                            ui.set_width(ui.available_width());
                            ui.horizontal_top(|ui| {
                                let (r, _) = ui.allocate_exact_size(Vec2::splat(26.0), Sense::hover());
                                draw_info_icon(ui.painter(), r.center(), 11.0);
                                ui.add_space(4.0);
                                ui.vertical(|ui| {
                                    ui.set_width(ui.available_width());
                                    ui.label(
                                        egui::RichText::new("WHY THIS CAT")
                                            .font(font(12.0, W::Semi))
                                            .color(HINT_TITLE),
                                    );
                                    ui.label(egui::RichText::new(text).font(font(15.0, W::Regular)).color(HINT_TEXT));
                                });
                            });
                        });
                });
            });
            ui.add_space(10.0);
        }

        if playing {
            let deds = self.deductions();
            let shown: Vec<rules::Deduction> = deds.into_iter().take(3).collect();
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new("FREE DEDUCTIONS · click one to mark its X's")
                        .font(font(11.5, W::Semi))
                        .color(INK_FAINT),
                );
            });
            ui.add_space(6.0);
            let avail = ui.available_width();
            if shown.is_empty() {
                // keep the strip's height constant so the board never jumps
                ui.horizontal(|ui| {
                    let w = button_width(ui, "no deductions yet");
                    ui.add_space(((avail - w) * 0.5).max(0.0));
                    pill_button(ui, "no deductions yet", Kind::Tonal, false);
                });
            } else {
                let spacing = 8.0;
                let labels: Vec<String> = shown
                    .iter()
                    .map(|d| format!("{}  ·  {}", d.title, d.cells.len()))
                    .collect();
                let total: f32 = labels.iter().map(|l| button_width(ui, l)).sum::<f32>()
                    + spacing * (labels.len() as f32 - 1.0);
                let mut clicked: Option<usize> = None;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = spacing;
                    ui.add_space(((avail - total) * 0.5).max(0.0));
                    for (i, label) in labels.iter().enumerate() {
                        let r = pill_button(ui, label, Kind::Tonal, true);
                        if r.clicked() {
                            clicked = Some(i);
                        }
                        r.on_hover_text(&shown[i].why);
                    }
                });
                if let Some(i) = clicked {
                    self.apply_deduction(&shown[i]);
                }
            }
            ui.add_space(8.0);
        }

        let labels = ["Hint", "Extra life", "Clear marks", "New game"];
        let spacing = 10.0;
        let total: f32 =
            labels.iter().map(|l| button_width(ui, l)).sum::<f32>() + spacing * 2.0 + 22.0;
        let avail = ui.available_width();
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = spacing;
            ui.add_space(((avail - total) * 0.5).max(0.0));
            if pill_button(ui, "Hint", Kind::Tonal, playing).clicked() {
                self.give_hint();
            }
            if pill_button(ui, "Extra life", Kind::Tonal, !self.won && self.puzzle.is_some()).clicked() {
                self.buy_extra_life();
            }
            if pill_button(ui, "Clear marks", Kind::Tonal, !self.won).clicked() {
                self.clear_marks();
            }
            ui.add_space(22.0 - spacing);
            if pill_button(ui, "New game", Kind::Primary, true).clicked() {
                self.request_new_puzzle();
            }
        });
        ui.add_space(8.0);
        ui.vertical_centered(|ui| {
            ui.label(
                egui::RichText::new("Click: mark X   ·   Double-click: place a cat   ·   Drag: toggle X marks")
                    .font(font(12.5, W::Regular))
                    .color(INK_SOFT),
            );
        });
    }

    fn board(&mut self, ui: &mut egui::Ui) {
        let ctx = ui.ctx().clone();
        let now = self.now;
        let full = ui.max_rect();
        let avail = full.shrink2(Vec2::new(16.0, 10.0));
        let side = avail.width().min(avail.height()).max(160.0);
        let card = Rect::from_center_size(avail.center(), Vec2::splat(side));
        let pad = (side * 0.04).clamp(12.0, 26.0);
        let well = card.shrink(pad);
        let wpad = (side * 0.014).clamp(4.0, 9.0);
        let inner = well.shrink(wpad);
        let n = self.n;
        let gap = (inner.width() / n as f32 * 0.075).clamp(3.5, 6.0);
        let tile = (inner.width() - gap * (n as f32 - 1.0)) / n as f32;
        let radius = (tile * 0.17).clamp(5.0, 12.0);
        let b = Board { inner, tile, gap, n };
        if !self.dumped && std::env::var_os("CATS_DEBUG").is_some() {
            eprintln!("LAYOUT inner={:?} tile={} gap={}", inner, tile, gap);
            self.dumped = true;
        }

        let response = ui.allocate_rect(card, Sense::click_and_drag());
        let painter = ui.painter().clone();

        // card + well
        painter.add(soft_shadow(10, 34, 0, 34).as_shape(card, cr(18.0)));
        painter.add(soft_shadow(1, 3, 0, 18).as_shape(card, cr(18.0)));
        painter.rect_filled(card, cr(18.0), CARD);
        let well_r = (18.0 - pad * 0.5).max(10.0);
        painter.rect_filled(well, cr(well_r), WELL);
        painter.rect_stroke(
            well,
            cr(well_r),
            Stroke::new(1.0, Color32::from_rgba_unmultiplied(90, 60, 30, 12)),
            StrokeKind::Inside,
        );

        let Some(p) = self.puzzle.clone() else {
            painter.text(
                well.center() - Vec2::new(0.0, 14.0),
                Align2::CENTER_CENTER,
                "Building a puzzle…",
                font(20.0, W::Semi),
                INK_SOFT,
            );
            for i in 0..3 {
                let ph = ((now as f32 * 3.0) - i as f32 * 0.6).sin() * 0.5 + 0.5;
                painter.circle_filled(
                    well.center() + Vec2::new((i as f32 - 1.0) * 16.0, 18.0),
                    3.0 + ph * 2.0,
                    ACCENT.gamma_multiply(0.35 + 0.65 * ph),
                );
            }
            ctx.request_repaint_after(Duration::from_millis(33));
            return;
        };

        // ── input ──
        let active = !self.won && !self.game_over;
        if active {
            let (pressed, down, pos, decided) = ctx.input(|i| {
                (
                    i.pointer.primary_pressed(),
                    i.pointer.primary_down(),
                    i.pointer.interact_pos(),
                    i.pointer.is_decidedly_dragging(),
                )
            });
            if pressed && response.contains_pointer() {
                if let Some(pos) = pos {
                    if let Some(origin) = b.cell_at(pos) {
                        self.drag = Some(Drag { origin, last: pos, painting: false, cur: origin });
                    }
                }
            }
            let mut drag = self.drag.take();
            if let (Some(d), true, Some(pos)) = (drag.as_mut(), down, pos) {
                if !d.painting && (b.cell_at(pos) != Some(d.origin) || decided) {
                    d.painting = true;
                    // the press already toggled the origin cell (single_click runs on
                    // press), so `cur` starts there and it is not flipped twice
                }
                if d.painting {
                    let steps = ((pos - d.last).length() / (tile * 0.25)).ceil().max(1.0) as usize;
                    for s in 1..=steps {
                        let q = d.last + (pos - d.last) * (s as f32 / steps as f32);
                        if let Some(i) = b.cell_at(q) {
                            self.drag_flip(i, &mut d.cur);
                        }
                    }
                }
                d.last = pos;
            }
            let painted = drag.as_ref().is_some_and(|d| d.painting);
            self.drag = if down { drag } else { None };

            if !painted {
                // Clicks are handled from raw press events so a double click is
                // recognised even when two presses land in the same repaint.
                let presses: Vec<Pos2> = ctx.input(|i| {
                    i.events
                        .iter()
                        .filter_map(|e| match e {
                            egui::Event::PointerButton {
                                pos,
                                button: egui::PointerButton::Primary,
                                pressed: true,
                                ..
                            } => Some(*pos),
                            _ => None,
                        })
                        .collect()
                });
                for p in presses {
                    let at = b.cell_at(p);
                    if std::env::var_os("CATS_DEBUG").is_some() {
                        eprintln!("PRESS pos={:?} at={:?} inner={:?}", p, at, b.inner);
                    }
                    let Some(idx) = at else { continue };
                    let dbl = matches!(self.last_press, Some((i, t)) if i == idx && self.now - t < 0.45);
                    if dbl {
                        self.last_press = None;
                        self.double_click(idx);
                    } else {
                        self.last_press = Some((idx, self.now));
                        self.single_click(idx);
                    }
                }
                if let Some(idx) = response.interact_pointer_pos().and_then(|q| b.cell_at(q)) {
                    if response.secondary_clicked() {
                        // right click kept as a plain X toggle
                        match self.cells[idx] {
                            Cell::Empty => self.set_cell(idx, Cell::X),
                            Cell::X => self.set_cell(idx, Cell::Empty),
                            Cell::Cat => {}
                        }
                    }
                }
            }
        } else {
            self.drag = None;
        }
        let active = !self.won && !self.game_over;
        let hovered = if active { response.hover_pos().and_then(|q| b.cell_at(q)) } else { None };
        if hovered.is_some() {
            ctx.set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        // ── same-region connectors (drawn under the tiles) ──
        for r in 0..n {
            for c in 0..n {
                let g = p.region_at(r, c);
                let col = shade(region_color(g, n), 0.9);
                let a = b.rect(r, c);
                if c + 1 < n && p.region_at(r, c + 1) == g {
                    let nb = b.rect(r, c + 1);
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(a.right() - 1.0, a.top() + radius),
                            Pos2::new(nb.left() + 1.0, a.bottom() - radius),
                        ),
                        CornerRadius::ZERO,
                        col,
                    );
                }
                if r + 1 < n && p.region_at(r + 1, c) == g {
                    let nb = b.rect(r + 1, c);
                    painter.rect_filled(
                        Rect::from_min_max(
                            Pos2::new(a.left() + radius, a.bottom() - 1.0),
                            Pos2::new(a.right() - radius, nb.top() + 1.0),
                        ),
                        CornerRadius::ZERO,
                        col,
                    );
                }
            }
        }

        // ── glows / lift shadows, under every tile so neighbours don't clip them ──
        let lift = Vec2::new(0.0, -1.5);
        let hint_pulse = self.hint_at.map(|t| ((now - t) as f32 * 3.4).sin() * 0.5 + 0.5).unwrap_or(0.0);
        if let Some((hr, hc)) = self.hint {
            painter.add(
                Shadow {
                    offset: [0, 0],
                    blur: 18,
                    spread: (2.0 + hint_pulse * 4.0) as u8,
                    color: Color32::from_rgba_unmultiplied(232, 140, 60, (90.0 + 90.0 * hint_pulse) as u8),
                }
                .as_shape(b.rect(hr, hc), cr(radius)),
            );
        }
        if let Some(h) = hovered {
            painter.add(soft_shadow(3, 10, 0, 46).as_shape(b.rect(h / n, h % n).translate(lift), cr(radius)));
        }

        // ── tiles ──
        let mut animating = false;
        for r in 0..n {
            for c in 0..n {
                let idx = r * n + c;
                let g = p.region_at(r, c);
                let base = region_color(g, n);
                let mut rect = b.rect(r, c);
                let is_hover = hovered == Some(idx);
                let is_hint = self.hint == Some((r, c));

                // wrong-cat shake
                let mut flash = 0.0;
                if let Some(((wr, wc), t)) = self.wrong {
                    if (wr, wc) == (r, c) {
                        let dt = (now - t) as f32;
                        if dt < 0.7 {
                            animating = true;
                            flash = 1.0 - dt / 0.7;
                            if dt < 0.42 {
                                let amp = tile * 0.05 * (1.0 - dt / 0.42);
                                rect = rect.translate(Vec2::new((dt * 52.0).sin() * amp, 0.0));
                            }
                        }
                    }
                }
                if is_hover {
                    rect = rect.translate(lift);
                }

                let fill = if is_hover { mix(base, Color32::WHITE, 0.16) } else { base };
                painter.rect_filled(rect, cr(radius), fill);
                painter.rect_stroke(rect, cr(radius), Stroke::new(1.0, shade(base, 0.90)), StrokeKind::Inside);

                // Region outline: darken only the tile edges that meet a
                // different colour, so each region reads as one solid shape.
                let edge = Stroke::new(2.6, shade(base, 0.66));
                let inset = 1.0;
                let (l, t, rr, bb) = (rect.left() + inset, rect.top() + inset, rect.right() - inset, rect.bottom() - inset);
                if r == 0 || p.region_at(r - 1, c) != g {
                    painter.line_segment([egui::pos2(l, t), egui::pos2(rr, t)], edge);
                }
                if r + 1 == n || p.region_at(r + 1, c) != g {
                    painter.line_segment([egui::pos2(l, bb), egui::pos2(rr, bb)], edge);
                }
                if c == 0 || p.region_at(r, c - 1) != g {
                    painter.line_segment([egui::pos2(l, t), egui::pos2(l, bb)], edge);
                }
                if c + 1 == n || p.region_at(r, c + 1) != g {
                    painter.line_segment([egui::pos2(rr, t), egui::pos2(rr, bb)], edge);
                }

                if is_hint {
                    painter.rect_filled(rect, cr(radius), ACCENT.gamma_multiply(0.10 + 0.12 * hint_pulse));
                    painter.rect_stroke(
                        rect.expand(1.5 + hint_pulse * 1.5),
                        cr(radius + 2.0),
                        Stroke::new(2.5, ACCENT.gamma_multiply(0.75 + 0.25 * hint_pulse)),
                        StrokeKind::Outside,
                    );
                }
                if flash > 0.0 {
                    painter.rect_filled(rect, cr(radius), WRONG.gamma_multiply(0.6 * flash));
                }
                if is_hover {
                    painter.rect_stroke(
                        rect,
                        cr(radius),
                        Stroke::new(2.0, Color32::from_rgba_unmultiplied(255, 255, 255, 230)),
                        StrokeKind::Outside,
                    );
                    painter.rect_stroke(
                        rect.expand(2.0),
                        cr(radius + 2.0),
                        Stroke::new(1.0, INK.gamma_multiply(0.18)),
                        StrokeKind::Outside,
                    );
                }

                let age = (now - self.changed_at[idx]) as f32;
                match self.cells[idx] {
                    Cell::Cat => {
                        let mut s = if age < 0.18 {
                            animating = true;
                            0.55 + 0.45 * ease_out_back(age / 0.16)
                        } else {
                            1.0
                        };
                        if self.won {
                            let t = (now - self.ended_at) as f32 - (r + c) as f32 * 0.035;
                            if (0.0..0.4).contains(&t) {
                                s *= 1.0 + 0.1 * (PI * t / 0.4).sin();
                            }
                            if t < 0.4 {
                                animating = true;
                            }
                        }
                        let clip = painter.with_clip_rect(rect.intersect(painter.clip_rect()));
                        draw_cat(&clip, rect.center(), tile * s * 1.07, 1.0);
                    }
                    Cell::X => {
                        if age < 0.2 {
                            animating = true;
                        }
                        draw_x(&painter, rect, age);
                    }
                    Cell::Empty => {}
                }
            }
        }
        if self.hint.is_some() {
            ctx.request_repaint_after(Duration::from_millis(30));
        }

        // ── end-of-run panel ──
        if self.won || self.game_over {
            let t = (now - self.ended_at) as f32;
            let e = ease_out(t / 0.35);
            if t < 0.9 {
                animating = true;
            }
            painter.rect_filled(well, cr(well_r), CARD.gamma_multiply(0.55 * e));
            let w = (well.width() * 0.62).clamp(240.0, 400.0);
            let h = 188.0f32.min(well.height() * 0.8);
            let panel = Rect::from_center_size(
                well.center() + Vec2::new(0.0, 10.0 * (1.0 - e)),
                Vec2::new(w, h) * (0.94 + 0.06 * e),
            );
            painter.add(soft_shadow(10, 30, 0, (52.0 * e) as u8).as_shape(panel, cr(18.0)));
            painter.rect_filled(panel, cr(18.0), CARD.gamma_multiply(e));
            painter.rect_stroke(panel, cr(18.0), Stroke::new(1.0, HAIRLINE.gamma_multiply(e)), StrokeKind::Inside);
            let top = panel.top();
            let cx = panel.center().x;
            let badge = Rect::from_center_size(Pos2::new(cx, top + 38.0), Vec2::splat(44.0));
            let (badge_col, title) = if self.won { (ACCENT, "Solved") } else { (INK_SOFT, "Out of lives") };
            painter.rect_filled(badge, cr(13.0), badge_col.gamma_multiply(e));
            draw_cat(&painter.with_clip_rect(badge.shrink(1.0)), badge.center() + Vec2::new(0.0, 1.0), 44.0, e);
            painter.text(
                Pos2::new(cx, top + 88.0),
                Align2::CENTER_CENTER,
                title,
                font(30.0, W::Bold),
                INK.gamma_multiply(e),
            );
            let time = fmt_time(self.elapsed.as_millis() as u64);
            let (sub, tag, tag_col) = if self.won {
                if self.assisted {
                    (format!("Time {time}"), String::from("Assisted — not counted"), AMBER)
                } else {
                    (format!("Time {time}"), format!("Clean win #{}", self.stats.clean_wins), SAGE)
                }
            } else {
                (String::from("Take an extra life, or start a fresh board"), String::new(), INK_SOFT)
            };
            painter.text(
                Pos2::new(cx, top + 120.0),
                Align2::CENTER_CENTER,
                sub,
                font(15.5, W::Regular),
                INK_SOFT.gamma_multiply(e),
            );
            if !tag.is_empty() {
                let g = painter.layout_no_wrap(tag, font(13.0, W::Semi), tag_col);
                let r = Rect::from_center_size(Pos2::new(cx, top + 152.0), g.size() + Vec2::new(24.0, 10.0));
                painter.rect_filled(r, cr(13.0), tag_col.gamma_multiply(0.15 * e));
                painter.galley(r.center() - g.size() * 0.5, g, tag_col.gamma_multiply(e));
            }
        }

        if animating {
            ctx.request_repaint();
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.now = ctx.input(|i| i.time);
        if std::env::var_os("CATS_DEBUG").is_some() && (self.message != self.last_msg || self.hint_text != self.last_hint) {
            eprintln!("MSG={:?} HINT={:?}", self.message, self.hint_text);
            self.last_msg = self.message.clone();
            self.last_hint = self.hint_text.clone();
        }
        self.poll_generator(&ctx);

        if self.start.is_some() && !self.won && !self.game_over {
            self.elapsed = self.start.map(|s| s.elapsed()).unwrap_or_default();
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        if let Some((_, t)) = self.wrong {
            if self.now - t > 0.8 {
                self.wrong = None;
            }
        }

        let bare = |m: egui::Margin| egui::Frame::new().fill(BG).inner_margin(m);

        egui::Panel::top("hud")
            .frame(bare(egui::Margin { left: 22, right: 22, top: 16, bottom: 6 }))
            .show_separator_line(false)
            .resizable(false)
            .show(ui, |ui| self.header(ui));

        egui::Panel::bottom("controls")
            .frame(bare(egui::Margin { left: 22, right: 22, top: 6, bottom: 14 }))
            .show_separator_line(false)
            .resizable(false)
            .show(ui, |ui| self.controls(ui));

        egui::CentralPanel::default()
            .frame(bare(egui::Margin::same(0)))
            .show(ui, |ui| self.board(ui));
    }
}

fn main() -> eframe::Result<()> {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--selftest") {
        let t = Instant::now();
        for seed in 0..10u64 {
            match puzzle::generate(DEFAULT_N, seed + 1000) {
                Some(rep) => println!(
                    "seed {:>4}: {} attempts, {:>5} ms, unique solution verified",
                    seed,
                    rep.attempts,
                    rep.elapsed.as_millis()
                ),
                None => println!("seed {:>4}: FAILED", seed),
            }
        }
        println!("total {:?}", t.elapsed());
        return Ok(());
    }

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1000.0, 960.0])
            .with_min_inner_size([620.0, 700.0])
            .with_title("Cats Grid"),
        ..Default::default()
    };
    eframe::run_native("Cats Grid", options, Box::new(|cc| Ok(Box::new(App::new(cc)))))
}
