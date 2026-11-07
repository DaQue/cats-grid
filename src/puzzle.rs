//! Cats Grid rules engine: unique-solution generator + exact solver.
//!
//! Rules (Star Battle family): exactly one cat per row, per column and per
//! colored region, and no two cats may touch — orthogonally or diagonally.

use std::time::{Duration, Instant};

pub const DEFAULT_N: usize = 10;

#[derive(Clone, Debug)]
pub struct Puzzle {
    pub n: usize,
    /// Region id per cell, row-major, values 0..n.
    pub region: Vec<u8>,
    /// The one valid placement.
    pub solution: Vec<(usize, usize)>,
}

impl Puzzle {
    #[inline]
    pub fn region_at(&self, r: usize, c: usize) -> u8 {
        self.region[r * self.n + c]
    }
}

/// Small deterministic PRNG (xorshift64*) so the crate needs no rand dependency.
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Rng(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1)
    }
    #[inline]
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }
    pub fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
    pub fn unit(&mut self) -> f64 {
        (self.next() >> 11) as f64 / (1u64 << 53) as f64
    }
    pub fn shuffle<T>(&mut self, v: &mut [T]) {
        for i in (1..v.len()).rev() {
            let j = self.below(i + 1);
            v.swap(i, j);
        }
    }
}

#[inline]
fn neighbours(n: usize, r: usize, c: usize, out: &mut Vec<usize>) {
    out.clear();
    if r > 0 {
        out.push((r - 1) * n + c);
    }
    if r + 1 < n {
        out.push((r + 1) * n + c);
    }
    if c > 0 {
        out.push(r * n + c - 1);
    }
    if c + 1 < n {
        out.push(r * n + c + 1);
    }
}

// ---------------------------------------------------------------- solver ----

struct Search<'a> {
    n: usize,
    region: &'a [u8],
    excluded: Vec<bool>,
    /// Forced column per row, if the caller pinned a cat in that row.
    given: Vec<Option<usize>>,
    limit: usize,
    found: Vec<Vec<(usize, usize)>>,
    region_cells: Vec<Vec<(usize, usize)>>,
}

impl<'a> Search<'a> {
    fn new(n: usize, region: &'a [u8], cats: &[(usize, usize)], xs: &[(usize, usize)], limit: usize) -> Self {
        let mut excluded = vec![false; n * n];
        for &(r, c) in xs {
            excluded[r * n + c] = true;
        }
        let mut given = vec![None; n];
        for &(r, c) in cats {
            given[r] = Some(c);
        }
        let mut region_cells = vec![Vec::new(); n];
        for r in 0..n {
            for c in 0..n {
                region_cells[region[r * n + c] as usize].push((r, c));
            }
        }
        Search {
            n,
            region,
            excluded,
            given,
            limit,
            found: Vec::new(),
            region_cells,
        }
    }

    /// Is every region that still needs a cat placeable somewhere in the rows left?
    fn regions_feasible(&self, row: usize, used_col: u32, used_reg: u32, prev_col: Option<usize>) -> bool {
        for g in 0..self.n {
            if used_reg >> g & 1 == 1 {
                continue;
            }
            let mut ok = false;
            for &(r, c) in &self.region_cells[g] {
                if r < row || self.excluded[r * self.n + c] || used_col >> c & 1 == 1 {
                    continue;
                }
                if r == row {
                    if let Some(p) = prev_col {
                        if c.abs_diff(p) < 2 {
                            continue;
                        }
                    }
                }
                ok = true;
                break;
            }
            if !ok {
                return false;
            }
        }
        true
    }

    fn rec(&mut self, row: usize, used_col: u32, used_reg: u32, prev_col: Option<usize>, path: &mut Vec<(usize, usize)>) {
        if self.found.len() >= self.limit {
            return;
        }
        if row == self.n {
            self.found.push(path.clone());
            return;
        }
        if !self.regions_feasible(row, used_col, used_reg, prev_col) {
            return;
        }
        let candidates: Vec<usize> = match self.given[row] {
            Some(c) => vec![c],
            None => (0..self.n).collect(),
        };
        for c in candidates {
            let idx = row * self.n + c;
            if self.excluded[idx] || used_col >> c & 1 == 1 {
                continue;
            }
            if let Some(p) = prev_col {
                if c.abs_diff(p) < 2 {
                    continue;
                }
            }
            let g = self.region[idx] as usize;
            if used_reg >> g & 1 == 1 {
                continue;
            }
            path.push((row, c));
            self.rec(
                row + 1,
                used_col | 1 << c,
                used_reg | 1 << g,
                Some(c),
                path,
            );
            path.pop();
            if self.found.len() >= self.limit {
                return;
            }
        }
    }
}

/// Up to `limit` solutions consistent with the pinned cats and excluded cells.
pub fn solutions(
    n: usize,
    region: &[u8],
    cats: &[(usize, usize)],
    xs: &[(usize, usize)],
    limit: usize,
) -> Vec<Vec<(usize, usize)>> {
    let mut s = Search::new(n, region, cats, xs, limit);
    let mut path = Vec::with_capacity(n);
    s.rec(0, 0, 0, None, &mut path);
    s.found
}


// --------------------------------------------------------------- hints -----

/// Board state used to justify a hint: where the cats already sit, so the
/// explainer can say *why* a square is dead rather than only that it is.
struct Board<'a> {
    n: usize,
    region: &'a [u8],
    cats: &'a [(usize, usize)],
    row_cat: Vec<Option<usize>>,
    col_cat: Vec<Option<usize>>,
    reg_cat: Vec<Option<(usize, usize)>>,
}

impl<'a> Board<'a> {
    fn new(n: usize, region: &'a [u8], cats: &'a [(usize, usize)]) -> Self {
        let nr = region.iter().copied().max().unwrap_or(0) as usize + 1;
        let mut b = Self {
            n,
            region,
            cats,
            row_cat: vec![None; n],
            col_cat: vec![None; n],
            reg_cat: vec![None; nr],
        };
        for &(r, c) in cats {
            b.row_cat[r] = Some(c);
            b.col_cat[c] = Some(r);
            b.reg_cat[region[r * n + c] as usize] = Some((r, c));
        }
        b
    }

    /// Why a cat cannot sit at `(r, c)` right now — `None` when it still can.
    fn blocked(&self, r: usize, c: usize, color: &dyn Fn(u8) -> &'static str) -> Option<String> {
        let g = self.region[r * self.n + c] as usize;
        if let Some(oc) = self.row_cat[r] {
            if oc != c {
                return Some(format!("row {} already has its cat in column {}", r + 1, oc + 1));
            }
        }
        if let Some(or) = self.col_cat[c] {
            if or != r {
                return Some(format!("column {} already has its cat in row {}", c + 1, or + 1));
            }
        }
        if let Some((cr, cc)) = self.reg_cat[g] {
            if (cr, cc) != (r, c) {
                return Some(format!("the {} region already holds its cat", color(g as u8)));
            }
        }
        for &(cr, cc) in self.cats {
            if (cr, cc) == (r, c) {
                continue;
            }
            if cr.abs_diff(r) <= 1 && cc.abs_diff(c) <= 1 {
                return Some(format!("it touches the cat at row {}, column {}", cr + 1, cc + 1));
            }
        }
        None
    }

    fn open_in_row(&self, r: usize, skip: usize, color: &dyn Fn(u8) -> &'static str) -> Vec<usize> {
        (0..self.n)
            .filter(|&c| c != skip && self.blocked(r, c, color).is_none())
            .collect()
    }

    fn open_in_col(&self, c: usize, skip: usize, color: &dyn Fn(u8) -> &'static str) -> Vec<usize> {
        (0..self.n)
            .filter(|&r| r != skip && self.blocked(r, c, color).is_none())
            .collect()
    }

    fn cells_of_region(&self, g: u8) -> Vec<(usize, usize)> {
        (0..self.n * self.n)
            .map(|i| (i / self.n, i % self.n))
            .filter(|&(r, c)| self.region[r * self.n + c] == g)
            .collect()
    }
}

/// Name a unit that has no square left for its cat.
fn starved(
    n: usize,
    region: &[u8],
    cats: &[(usize, usize)],
    color: &dyn Fn(u8) -> &'static str,
) -> Option<String> {
    let b = Board::new(n, region, cats);
    for g in 0..b.reg_cat.len() {
        if b.reg_cat[g].is_some() {
            continue;
        }
        let cells = b.cells_of_region(g as u8);
        if !cells.is_empty() && cells.iter().all(|&(r, c)| b.blocked(r, c, color).is_some()) {
            return Some(format!("the {} region with no square left for its cat", color(g as u8)));
        }
    }
    for r in 0..n {
        if b.row_cat[r].is_none() && (0..n).all(|c| b.blocked(r, c, color).is_some()) {
            return Some(format!("row {} with no square left for its cat", r + 1));
        }
    }
    for c in 0..n {
        if b.col_cat[c].is_none() && (0..n).all(|r| b.blocked(r, c, color).is_some()) {
            return Some(format!("column {} with no square left for its cat", c + 1));
        }
    }
    None
}

/// Forced-move steps taken, and the unit that ran out of room.
type Chain = (Vec<String>, String);

/// Play out the moves that are forced after `seed`, using only the counting a
/// player can do on sight: a unit with one open square has to take it, and a
/// unit with none is lost. On failure, hand back the chain of forced moves and
/// the unit that ran out of room — that chain *is* the explanation.
fn propagate(
    n: usize,
    region: &[u8],
    seed: &[(usize, usize)],
    color: &dyn Fn(u8) -> &'static str,
) -> Result<Vec<(usize, usize)>, Chain> {
    let mut cats = seed.to_vec();
    let mut steps: Vec<String> = Vec::new();
    for _ in 0..n * 2 {
        if let Some(dead) = starved(n, region, &cats, color) {
            return Err((steps, dead));
        }
        let b = Board::new(n, region, &cats);
        let mut forced: Option<((usize, usize), String)> = None;

        for r in 0..n {
            if b.row_cat[r].is_some() {
                continue;
            }
            let open = b.open_in_row(r, usize::MAX, color);
            if open.len() == 1 {
                forced = Some((
                    (r, open[0]),
                    format!("row {} to take column {}", r + 1, open[0] + 1),
                ));
                break;
            }
        }
        if forced.is_none() {
            for c in 0..n {
                if b.col_cat[c].is_some() {
                    continue;
                }
                let open = b.open_in_col(c, usize::MAX, color);
                if open.len() == 1 {
                    forced = Some((
                        (open[0], c),
                        format!("column {} to take row {}", c + 1, open[0] + 1),
                    ));
                    break;
                }
            }
        }
        if forced.is_none() {
            'regions: for g in 0..b.reg_cat.len() {
                if b.reg_cat[g].is_some() {
                    continue;
                }
                let open: Vec<(usize, usize)> = b
                    .cells_of_region(g as u8)
                    .into_iter()
                    .filter(|&(r, c)| b.blocked(r, c, color).is_none())
                    .collect();
                if open.len() == 1 {
                    let (r, c) = open[0];
                    forced = Some((
                        (r, c),
                        format!(
                            "the {} region to take row {}, column {}",
                            color(g as u8),
                            r + 1,
                            c + 1
                        ),
                    ));
                    break 'regions;
                }
            }
        }

        match forced {
            Some((cell, note)) => {
                cats.push(cell);
                steps.push(note);
            }
            None => return Ok(cats),
        }
    }
    Ok(cats)
}

/// Try to kill every alternative in one unit with full search, preferring an
/// alternative that dies with a chain of forced moves (score 2) over one that
/// only dies under search (score 1). `None` means the unit is not settled.
fn unit_verdict(
    n: usize,
    region: &[u8],
    cats: &[(usize, usize)],
    alts: &[(usize, usize)],
    color: &dyn Fn(u8) -> &'static str,
    label: &dyn Fn((usize, usize)) -> String,
) -> Option<(u8, String)> {
    let mut best: Option<(u8, String)> = None;
    for &cell in alts {
        let mut forced = cats.to_vec();
        forced.push(cell);
        if !solutions(n, region, &forced, &[], 1).is_empty() {
            return None;
        }
        let name = label(cell);
        let (score, text) = match propagate(n, region, &forced, color) {
            Err((steps, fail)) if !steps.is_empty() => {
                let shown: Vec<String> = steps.iter().take(2).cloned().collect();
                (
                    2,
                    format!("{name} would force {}, which leaves {fail}", shown.join(", then ")),
                )
            }
            Err((_, fail)) => (1, format!("{name} leaves {fail}")),
            Ok(_) => (1, format!("{name} leaves no way to finish the board")),
        };
        if best.as_ref().is_none_or(|b| score > b.0) {
            best = Some((score, text));
        }
        if score >= 2 {
            break;
        }
    }
    best
}

/// Argue for one square. Score 3 means a unit is down to its last free square,
/// 2 means the alternatives die with a named chain of forced moves, 1 means
/// they die only under full search, 0 means no argument was found.
fn explain_one(
    n: usize,
    region: &[u8],
    cats: &[(usize, usize)],
    target: (usize, usize),
    color: &dyn Fn(u8) -> &'static str,
) -> (u8, String) {
    let (tr, tc) = target;
    let b = Board::new(n, region, cats);

    // 1. A unit with exactly one open square left.
    if b.open_in_row(tr, tc, color).is_empty() {
        let eg = (0..n).filter(|&c| c != tc).find_map(|c| b.blocked(tr, c, color));
        return match eg {
            Some(e) => (
                3,
                format!(
                    "Row {} is down to one free square — column {}. Everything else there is blocked; for instance {e}.",
                    tr + 1,
                    tc + 1
                ),
            ),
            None => (3, format!("Row {} can take a cat in column {} only.", tr + 1, tc + 1)),
        };
    }
    if b.open_in_col(tc, tr, color).is_empty() {
        let eg = (0..n).filter(|&r| r != tr).find_map(|r| b.blocked(r, tc, color));
        return match eg {
            Some(e) => (
                3,
                format!(
                    "Column {} is down to one free square — row {}. Everything else there is blocked; for instance {e}.",
                    tc + 1,
                    tr + 1
                ),
            ),
            None => (3, format!("Column {} can take a cat in row {} only.", tc + 1, tr + 1)),
        };
    }

    let g = region[tr * n + tc];
    let reg_cells = b.cells_of_region(g);
    if reg_cells.len() == 1 {
        return (
            2,
            format!(
                "The {} region is a single square — row {}, column {} — so the cat for that colour has nowhere else to go.",
                color(g),
                tr + 1,
                tc + 1
            ),
        );
    }
    let reg_open: Vec<(usize, usize)> = reg_cells
        .iter()
        .copied()
        .filter(|&(r, c)| (r, c) != target && b.blocked(r, c, color).is_none())
        .collect();
    if reg_open.is_empty() {
        let eg = reg_cells
            .iter()
            .copied()
            .filter(|&(r, c)| (r, c) != target)
            .find_map(|(r, c)| b.blocked(r, c, color));
        return match eg {
            Some(e) => (
                3,
                format!(
                    "The {} region is down to one free square — row {}, column {}. The rest of it is blocked; for instance {e}.",
                    color(g),
                    tr + 1,
                    tc + 1
                ),
            ),
            None => (
                3,
                format!(
                    "The {} region can take a cat at row {}, column {} only.",
                    color(g),
                    tr + 1,
                    tc + 1
                ),
            ),
        };
    }

    // 2. Every alternative in a unit is provably dead.
    let row_alts: Vec<(usize, usize)> = b
        .open_in_row(tr, tc, color)
        .into_iter()
        .map(|c| (tr, c))
        .collect();
    if let Some((score, detail)) = unit_verdict(n, region, cats, &row_alts, color, &|(_, c)| {
        format!("a cat in column {} instead", c + 1)
    }) {
        return (
            score,
            format!(
                "Row {} needs its cat in column {} — {}.",
                tr + 1,
                tc + 1,
                detail
            ),
        );
    }
    let col_alts: Vec<(usize, usize)> = b
        .open_in_col(tc, tr, color)
        .into_iter()
        .map(|r| (r, tc))
        .collect();
    if let Some((score, detail)) = unit_verdict(n, region, cats, &col_alts, color, &|(r, _)| {
        format!("a cat in row {} instead", r + 1)
    }) {
        return (
            score,
            format!(
                "Column {} needs its cat in row {} — {}.",
                tc + 1,
                tr + 1,
                detail
            ),
        );
    }
    if let Some((score, detail)) = unit_verdict(n, region, cats, &reg_open, color, &|(r, c)| {
        format!("a cat at row {}, column {} instead", r + 1, c + 1)
    }) {
        return (
            score,
            format!(
                "The {} region needs its cat at row {}, column {} — {}.",
                color(g),
                tr + 1,
                tc + 1,
                detail
            ),
        );
    }

    (
        0,
        format!(
            "Row {}, column {} is the only square left that still completes the board.",
            tr + 1,
            tc + 1
        ),
    )
}

/// Explain *why* a cat belongs at `target`, in words a player can check.
///
/// The reasoning is always a deduction a player could make: a unit (row, column
/// or colour) is down to its last free square, or a cat anywhere else forces a
/// chain of moves that leaves some unit with no square left. Player X marks are
/// ignored on purpose — a hinted proof must never lean on a wrong note.
#[cfg(test)]
pub fn explain(
    n: usize,
    region: &[u8],
    cats: &[(usize, usize)],
    target: (usize, usize),
    color: &dyn Fn(u8) -> &'static str,
) -> String {
    explain_one(n, region, cats, target, color).1
}

/// Pick the remaining cat with the clearest argument, so a hint explains
/// something the player can follow instead of announcing an arbitrary square.
pub fn best_hint(
    n: usize,
    region: &[u8],
    cats: &[(usize, usize)],
    solution: &[(usize, usize)],
    color: &dyn Fn(u8) -> &'static str,
) -> Option<((usize, usize), String)> {
    let mut best: Option<(u8, (usize, usize), String)> = None;
    for &cell in solution {
        if cats.contains(&cell) {
            continue;
        }
        let (score, text) = explain_one(n, region, cats, cell, color);
        if best.as_ref().is_none_or(|b| score > b.0) {
            best = Some((score, cell, text));
        }
        if score == 3 {
            break;
        }
    }
    best.map(|(_, cell, text)| (cell, text))
}

// ------------------------------------------------------------- generator ----

/// Backtracking random placement: one per row/column, no two touching.
fn random_placement(n: usize, rng: &mut Rng) -> Option<Vec<(usize, usize)>> {
    fn go(
        n: usize,
        row: usize,
        used_col: u32,
        prev: Option<usize>,
        rng: &mut Rng,
        path: &mut Vec<(usize, usize)>,
    ) -> bool {
        if row == n {
            return true;
        }
        let mut cols: Vec<usize> = (0..n).collect();
        rng.shuffle(&mut cols);
        for c in cols {
            if used_col >> c & 1 == 1 {
                continue;
            }
            if let Some(p) = prev {
                if c.abs_diff(p) < 2 {
                    continue;
                }
            }
            path.push((row, c));
            if go(n, row + 1, used_col | 1 << c, Some(c), rng, path) {
                return true;
            }
            path.pop();
        }
        false
    }
    let mut path = Vec::with_capacity(n);
    if go(n, 0, 0, None, rng, &mut path) {
        Some(path)
    } else {
        None
    }
}

/// Grow `n` connected regions, one seeded at each solution cell.
/// Cells are handed out to the currently smallest region, which keeps the
/// regions balanced instead of letting one colour swallow the board.
fn grow_regions(n: usize, sol: &[(usize, usize)], rng: &mut Rng) -> Vec<u8> {
    let mut region = vec![u8::MAX; n * n];
    let mut sizes = vec![0usize; n];
    let mut frontier: Vec<Vec<usize>> = vec![Vec::new(); n];

    for (i, &(r, c)) in sol.iter().enumerate() {
        region[r * n + c] = i as u8;
        sizes[i] = 1;
    }
    let mut scratch = Vec::with_capacity(4);
    for (i, &(r, c)) in sol.iter().enumerate() {
        neighbours(n, r, c, &mut scratch);
        for &nb in &scratch {
            if region[nb] == u8::MAX {
                frontier[i].push(nb);
            }
        }
    }

    let mut remaining = n * n - n;
    let mut active: Vec<usize> = Vec::new();
    while remaining > 0 {
        active.clear();
        active.extend((0..n).filter(|&i| !frontier[i].is_empty()));
        if active.is_empty() {
            break;
        }
        // weight by inverse size (with a little noise so shapes stay irregular)
        let mut total = 0.0;
        let mut weights = Vec::with_capacity(active.len());
        for &i in &active {
            let w = 1.0 / (sizes[i] as f64) * (0.55 + 0.9 * rng.unit());
            weights.push(w);
            total += w;
        }
        let mut pick = rng.unit() * total;
        let mut chosen = active[active.len() - 1];
        for (k, &i) in active.iter().enumerate() {
            pick -= weights[k];
            if pick <= 0.0 {
                chosen = i;
                break;
            }
        }
        let at = rng.below(frontier[chosen].len());
        let cell = frontier[chosen].swap_remove(at);
        if region[cell] != u8::MAX {
            continue;
        }
        region[cell] = chosen as u8;
        sizes[chosen] += 1;
        remaining -= 1;
        neighbours(n, cell / n, cell % n, &mut scratch);
        for &nb in &scratch {
            if region[nb] == u8::MAX {
                frontier[chosen].push(nb);
            }
        }
    }

    region
}

fn region_connected(n: usize, region: &[u8]) -> bool {
    let mut seen = vec![false; n * n];
    let mut stack = Vec::new();
    let mut scratch = Vec::with_capacity(4);
    for g in 0..n {
        let start = match region.iter().position(|&x| x as usize == g) {
            Some(s) => s,
            None => return false,
        };
        let mut visited = 0usize;
        seen.iter_mut().for_each(|x| *x = false);
        stack.clear();
        stack.push(start);
        while let Some(cell) = stack.pop() {
            if seen[cell] {
                continue;
            }
            seen[cell] = true;
            visited += 1;
            neighbours(n, cell / n, cell % n, &mut scratch);
            for &nb in &scratch {
                if region[nb] as usize == g && !seen[nb] {
                    stack.push(nb);
                }
            }
        }
        let cells_in_region = region.iter().filter(|&&x| x as usize == g).count();
        if visited != cells_in_region {
            return false;
        }
    }
    true
}

pub struct GenReport {
    pub puzzle: Puzzle,
    pub attempts: usize,
    pub elapsed: Duration,
}

/// True when every cell carrying region id `g` is reachable from every other
/// one — Star Battle regions must be orthogonally contiguous.
fn region_connected_id(n: usize, region: &[u8], g: u8) -> bool {
    let start = match region.iter().position(|&x| x == g) {
        Some(s) => s,
        None => return false,
    };
    let total = region.iter().filter(|&&x| x == g).count();
    let mut seen = vec![false; n * n];
    let mut stack = vec![start];
    let mut visited = 0usize;
    let mut scratch = Vec::with_capacity(4);
    while let Some(cell) = stack.pop() {
        if seen[cell] {
            continue;
        }
        seen[cell] = true;
        visited += 1;
        neighbours(n, cell / n, cell % n, &mut scratch);
        for &nb in &scratch {
            if region[nb] == g && !seen[nb] {
                stack.push(nb);
            }
        }
    }
    visited == total
}

/// Generate a puzzle with exactly one solution.
///
/// Growing regions around a known placement almost never yields a unique board
/// (empirically: every one of 2000 blobs admitted 9+ solutions). So instead the
/// region borders are nudged until the solver can only see the placement we
/// started from: pick a cell that an *alternate* solution uses, hand that cell
/// to a neighbouring region, and keep the move when it narrows the solution
/// set. The planted placement's own cells are never moved, so it always stays
/// legal and the count can never fall below one.
pub fn generate(n: usize, seed: u64) -> Option<GenReport> {
    let started = Instant::now();
    let mut rng = Rng::new(seed);
    let budget = Duration::from_secs(10);
    for attempts in 1..=256usize {
        let sol = match random_placement(n, &mut rng) {
            Some(s) => s,
            None => continue,
        };
        let mut region = grow_regions(n, &sol, &mut rng);
        if region.contains(&u8::MAX) || !region_connected(n, &region) {
            continue;
        }
        let mut sols = solutions(n, &region, &[], &[], 4);
        let mut best = sols.len();
        let mut stall = 0usize;
        while best > 1 && stall < 25_000 {
            stall += 1;
            if started.elapsed() > budget {
                return None;
            }
            let mut idx = rng.below(n * n);
            let mut g = region[idx];
            if sols.len() > 1 && rng.unit() < 0.65 {
                let alt = &sols[1 + rng.below(sols.len() - 1)];
                let pick = alt[rng.below(alt.len())];
                let cand = pick.0 * n + pick.1;
                if sol.iter().all(|&(r, c)| r * n + c != cand) {
                    idx = cand;
                    g = region[idx];
                }
            }
            if sol.iter().any(|&(r, c)| r * n + c == idx) {
                continue;
            }
            let mut scratch = Vec::with_capacity(4);
            neighbours(n, idx / n, idx % n, &mut scratch);
            if scratch.is_empty() {
                continue;
            }
            let h = region[scratch[rng.below(scratch.len())]];
            if h == g {
                continue;
            }
            region[idx] = h;
            if !region_connected_id(n, &region, g) {
                region[idx] = g;
                continue;
            }
            let k = solutions(n, &region, &[], &[], 4);
            if k.len() < best {
                best = k.len();
                sols = k;
                stall = 0;
            } else if k.len() == best && rng.unit() < 0.4 {
                sols = k;
            } else {
                region[idx] = g;
            }
        }
        if best == 1 {
            return Some(GenReport {
                puzzle: Puzzle { n, region, solution: sols[0].clone() },
                attempts,
                elapsed: started.elapsed(),
            });
        }
    }
    None
}

// ---------------------------------------------------------------- tests ----

#[cfg(test)]
mod tests {
    use super::*;

    fn rules_hold(n: usize, region: &[u8], sol: &[(usize, usize)]) -> bool {
        if sol.len() != n {
            return false;
        }
        let mut rows = vec![false; n];
        let mut cols = vec![false; n];
        let mut regs = vec![false; n];
        for (i, &(r, c)) in sol.iter().enumerate() {
            if rows[r] || cols[c] || regs[region[r * n + c] as usize] {
                return false;
            }
            rows[r] = true;
            cols[c] = true;
            regs[region[r * n + c] as usize] = true;
            for &(r2, c2) in &sol[i + 1..] {
                let dr = (r as i32 - r2 as i32).abs();
                let dc = (c as i32 - c2 as i32).abs();
                if dr <= 1 && dc <= 1 {
                    return false;
                }
            }
        }
        true
    }

    #[test]
    fn generates_unique_valid_board() {
        let rep = generate(10, 42).expect("generator failed");
        let p = &rep.puzzle;
        assert_eq!(p.solution.len(), 10);
        assert!(rules_hold(p.n, &p.region, &p.solution));
        assert!(region_connected(p.n, &p.region));
        assert_eq!(solutions(p.n, &p.region, &[], &[], 5).len(), 1);
    }

    #[test]
    fn hint_matches_unique_solution() {
        let p = generate(10, 7).unwrap().puzzle;
        // pin two true cats, the completion must still be the same solution
        let cats = vec![p.solution[0], p.solution[3]];
        let sols = solutions(p.n, &p.region, &cats, &[], 3);
        assert_eq!(sols.len(), 1);
        let mut a = sols[0].clone();
        let mut b = p.solution.clone();
        a.sort();
        b.sort();
        assert_eq!(a, b);
    }

    #[test]
    fn wrong_cat_kills_all_solutions() {
        let p = generate(10, 11).unwrap().puzzle;
        let mut bad = None;
        'outer: for r in 0..p.n {
            for c in 0..p.n {
                if !p.solution.contains(&(r, c)) {
                    bad = Some((r, c));
                    break 'outer;
                }
            }
        }
        let bad = bad.unwrap();
        assert_eq!(solutions(p.n, &p.region, &[bad], &[], 1).len(), 0);
    }

    #[test]
    fn generation_speed() {
        let t = std::time::Instant::now();
        for seed in 0..5u64 {
            let rep = generate(10, seed + 100).expect("gen");
            assert_eq!(rep.puzzle.solution.len(), 10);
        }
        println!("5 puzzles in {:?}", t.elapsed());
    }

    fn palette(g: u8) -> &'static str {
        ["pink", "apricot", "butter", "lime", "mint", "teal", "sky", "lilac"]
            [g as usize % 8]
    }

    /// A hint must argue from the board, name the square, and never be terse.
    #[test]
    fn hints_explain_themselves() {
        for seed in [1u64, 7, 99] {
            let p = generate(10, seed).expect("puzzle").puzzle;
            for &(r, c) in &p.solution {
                let e = explain(p.n, &p.region, &[], (r, c), &palette);
                let l = e.to_lowercase();
                assert!(e.len() > 40, "too terse: {e}");
                assert!(l.contains(&format!("row {}", r + 1)), "row missing: {e}");
                assert!(l.contains(&format!("column {}", c + 1)), "col missing: {e}");
            }
        }
    }

    /// With nine cats down, the last one is forced by its row — the hint should
    /// say so in those terms rather than announce the answer.
    #[test]
    fn last_cat_hint_counts_the_free_square() {
        let p = generate(10, 4242).expect("puzzle").puzzle;
        let mut placed = p.solution.clone();
        let last = placed.pop().unwrap();
        let e = explain(p.n, &p.region, &placed, last, &palette);
        let l = e.to_lowercase();
        assert!(
            l.contains("down to one free square") || l.contains("dead end"),
            "no deduction: {e}"
        );
    }
}
