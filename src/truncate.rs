// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

//! Pure layout logic for truncating large results to fit the screen, modeled on
//! DuckDB's box renderer:
//!
//! - Rows beyond `max_rows` are hidden from the middle of the result, keeping
//!   the first and last rows visible.
//! - Columns wider than the terminal are first shortened (never below
//!   [`MIN_COL_WIDTH_BEFORE_PRUNING`]), and if that is not enough, columns
//!   are hidden starting from the middle and replaced with a single `…`
//!   column.

use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

/// Default number of rows to display in a terminal.
pub const DEFAULT_MAX_ROWS: usize = 40;

/// Columns are not shortened below this width before columns are hidden.
pub const MIN_COL_WIDTH_BEFORE_PRUNING: usize = 20;

/// Marker used for truncated values and hidden columns.
pub const ELLIPSIS: &str = "…";
const ELLIPSIS_WIDTH: usize = 1;

/// Marker used in the divider rows that stand in for hidden rows.
pub const DOT: &str = "·";

/// Number of divider rows rendered in place of hidden rows.
pub const DIVIDER_ROWS: usize = 3;

/// Return how many rows to render from the top and the bottom of a result with
/// `total` rows. When nothing is hidden the bottom count is zero.
pub fn split_rows(total: usize, max_rows: Option<usize>) -> (usize, usize) {
    match max_rows {
        // Hiding rows adds divider rows, so it only makes sense when it
        // actually reduces the number of lines displayed.
        Some(max) if total > max + DIVIDER_ROWS => {
            let top = max.div_ceil(2);
            (top, max - top)
        }
        _ => (total, 0),
    }
}

/// The display width of a (possibly multi-line) value: its widest line.
pub fn display_width(value: &str) -> usize {
    value.lines().map(UnicodeWidthStr::width).max().unwrap_or(0)
}

/// Truncate each line of `value` to at most `width` display columns, marking
/// truncated lines with [`ELLIPSIS`].
pub fn truncate(value: &str, width: usize) -> String {
    if display_width(value) <= width {
        return value.to_string();
    }
    value
        .split('\n')
        .map(|line| truncate_line(line, width))
        .collect::<Vec<_>>()
        .join("\n")
}

fn truncate_line(line: &str, width: usize) -> String {
    if line.width() <= width {
        return line.to_string();
    }
    if width < ELLIPSIS_WIDTH {
        return String::new();
    }
    let budget = width - ELLIPSIS_WIDTH;
    let mut used = 0;
    let mut out = String::new();
    for ch in line.chars() {
        let w = ch.width().unwrap_or(0);
        if used + w > budget {
            break;
        }
        used += w;
        out.push(ch);
    }
    out.push_str(ELLIPSIS);
    out
}

/// The width a table style adds around column contents: borders, separators
/// and padding. A table with `n >= 1` columns of content widths `w` renders as
/// `first_column + (n - 1) * per_column + sum(w)` columns wide.
///
/// The first column is counted separately because styles without outer
/// borders have one fewer separator than columns, which a single fixed term
/// could only express as a negative value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Overhead {
    /// Overhead of a single-column table: outer borders plus padding.
    pub first_column: usize,
    /// Overhead of each additional column: separator plus padding.
    pub per_column: usize,
}

impl Overhead {
    pub fn total(&self, columns: usize) -> usize {
        match columns {
            0 => 0,
            n => self.first_column + self.per_column * (n - 1),
        }
    }
}

/// A column in the rendered layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutColumn {
    /// A column of the result, rendered with the given content width.
    Data { index: usize, width: usize },
    /// The `…` column standing in for hidden columns.
    Ellipsis,
}

/// Decide which columns to show and how wide each should be so that the table
/// fits within `max_width`. `natural` holds the untruncated content width of
/// each column.
pub fn layout_columns(
    natural: &[usize],
    max_width: Option<usize>,
    overhead: Overhead,
) -> Vec<LayoutColumn> {
    let n = natural.len();
    let Some(max_width) = max_width else {
        return build_layout(natural, &vec![false; n]);
    };
    let budget = |columns: usize| max_width.saturating_sub(overhead.total(columns));

    // Shorten the widest columns, but not below MIN_COL_WIDTH_BEFORE_PRUNING.
    if let Some(widths) = fit_widths(natural, budget(n), MIN_COL_WIDTH_BEFORE_PRUNING) {
        return build_layout(&widths, &vec![false; n]);
    }
    if n == 1 {
        let widths = fit_widths(natural, budget(1), 1).unwrap_or(vec![1]);
        return build_layout(&widths, &[false]);
    }

    prune_columns(natural, max_width, overhead)
}

/// Column indices in the order they are hidden: starting from the middle and
/// alternating outwards (left first), e.g. `2, 1, 3, 0, 4` for five columns.
fn middle_out(n: usize) -> impl Iterator<Item = usize> {
    let mid = n / 2;
    (0..n).map(move |i| {
        if i % 2 == 0 {
            mid + i / 2
        } else {
            mid - i.div_ceil(2)
        }
    })
}

/// Cap every column at [`MIN_COL_WIDTH_BEFORE_PRUNING`] and hide columns from the middle
/// until the rest fit alongside the `…` column.
fn prune_columns(natural: &[usize], max_width: usize, overhead: Overhead) -> Vec<LayoutColumn> {
    let n = natural.len();
    let mut widths: Vec<usize> = natural
        .iter()
        .map(|&w| w.min(MIN_COL_WIDTH_BEFORE_PRUNING))
        .collect();
    let mut pruned = vec![false; n];
    let mut visible = n;
    let mut visible_sum: usize = widths.iter().sum();
    // The `…` column takes up one more column of overhead plus its content.
    let data_budget = |visible: usize| {
        max_width
            .saturating_sub(overhead.total(visible + 1))
            .saturating_sub(ELLIPSIS_WIDTH)
    };
    let mut order = middle_out(n);
    while visible > 1 && (visible == n || visible_sum > data_budget(visible)) {
        let Some(c) = order.next() else { break };
        pruned[c] = true;
        visible -= 1;
        visible_sum -= widths[c];
    }

    let data_budget = data_budget(visible);
    let visible_columns = || (0..n).filter(|&c| !pruned[c]);
    if visible_sum <= data_budget {
        // Hand any leftover space back to the visible columns, left to right.
        let mut spare = data_budget - visible_sum;
        for c in visible_columns() {
            let grow = spare.min(natural[c] - widths[c]);
            widths[c] += grow;
            spare -= grow;
        }
    } else {
        // Even a single column does not fit: shrink it as far as needed.
        let visible_widths: Vec<usize> = visible_columns().map(|c| widths[c]).collect();
        let fitted = fit_widths(&visible_widths, data_budget, 1)
            .unwrap_or_else(|| vec![1; visible_widths.len()]);
        for (c, w) in visible_columns().zip(fitted) {
            widths[c] = w;
        }
    }

    build_layout(&widths, &pruned)
}

fn build_layout(widths: &[usize], pruned: &[bool]) -> Vec<LayoutColumn> {
    let mut layout = Vec::with_capacity(widths.len());
    let mut added_ellipsis = false;
    for (index, (&width, &is_pruned)) in widths.iter().zip(pruned).enumerate() {
        if !is_pruned {
            layout.push(LayoutColumn::Data { index, width });
        } else if !added_ellipsis {
            layout.push(LayoutColumn::Ellipsis);
            added_ellipsis = true;
        }
    }
    layout
}

/// Shrink the widest columns first ("water-filling") until the widths sum to at
/// most `budget`, never shrinking a column below `floor`. Columns already
/// narrower than `floor` are left alone. Returns `None` if the budget cannot be
/// met.
fn fit_widths(widths: &[usize], budget: usize, floor: usize) -> Option<Vec<usize>> {
    let capped_sum = |cap: usize| widths.iter().map(|&w| w.min(cap)).sum::<usize>();
    if widths.iter().sum::<usize>() <= budget {
        return Some(widths.to_vec());
    }
    if capped_sum(floor) > budget {
        return None;
    }
    // Find the largest cap that fits.
    let (mut lo, mut hi) = (floor, widths.iter().copied().max().unwrap_or(floor));
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        if capped_sum(mid) <= budget {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let mut fitted: Vec<usize> = widths.iter().map(|&w| w.min(lo)).collect();
    // Distribute the remainder to the columns that were capped.
    let mut spare = budget - capped_sum(lo);
    for (f, &w) in fitted.iter_mut().zip(widths) {
        if spare == 0 {
            break;
        }
        if w > *f {
            *f += 1;
            spare -= 1;
        }
    }
    Some(fitted)
}

/// Build the footer line summarizing hidden rows and columns, or `None` if
/// nothing was hidden. The footer is laid out to span `table_width` columns.
/// The optional `hint` (e.g. how to show the entire result) is centered
/// between the row and column counts when there is room for it.
pub fn footer(
    total_rows: usize,
    shown_rows: usize,
    total_columns: usize,
    shown_columns: usize,
    table_width: usize,
    hint: Option<&str>,
) -> Option<String> {
    if shown_rows >= total_rows && shown_columns >= total_columns {
        return None;
    }
    let mut left = plural(total_rows, "row");
    if shown_rows < total_rows {
        left.push_str(&format!(" ({shown_rows} shown)"));
    }
    let mut right = plural(total_columns, "column");
    if shown_columns < total_columns {
        right.push_str(&format!(" ({shown_columns} shown)"));
    }
    let used = 2 + left.width() + right.width() + 2;
    let padding = table_width.saturating_sub(used);
    if let Some(hint) = hint {
        // Like DuckDB, only show the hint if it leaves some breathing room.
        if padding >= hint.width() + 10 {
            let spare = padding - hint.width();
            let lpad = spare / 2;
            let rpad = spare - lpad;
            return Some(format!(
                "  {left}{}{hint}{}{right}",
                " ".repeat(lpad),
                " ".repeat(rpad)
            ));
        }
    }
    Some(format!("  {left}{}{right}", " ".repeat(padding.max(2))))
}

fn plural(count: usize, noun: &str) -> String {
    if count == 1 {
        format!("{count} {noun}")
    } else {
        format!("{count} {noun}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const OVERHEAD: Overhead = Overhead {
        first_column: 4,
        per_column: 3,
    };

    fn widths(layout: &[LayoutColumn]) -> Vec<Option<usize>> {
        layout
            .iter()
            .map(|c| match c {
                LayoutColumn::Data { width, .. } => Some(*width),
                LayoutColumn::Ellipsis => None,
            })
            .collect()
    }

    fn rendered_width(layout: &[LayoutColumn]) -> usize {
        let content: usize = widths(layout).iter().map(|w| w.unwrap_or(1)).sum();
        content + OVERHEAD.total(layout.len())
    }

    #[test]
    fn test_split_rows() {
        assert_eq!(split_rows(10, None), (10, 0));
        assert_eq!(split_rows(10, Some(40)), (10, 0));
        // Slightly over the limit: hiding rows would not save any lines.
        assert_eq!(split_rows(43, Some(40)), (43, 0));
        assert_eq!(split_rows(44, Some(40)), (20, 20));
        assert_eq!(split_rows(100, Some(5)), (3, 2));
    }

    #[test]
    fn test_truncate() {
        assert_eq!(truncate("hello", 10), "hello");
        assert_eq!(truncate("hello", 5), "hello");
        assert_eq!(truncate("hello", 4), "hel…");
        assert_eq!(truncate("hello", 1), "…");
        assert_eq!(truncate("hello", 0), "");
        assert_eq!(truncate("ab\nhello", 4), "ab\nhel…");
        // Wide characters take two columns.
        assert_eq!(truncate("日本語です", 6), "日本…");
    }

    #[test]
    fn test_middle_out() {
        assert_eq!(middle_out(0).collect::<Vec<_>>(), Vec::<usize>::new());
        assert_eq!(middle_out(1).collect::<Vec<_>>(), vec![0]);
        assert_eq!(middle_out(4).collect::<Vec<_>>(), vec![2, 1, 3, 0]);
        assert_eq!(middle_out(5).collect::<Vec<_>>(), vec![2, 1, 3, 0, 4]);
        assert_eq!(middle_out(6).collect::<Vec<_>>(), vec![3, 2, 4, 1, 5, 0]);
    }

    #[test]
    fn test_layout_unlimited() {
        let layout = layout_columns(&[5, 50, 500], None, OVERHEAD);
        assert_eq!(widths(&layout), vec![Some(5), Some(50), Some(500)]);
    }

    #[test]
    fn test_layout_fits() {
        let layout = layout_columns(&[5, 10], Some(80), OVERHEAD);
        assert_eq!(widths(&layout), vec![Some(5), Some(10)]);
    }

    #[test]
    fn test_layout_shortens_widest_first() {
        // 1 + 3*3 + (5 + 30 + 60) = 105 > 80, need to shed 25
        let layout = layout_columns(&[5, 30, 60], Some(80), OVERHEAD);
        assert_eq!(widths(&layout), vec![Some(5), Some(30), Some(35)]);
        assert_eq!(rendered_width(&layout), 80);
    }

    #[test]
    fn test_layout_shortens_equally() {
        let layout = layout_columns(&[50, 50], Some(80), OVERHEAD);
        assert_eq!(widths(&layout), vec![Some(37), Some(36)]);
        assert_eq!(rendered_width(&layout), 80);
    }

    #[test]
    fn test_layout_prunes_middle_columns() {
        let layout = layout_columns(&[20; 10], Some(80), OVERHEAD);
        // Three data columns fit alongside the ellipsis column; the middle
        // columns are hidden first.
        assert_eq!(
            layout,
            vec![
                LayoutColumn::Data {
                    index: 0,
                    width: 20
                },
                LayoutColumn::Data {
                    index: 1,
                    width: 20
                },
                LayoutColumn::Ellipsis,
                LayoutColumn::Data {
                    index: 9,
                    width: 20
                },
            ]
        );
        assert!(rendered_width(&layout) <= 80);
    }

    #[test]
    fn test_layout_prune_gives_back_space() {
        let layout = layout_columns(&[30, 30, 30, 30], Some(80), OVERHEAD);
        assert!(layout.contains(&LayoutColumn::Ellipsis));
        assert!(rendered_width(&layout) <= 80);
        // The leftover space is handed back to the first visible column.
        assert!(
            matches!(layout[0], LayoutColumn::Data { width, .. } if width > MIN_COL_WIDTH_BEFORE_PRUNING)
        );
    }

    #[test]
    fn test_layout_single_wide_column() {
        let layout = layout_columns(&[500], Some(80), OVERHEAD);
        assert_eq!(widths(&layout), vec![Some(76)]);
        assert_eq!(rendered_width(&layout), 80);
    }

    #[test]
    fn test_layout_narrow_terminal() {
        let layout = layout_columns(&[30, 30], Some(20), OVERHEAD);
        assert!(rendered_width(&layout) <= 20, "{layout:?}");
    }

    #[test]
    fn test_footer() {
        assert_eq!(footer(10, 10, 3, 3, 40, None), None);
        assert_eq!(footer(10, 10, 3, 3, 40, Some("hint")), None);
        assert_eq!(
            footer(100, 40, 3, 3, 40, None).unwrap(),
            "  100 rows (40 shown)        3 columns"
        );
        assert_eq!(
            footer(1, 1, 10, 4, 0, None).unwrap(),
            "  1 row  10 columns (4 shown)"
        );
    }

    #[test]
    fn test_footer_hint() {
        // Enough room: the hint is centered between the counts.
        assert_eq!(
            footer(100, 40, 3, 3, 50, Some("hint")).unwrap(),
            "  100 rows (40 shown)       hint       3 columns"
        );
        // Not enough room: the hint is dropped.
        assert_eq!(
            footer(100, 40, 3, 3, 40, Some("hint")).unwrap(),
            "  100 rows (40 shown)        3 columns"
        );
    }
}
