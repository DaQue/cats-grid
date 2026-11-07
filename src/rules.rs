//! One-rule-at-a-time eliminations.
//!
//! Each `Deduction` is the X marks implied by a *single* named rule given the
//! cats already on the board — nothing chained, nothing speculative. The UI
//! offers them one button at a time so the player can see the reasoning.

/// A single rule application: cross out `cells` because of `why`.
pub struct Deduction {
    /// short button label, e.g. "Row 4" or "Rose"
    pub title: String,
    /// the full sentence explaining the rule and what it implies
    pub why: String,
    /// cells (row, col) that should be crossed out
    pub cells: Vec<(usize, usize)>,
}

/// All deductions currently available, biggest win first.
///
/// `empty` reports whether a cell still carries no mark; only those cells are
/// worth crossing out.
pub fn deductions(
    n: usize,
    region: &[u8],
    cats: &[(usize, usize)],
    color_name: &dyn Fn(u8) -> &'static str,
    empty: &dyn Fn(usize, usize) -> bool,
) -> Vec<Deduction> {
    let mut out: Vec<Deduction> = Vec::new();

    // A row that already holds its cat cannot hold another.
    for r in 0..n {
        let in_row: Vec<&(usize, usize)> = cats.iter().filter(|(cr, _)| *cr == r).collect();
        if in_row.len() != 1 {
            continue;
        }
        let c0 = in_row[0].1;
        let cells: Vec<(usize, usize)> = (0..n).filter(|&c| c != c0 && empty(r, c)).map(|c| (r, c)).collect();
        if cells.is_empty() {
            continue;
        }
        out.push(Deduction {
            title: format!("Row {}", r + 1),
            why: format!(
                "Row {} already has its cat in column {}, so the other {} cells in that row must be empty.",
                r + 1,
                c0 + 1,
                cells.len()
            ),
            cells,
        });
    }

    // Same argument down a column.
    for c in 0..n {
        let in_col: Vec<&(usize, usize)> = cats.iter().filter(|(_, cc)| *cc == c).collect();
        if in_col.len() != 1 {
            continue;
        }
        let r0 = in_col[0].0;
        let cells: Vec<(usize, usize)> = (0..n).filter(|&r| r != r0 && empty(r, c)).map(|r| (r, c)).collect();
        if cells.is_empty() {
            continue;
        }
        out.push(Deduction {
            title: format!("Column {}", c + 1),
            why: format!(
                "Column {} already has its cat in row {}, so the other {} cells in that column must be empty.",
                c + 1,
                r0 + 1,
                cells.len()
            ),
            cells,
        });
    }

    // Each colour holds exactly one cat, so a solved colour empties the rest.
    for g in 0..n as u8 {
        let members: Vec<(usize, usize)> = (0..n * n)
            .filter(|i| region[*i] == g)
            .map(|i| (i / n, i % n))
            .collect();
        let inside: Vec<&(usize, usize)> = cats.iter().filter(|(r, c)| region[r * n + c] == g).collect();
        if inside.len() != 1 {
            continue;
        }
        let cells: Vec<(usize, usize)> = members
            .into_iter()
            .filter(|&(r, c)| !(r == inside[0].0 && c == inside[0].1) && empty(r, c))
            .collect();
        if cells.is_empty() {
            continue;
        }
        let name = color_name(g);
        out.push(Deduction {
            title: name.to_string(),
            why: format!(
                "The {} region already has its cat at row {}, column {}, so its other {} cells must be empty.",
                name.to_lowercase(),
                inside[0].0 + 1,
                inside[0].1 + 1,
                cells.len()
            ),
            cells,
        });
    }

    // Cats never touch, not even diagonally.
    for &(r, c) in cats {
        let mut cells = Vec::new();
        for dr in -1i32..=1 {
            for dc in -1i32..=1 {
                if dr == 0 && dc == 0 {
                    continue;
                }
                let rr = r as i32 + dr;
                let cc = c as i32 + dc;
                if rr < 0 || cc < 0 || rr >= n as i32 || cc >= n as i32 {
                    continue;
                }
                let (rr, cc) = (rr as usize, cc as usize);
                if empty(rr, cc) {
                    cells.push((rr, cc));
                }
            }
        }
        if cells.is_empty() {
            continue;
        }
        out.push(Deduction {
            title: format!("Touch {}.{}", r + 1, c + 1),
            why: format!(
                "No two cats may touch, even diagonally, so the {} cells around the cat at row {}, column {} must be empty.",
                cells.len(),
                r + 1,
                c + 1
            ),
            cells,
        });
    }

    out.sort_by_key(|d| std::cmp::Reverse(d.cells.len()));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn names(g: u8) -> &'static str {
        ["rose", "mint", "teal", "plum"][g as usize % 4]
    }

    /// 4x4 board, one cat at (0,0); every other cell unmarked.
    fn base() -> (usize, Vec<u8>, Vec<(usize, usize)>) {
        let n = 4;
        let region: Vec<u8> = (0..n * n).map(|i| (i / n) as u8).collect();
        (n, region, vec![(0, 0)])
    }

    #[test]
    fn row_and_column_rules_cross_out_their_units() {
        let (n, region, cats) = base();
        let d = deductions(n, &region, &cats, &names, &|_, _| true);
        let row = d.iter().find(|x| x.title == "Row 1").expect("row rule");
        assert_eq!(row.cells.len(), 3);
        assert!(row.cells.iter().all(|&(r, _)| r == 0));
        assert!(!row.cells.contains(&(0, 0)));

        let col = d.iter().find(|x| x.title == "Column 1").expect("column rule");
        assert_eq!(col.cells.len(), 3);
        assert!(col.cells.iter().all(|&(_, c)| c == 0));
    }

    #[test]
    fn colour_rule_empties_the_rest_of_the_region() {
        let (n, region, cats) = base();
        let d = deductions(n, &region, &cats, &names, &|_, _| true);
        let colour = d.iter().find(|x| x.title == "rose").expect("colour rule");
        assert_eq!(colour.cells.len(), 3);
        assert!(colour.cells.iter().all(|&(r, _)| r == 0));
        assert!(colour.why.contains("rose"));
    }

    #[test]
    fn touch_rule_covers_diagonals() {
        let (n, region, cats) = base();
        let d = deductions(n, &region, &cats, &names, &|_, _| true);
        let touch = d.iter().find(|x| x.title.starts_with("Touch")).expect("touch rule");
        // corner cat at (0,0) has three neighbours
        assert_eq!(touch.cells.len(), 3);
    }

    #[test]
    fn already_marked_cells_are_skipped() {
        let (n, region, cats) = base();
        let d = deductions(n, &region, &cats, &names, &|r, c| !(r == 0 && c == 1));
        let row = d.iter().find(|x| x.title == "Row 1").expect("row rule");
        assert_eq!(row.cells.len(), 2);
        assert!(!row.cells.contains(&(0, 1)));
    }

    #[test]
    fn nothing_available_without_a_cat() {
        let (n, region, _) = base();
        let d = deductions(n, &region, &[], &names, &|_, _| true);
        assert!(d.is_empty());
    }

    #[test]
    fn biggest_deduction_first() {
        let n = 4;
        let region: Vec<u8> = (0..n * n).map(|i| (i / n) as u8).collect();
        let d = deductions(n, &region, &[(1, 1)], &names, &|_, _| true);
        let sizes: Vec<usize> = d.iter().map(|x| x.cells.len()).collect();
        assert!(sizes.windows(2).all(|w| w[0] >= w[1]));
    }

    #[test]
    fn never_lists_a_placed_cat() {
        let n = 4;
        let region: Vec<u8> = (0..n * n).map(|i| (i / n) as u8).collect();
        let cats = [(0, 0), (2, 1)];
        // a cell is only "empty" when no cat sits on it
        let d = deductions(n, &region, &cats, &names, &|r, c| !cats.contains(&(r, c)));
        assert!(!d.is_empty());
        assert!(d.iter().all(|x| x.cells.iter().all(|cell| !cats.contains(cell))));
        assert!(d.iter().all(|x| x.cells.len() <= n * n));
    }
}
