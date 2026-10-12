//! The fold (docs/design.md, 「折りたたみ」): no scrollbar anywhere. A list
//! that does not fit its box folds the rows farthest from the selected one to
//! one line first; if that is not enough it hides the farthest ones and says
//! how many are hidden above and below. This is the mock's `fold()`.

/// What to do with one row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Show {
    /// At its full height.
    Full,
    /// At the folded height (a card's title only, a list row's first line).
    Folded,
    /// Not drawn; counted in [`Fold::above`] or [`Fold::below`].
    Hidden,
}

/// The result: one [`Show`] per row, and the "⋯ 上に N 件" / "⋯ 下に N 件" counts.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Fold {
    pub rows: Vec<Show>,
    /// Hidden rows before the selected one.
    pub above: usize,
    /// Hidden rows after the selected one.
    pub below: usize,
}

/// Fold `heights` (each row's full height) into `box_h`. `folded` is a
/// folded row's height, `more` the height of the "⋯ N 件" line (shown above
/// and/or below only when rows are hidden on that side), and `selected` the
/// row to keep whole. The farthest row goes first; at equal distance, the
/// later one.
pub fn fold(heights: &[f32], folded: f32, more: f32, box_h: f32, selected: usize) -> Fold {
    let n = heights.len();
    let mut rows = vec![Show::Full; n];
    let mut out = Fold { rows: Vec::new(), above: 0, below: 0 };
    if n == 0 {
        return out;
    }
    let on = selected.min(n - 1);
    let mut far: Vec<usize> = (0..n).filter(|&k| k != on).collect();
    far.sort_by(|&a, &b| on.abs_diff(b).cmp(&on.abs_diff(a)).then(b.cmp(&a)));
    let mut total: f32 = heights.iter().sum();
    // A row never grows by being folded.
    for &k in &far {
        if total <= box_h {
            break;
        }
        total -= (heights[k] - folded).max(0.0);
        rows[k] = Show::Folded;
    }
    let (mut above, mut below) = (0, 0);
    let height = |total: f32, above: usize, below: usize| {
        total + if above > 0 { more } else { 0.0 } + if below > 0 { more } else { 0.0 }
    };
    for &k in &far {
        if height(total, above, below) <= box_h {
            break;
        }
        total -= heights[k].min(folded);
        rows[k] = Show::Hidden;
        if k < on {
            above += 1;
        } else {
            below += 1;
        }
    }
    out.rows = rows;
    out.above = above;
    out.below = below;
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(f: &Fold) -> String {
        f.rows
            .iter()
            .map(|s| match s {
                Show::Full => 'F',
                Show::Folded => 'm',
                Show::Hidden => '.',
            })
            .collect()
    }

    #[test]
    fn everything_fits() {
        let f = fold(&[40.0; 3], 10.0, 8.0, 120.0, 1);
        assert_eq!(shown(&f), "FFF");
        assert_eq!((f.above, f.below), (0, 0));
    }

    #[test]
    fn the_farthest_fold_first() {
        // 5 x 40 = 200 into 150: folding one saves 30, folding two 60.
        let f = fold(&[40.0; 5], 10.0, 8.0, 150.0, 0);
        assert_eq!(shown(&f), "FFFmm");
        // Equal distance: the later one goes first.
        let f = fold(&[40.0; 5], 10.0, 8.0, 170.0, 2);
        assert_eq!(shown(&f), "FFFFm");
    }

    #[test]
    fn the_selected_row_stays_whole() {
        let f = fold(&[40.0; 6], 10.0, 8.0, 60.0, 3);
        assert_eq!(f.rows[3], Show::Full);
    }

    #[test]
    fn hides_when_folding_is_not_enough() {
        // 6 x 40 folded to 10 = 40 + 5 x 10 = 90; the box is 70 with two
        // "more" lines of 8: hide until total + lines fits.
        let f = fold(&[40.0; 6], 10.0, 8.0, 70.0, 3);
        assert_eq!(f.rows[3], Show::Full);
        assert!(f.above + f.below > 0);
        let hidden = f.rows.iter().filter(|s| **s == Show::Hidden).count();
        assert_eq!(hidden, f.above + f.below);
        let total: f32 = f
            .rows
            .iter()
            .zip([40.0; 6])
            .map(|(s, h)| match s {
                Show::Full => h,
                Show::Folded => 10.0,
                Show::Hidden => 0.0,
            })
            .sum();
        let lines = (f.above > 0) as u8 as f32 * 8.0 + (f.below > 0) as u8 as f32 * 8.0;
        assert!(total + lines <= 70.0, "{total} + {lines}");
    }

    #[test]
    fn counts_split_above_and_below() {
        // Selected at the top: only rows below can be hidden.
        let f = fold(&[40.0; 8], 10.0, 8.0, 80.0, 0);
        assert_eq!(f.above, 0);
        assert!(f.below > 0);
        // Selected at the bottom: the mirror.
        let f = fold(&[40.0; 8], 10.0, 8.0, 80.0, 7);
        assert_eq!(f.below, 0);
        assert!(f.above > 0);
    }

    #[test]
    fn a_selection_in_the_middle_hides_both_ends() {
        let f = fold(&[40.0; 9], 10.0, 8.0, 100.0, 4);
        assert!(f.above > 0 && f.below > 0);
        assert_eq!(f.rows[4], Show::Full);
        // The nearest rows survive longest.
        assert_ne!(f.rows[3], Show::Hidden);
        assert_ne!(f.rows[5], Show::Hidden);
    }

    #[test]
    fn empty_and_out_of_range() {
        assert_eq!(fold(&[], 10.0, 8.0, 100.0, 0).rows.len(), 0);
        let f = fold(&[40.0; 3], 10.0, 8.0, 1000.0, 99);
        assert_eq!(shown(&f), "FFF");
    }

    #[test]
    fn a_row_shorter_than_the_fold_does_not_grow() {
        let f = fold(&[5.0, 40.0, 40.0], 10.0, 8.0, 60.0, 2);
        assert_eq!(f.rows[2], Show::Full);
        // Folding the 5-high row would gain nothing, but it may still be marked.
        assert!(f.rows[0] != Show::Full || f.rows[1] != Show::Full);
    }
}
