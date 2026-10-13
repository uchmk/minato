//! The fold (docs/design.md, 「折りたたみ」): no scrollbar anywhere. A list
//! that does not fit its box shows as many whole rows as fit, from the
//! selected one outward, hides the rest and says how many are hidden above
//! and below. No row is drawn shorter than it is.

/// What to do with one row.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Show {
    /// At its full height.
    Full,
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

/// Fit `heights` (each row's height) into `box_h`: the `selected` row
/// always, then the rows nearest it (the earlier first at equal distance)
/// while they fit, each "⋯ N 件" line of height `more` counted (one above
/// and/or below, only when rows are hidden on that side). It stops at the
/// first row that does not fit, so what is shown stays in one piece.
pub fn fold(heights: &[f32], more: f32, box_h: f32, selected: usize) -> Fold {
    let n = heights.len();
    if n == 0 {
        return Fold { rows: Vec::new(), above: 0, below: 0 };
    }
    let on = selected.min(n - 1);
    let mut near: Vec<usize> = (0..n).collect();
    near.sort_by(|&a, &b| on.abs_diff(a).cmp(&on.abs_diff(b)).then(a.cmp(&b)));
    let mut rows = vec![Show::Hidden; n];
    rows[on] = Show::Full;
    let (mut above, mut below) = (on, n - 1 - on);
    let mut total = heights[on];
    let fits =
        |total: f32, above: usize, below: usize| total + if above > 0 { more } else { 0.0 } + if below > 0 { more } else { 0.0 } <= box_h;
    for &at in &near[1..] {
        let (a, b) = if at < on { (above - 1, below) } else { (above, below - 1) };
        if !fits(total + heights[at], a, b) {
            break;
        }
        rows[at] = Show::Full;
        total += heights[at];
        (above, below) = (a, b);
    }
    Fold { rows, above, below }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shown(f: &Fold) -> String {
        f.rows
            .iter()
            .map(|s| match s {
                Show::Full => 'F',
                Show::Hidden => '.',
            })
            .collect()
    }

    #[test]
    fn everything_fits() {
        let f = fold(&[40.0; 3], 8.0, 120.0, 1);
        assert_eq!(shown(&f), "FFF");
        assert_eq!((f.above, f.below), (0, 0));
    }

    #[test]
    fn as_many_whole_rows_as_fit() {
        // 30 rows of 40 into 300 from the top: 7 whole (280 + 8 for the
        // line below), the rest counted below.
        let f = fold(&[40.0; 30], 8.0, 300.0, 0);
        assert_eq!(&shown(&f)[..8], "FFFFFFF.");
        assert_eq!((f.above, f.below), (0, 23));
        // A tall box shows them all: no cap.
        assert_eq!(shown(&fold(&[40.0; 30], 8.0, 1200.0, 0)), "F".repeat(30));
    }

    #[test]
    fn the_selected_row_stays_whole_even_alone() {
        let f = fold(&[40.0; 6], 8.0, 20.0, 3);
        assert_eq!(shown(&f), "...F..");
        assert_eq!((f.above, f.below), (3, 2));
    }

    #[test]
    fn the_shown_rows_center_on_the_selection_in_one_piece() {
        // 9 rows of 40 into 220 with two lines of 8: five whole, 2 each side
        // of the fifth.
        let f = fold(&[40.0; 9], 8.0, 220.0, 4);
        assert_eq!(shown(&f), "..FFFFF..");
        assert_eq!((f.above, f.below), (2, 2));
        // At equal distance the earlier row comes first.
        let f = fold(&[40.0; 9], 8.0, 180.0, 4);
        assert_eq!(shown(&f), "..FFFF...");
        let f = fold(&[40.0; 20], 8.0, 200.0, 10);
        let s = shown(&f);
        let first = s.find('F').unwrap();
        let last = s.rfind('F').unwrap();
        assert!(!s[first..=last].contains('.'), "{s}");
        assert_eq!((f.above, f.below), (first, 19 - last));
    }

    #[test]
    fn rows_of_different_heights_and_the_ends() {
        // Selected at the bottom: only rows above can be hidden.
        let f = fold(&[76.0, 54.0, 76.0, 54.0, 76.0], 8.0, 200.0, 4);
        assert_eq!(shown(&f), "...FF");
        assert_eq!((f.above, f.below), (3, 0));
        assert!(fold(&[], 8.0, 100.0, 0).rows.is_empty());
        assert_eq!(shown(&fold(&[40.0; 3], 8.0, 1000.0, 99)), "FFF");
    }
}
