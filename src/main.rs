//! Cats Grid — one cat per row, per column, per color, no two touching.
//! Native desktop build for the G760. Optional hints, extra lives, and a
//! win counter that only credits clean (unaided) solves.

mod puzzle;
mod stats;

use eframe::egui::{self, Color32, CornerRadius, Rect, Sense, Shape, Stroke, Vec2};
use puzzle::{solutions, Puzzle, DEFAULT_N};
use stats::Stats;
use std::sync::mpsc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Cell {
    Empty,
    Cat,
    X,
}

struct App {
    n: usize,
    puzzle: Option<Puzzle>,
    cells: Vec<Cell>,
    lives: u8,
    max_lives: u8,
    hint: Option<(usize, usize)>,
    hint_at: Option<Instant>,
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
    message: String,
    stats: Stats,
    rx: Option<mpsc::Receiver<Option<puzzle::GenReport>>>,
    wrong: Option<((usize, usize), Instant)>,
    gen_ms: Option<u128>,
}

impl App {
    fn new(_cc: &eframe::CreationContext<'_>) -> Self {
        let stats = Stats::load();
        let mut app = App {
            n: DEFAULT_N,
            puzzle: None,
            cells: vec![Cell::Empty; DEFAULT_N * DEFAULT_N],
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
            message: String::from("Building a puzzle…"),
            stats,
            rx: None,
            wrong: None,
            gen_ms: None,
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
                    self.puzzle = Some(rep.puzzle);
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
        self.cells[idx] = Cell::Cat;
        let cats = self.cats();
        // A legal-looking cat that still admits no completion is a wrong cat.
        if solutions(self.n, &p.region, &cats, &[], 1).is_empty() {
            self.cells[idx] = Cell::X;
            self.fail_cell(r, c, "no solution is left with a cat there");
            return;
        }
        self.hint = None;
        self.hint_text = None;
        if cats.len() == self.n {
            self.won = true;
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
            self.message = format!("{} cats placed, {} to go.", cats.len(), self.n - cats.len());
        }
    }

    fn fail_cell(&mut self, r: usize, c: usize, why: &str) {
        self.wrong = Some(((r, c), Instant::now()));
        if self.lives > 0 {
            self.lives -= 1;
        }
        self.hint = None;
        self.hint_text = None;
        if self.lives == 0 {
            self.game_over = true;
            self.elapsed = self.start.map(|s| s.elapsed()).unwrap_or_default();
            self.message = format!("Out of lives — {}.", why);
        } else {
            self.message = format!(
                "Not a cat: {}. Lost a life ({} left).",
                why, self.lives
            );
        }
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
        let Some((target, why)) = puzzle::best_hint(
            self.n,
            &p.region,
            &cats,
            &sol,
            &|g| color_name(g, self.n),
        ) else {
            return;
        };
        self.assisted = true;
        self.hints_used += 1;
        self.hint = Some(target);
        self.hint_at = Some(Instant::now());
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
        for cell in self.cells.iter_mut() {
            *cell = Cell::Empty;
        }
        self.hint = None;
        self.hint_text = None;
        self.message = String::from("Marks cleared.");
    }
}

fn fmt_time(ms: u64) -> String {
    let s = ms / 1000;
    format!("{}:{:02}", s / 60, s % 60)
}

/// Pastel palette from evenly spaced hues; stable per region.
fn region_color(g: u8, n: usize) -> Color32 {
    let hue = (g as f32 * 360.0 / n as f32 + 12.0) % 360.0;
    hsv(hue, 0.34, 0.96)
}

fn color_name(g: u8, n: usize) -> &'static str {
    let hue = (g as f32 * 360.0 / n as f32 + 12.0) % 360.0;
    match hue as u32 {
        0..=14 => "pink",
        15..=44 => "apricot",
        45..=74 => "butter",
        75..=104 => "lime",
        105..=134 => "mint",
        135..=164 => "green",
        165..=194 => "teal",
        195..=224 => "sky",
        225..=254 => "blue",
        255..=284 => "violet",
        285..=314 => "lilac",
        315..=344 => "rose",
        _ => "coral",
    }
}

fn hsv(h: f32, s: f32, v: f32) -> Color32 {
    let c = v * s;
    let hp = (h % 360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r1, g1, b1) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = v - c;
    Color32::from_rgb(
        ((r1 + m) * 255.0) as u8,
        ((g1 + m) * 255.0) as u8,
        ((b1 + m) * 255.0) as u8,
    )
}

fn draw_cat(p: &egui::Painter, cell: Rect) {
    let ink = Color32::from_rgb(48, 42, 55);
    let k = cell.width();
    let c = cell.center() + Vec2::new(0.0, k * 0.03);
    let head = k * 0.235;
    // ears
    p.add(Shape::convex_polygon(
        vec![
            c + Vec2::new(-head * 0.95, -head * 0.55),
            c + Vec2::new(-head * 1.02, -head * 1.55),
            c + Vec2::new(-head * 0.18, -head * 0.98),
        ],
        ink,
        Stroke::NONE,
    ));
    p.add(Shape::convex_polygon(
        vec![
            c + Vec2::new(head * 0.95, -head * 0.55),
            c + Vec2::new(head * 1.02, -head * 1.55),
            c + Vec2::new(head * 0.18, -head * 0.98),
        ],
        ink,
        Stroke::NONE,
    ));
    p.circle_filled(c, head, ink);
    // eyes
    let eye = Color32::from_rgb(240, 232, 190);
    p.circle_filled(c + Vec2::new(-head * 0.38, -head * 0.08), head * 0.17, eye);
    p.circle_filled(c + Vec2::new(head * 0.38, -head * 0.08), head * 0.17, eye);
    // nose + whiskers
    p.add(Shape::convex_polygon(
        vec![
            c + Vec2::new(0.0, head * 0.12),
            c + Vec2::new(-head * 0.12, head * 0.28),
            c + Vec2::new(head * 0.12, head * 0.28),
        ],
        Color32::from_rgb(235, 160, 170),
        Stroke::NONE,
    ));
    let whisker = Stroke::new((k * 0.014).max(1.0), ink);
    for dy in [-0.10, 0.06] {
        p.line_segment(
            [c + Vec2::new(head * 0.35, head * (0.30 + dy)), c + Vec2::new(head * 1.38, head * (0.16 + dy))],
            whisker,
        );
        p.line_segment(
            [c + Vec2::new(-head * 0.35, head * (0.30 + dy)), c + Vec2::new(-head * 1.38, head * (0.16 + dy))],
            whisker,
        );
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        self.poll_generator(&ctx);

        if self.start.is_some() && !self.won && !self.game_over {
            self.elapsed = self.start.map(|s| s.elapsed()).unwrap_or_default();
            ctx.request_repaint_after(Duration::from_millis(250));
        }
        if let Some((_, t)) = self.wrong {
            if t.elapsed() < Duration::from_millis(700) {
                ctx.request_repaint_after(Duration::from_millis(40));
            } else {
                self.wrong = None;
            }
        }

        egui::Panel::top("hud").show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new("Cats Grid").size(24.0).strong());
                ui.add_space(14.0);
                ui.label(egui::RichText::new(format!("Lives {}/{}", self.lives, self.max_lives)).size(17.0).color(
                    if self.lives >= 2 { Color32::from_rgb(30, 120, 60) } else { Color32::from_rgb(180, 40, 40) },
                ));
                ui.add_space(12.0);
                ui.label(egui::RichText::new(format!("Time {}", fmt_time(self.elapsed.as_millis() as u64))).size(17.0));
                ui.add_space(12.0);
                ui.label(
                    egui::RichText::new(format!("Clean wins {}", self.stats.clean_wins))
                        .size(17.0)
                        .strong()
                        .color(Color32::from_rgb(20, 110, 60)),
                );
                ui.label(egui::RichText::new(format!("(assisted {})", self.stats.assisted_wins)).size(14.0).weak());
                if self.assisted {
                    ui.add_space(10.0);
                    ui.label(
                        egui::RichText::new("assisted run")
                            .size(14.0)
                            .color(Color32::from_rgb(170, 90, 20)),
                    );
                }
                if let Some(best) = self.stats.best_clean_ms {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(egui::RichText::new(format!("best clean {}", fmt_time(best))).size(14.0).weak());
                    });
                }
            });
            ui.add_space(4.0);
        });

        egui::Panel::bottom("controls").show(ui, |ui| {
            ui.add_space(6.0);
            ui.horizontal(|ui| {
                if ui.add_enabled(!self.won && !self.game_over && self.puzzle.is_some(), egui::Button::new(egui::RichText::new("Hint").size(18.0))).clicked() {
                    self.give_hint();
                }
                if ui
                    .add_enabled(!self.won && self.puzzle.is_some(), egui::Button::new(egui::RichText::new("Extra life").size(18.0)))
                    .clicked()
                {
                    self.buy_extra_life();
                }
                if ui.add_enabled(!self.won, egui::Button::new(egui::RichText::new("Clear marks").size(18.0))).clicked() {
                    self.clear_marks();
                }
                ui.add_space(10.0);
                if ui.button(egui::RichText::new("New game").size(18.0)).clicked() {
                    self.request_new_puzzle();
                }
                ui.add_space(14.0);
                ui.label(egui::RichText::new(&self.message).size(16.0));
            });
            if let Some(text) = self.hint_text.clone() {
                ui.add_space(6.0);
                egui::Frame::new()
                    .fill(Color32::from_rgb(255, 248, 227))
                    .stroke(Stroke::new(1.0, Color32::from_rgb(226, 196, 126)))
                    .corner_radius(CornerRadius::same(8))
                    .inner_margin(egui::Margin::symmetric(10, 8))
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        ui.label(
                            egui::RichText::new("Why this cat")
                                .size(13.0)
                                .strong()
                                .color(Color32::from_rgb(146, 99, 18)),
                        );
                        ui.label(
                            egui::RichText::new(text)
                                .size(15.5)
                                .color(Color32::from_rgb(58, 44, 18)),
                        );
                    });
            }
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new("Left click: place a cat. Right click: mark a cell you ruled out.")
                    .size(13.0)
                    .weak(),
            );
            ui.add_space(4.0);
        });

        egui::CentralPanel::default().show(ui, |ui| {
            let full = ui.max_rect();
            let side = (full.width().min(full.height()) - 12.0).max(120.0);
            let grid = Rect::from_center_size(full.center(), Vec2::splat(side));
            let response = ui.allocate_rect(full, Sense::click());

            let Some(p) = self.puzzle.clone() else {
                ui.painter().text(
                    full.center(),
                    egui::Align2::CENTER_CENTER,
                    "Building a puzzle…",
                    egui::FontId::proportional(26.0),
                    Color32::GRAY,
                );
                return;
            };

            let cell = side / self.n as f32;
            let painter = ui.painter();

            for r in 0..self.n {
                for c in 0..self.n {
                    let rect = Rect::from_min_size(
                        grid.min + Vec2::new(c as f32 * cell, r as f32 * cell),
                        Vec2::splat(cell),
                    );
                    let g = p.region_at(r, c);
                    painter.rect_filled(rect, CornerRadius::ZERO, region_color(g, self.n));

                    if let Some(((wr, wc), t)) = self.wrong {
                        if wr == r && wc == c {
                            let a = 1.0 - (t.elapsed().as_secs_f32() / 0.7).min(1.0);
                            painter.rect_filled(
                                rect,
                                CornerRadius::ZERO,
                                Color32::from_rgba_unmultiplied(220, 40, 40, (150.0 * a) as u8),
                            );
                        }
                    }

                    match self.cells[r * self.n + c] {
                        Cell::Cat => draw_cat(painter, rect),
                        Cell::X => {
                            let st = Stroke::new((cell * 0.07).max(2.0), Color32::from_rgb(90, 90, 105));
                            let pad = cell * 0.24;
                            painter.line_segment(
                                [rect.left_top() + Vec2::splat(pad), rect.right_bottom() - Vec2::splat(pad)],
                                st,
                            );
                            painter.line_segment(
                                [rect.right_top() + Vec2::new(-pad, pad), rect.left_bottom() + Vec2::new(pad, -pad)],
                                st,
                            );
                        }
                        Cell::Empty => {}
                    }
                }
            }

            // region borders + outer frame
            let border = Stroke::new((cell * 0.045).max(2.0), Color32::from_rgb(45, 45, 55));
            let thin = Stroke::new(1.0, Color32::from_rgba_unmultiplied(70, 70, 80, 90));
            for r in 0..self.n {
                for c in 0..self.n {
                    let rect = Rect::from_min_size(
                        grid.min + Vec2::new(c as f32 * cell, r as f32 * cell),
                        Vec2::splat(cell),
                    );
                    painter.line_segment([rect.left_top(), rect.right_top()], thin);
                    painter.line_segment([rect.left_top(), rect.left_bottom()], thin);
                    if c + 1 < self.n && p.region_at(r, c) != p.region_at(r, c + 1) {
                        painter.line_segment(
                            [rect.right_top(), rect.right_bottom()],
                            border,
                        );
                    }
                    if r + 1 < self.n && p.region_at(r, c) != p.region_at(r + 1, c) {
                        painter.line_segment(
                            [rect.left_bottom(), rect.right_bottom()],
                            border,
                        );
                    }
                }
            }
            painter.rect_stroke(grid, CornerRadius::ZERO, border, egui::StrokeKind::Outside);

            // hint pulse
            if let Some((hr, hc)) = self.hint {
                let rect = Rect::from_min_size(
                    grid.min + Vec2::new(hc as f32 * cell, hr as f32 * cell),
                    Vec2::splat(cell),
                );
                let t = self.hint_at.map(|s| s.elapsed().as_secs_f32()).unwrap_or(0.0);
                let pulse = 0.55 + 0.45 * (t * 4.0).sin().abs();
                painter.rect_stroke(
                    rect.shrink(cell * 0.05),
                    CornerRadius::ZERO,
                    Stroke::new((cell * 0.055).max(3.0), Color32::from_rgba_unmultiplied(25, 150, 70, (255.0 * pulse) as u8)),
                    egui::StrokeKind::Inside,
                );
                ctx.request_repaint_after(Duration::from_millis(40));
            }

            if (self.won || self.game_over) && response.clicked() {
                // nothing: avoids accidental board edits behind the overlay
            }

            if !self.won && !self.game_over {
                if let Some(pos) = response.interact_pointer_pos() {
                    if grid.contains(pos) {
                        let cc = (((pos.x - grid.min.x) / cell).floor() as isize).clamp(0, self.n as isize - 1) as usize;
                        let rr = (((pos.y - grid.min.y) / cell).floor() as isize).clamp(0, self.n as isize - 1) as usize;
                        if response.clicked() {
                            match self.cells[rr * self.n + cc] {
                                Cell::Cat => self.cells[rr * self.n + cc] = Cell::Empty,
                                Cell::X => self.cells[rr * self.n + cc] = Cell::Empty,
                                Cell::Empty => self.place_cat(rr, cc),
                            }
                        } else if response.secondary_clicked() {
                            self.cells[rr * self.n + cc] = match self.cells[rr * self.n + cc] {
                                Cell::Empty => Cell::X,
                                Cell::X => Cell::Empty,
                                Cell::Cat => Cell::Cat,
                            };
                        }
                    }
                }
            }

            if self.won || self.game_over {
                let box_rect = Rect::from_center_size(grid.center(), Vec2::new(side * 0.62, side * 0.26));
                painter.rect_filled(box_rect, CornerRadius::same(14), Color32::from_rgba_unmultiplied(20, 22, 28, 232));
                let title = if self.won { "Solved" } else { "Out of lives" };
                painter.text(
                    box_rect.center() - Vec2::new(0.0, side * 0.045),
                    egui::Align2::CENTER_CENTER,
                    title,
                    egui::FontId::proportional(side * 0.055),
                    Color32::from_rgb(245, 246, 250),
                );
                let sub = if self.won {
                    if self.assisted {
                        format!("{} — assisted, not counted", fmt_time(self.elapsed.as_millis() as u64))
                    } else {
                        format!("{} — clean win #{}", fmt_time(self.elapsed.as_millis() as u64), self.stats.clean_wins)
                    }
                } else {
                    String::from("Extra life, or start a fresh board")
                };
                painter.text(
                    box_rect.center() + Vec2::new(0.0, side * 0.012),
                    egui::Align2::CENTER_CENTER,
                    sub,
                    egui::FontId::proportional(side * 0.026),
                    Color32::from_rgb(205, 208, 216),
                );
                painter.text(
                    box_rect.center() + Vec2::new(0.0, side * 0.062),
                    egui::Align2::CENTER_CENTER,
                    "use the buttons below",
                    egui::FontId::proportional(side * 0.02),
                    Color32::from_rgb(150, 155, 165),
                );
            }
        });
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
    eframe::run_native(
        "Cats Grid",
        options,
        Box::new(|cc| Ok(Box::new(App::new(cc)))),
    )
}
