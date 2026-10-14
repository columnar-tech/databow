// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::truncate::{self, LayoutColumn, Overhead};
use arrow_array::RecordBatch;
use arrow_cast::display::array_value_to_string;
use arrow_schema::ArrowError;
use clap::ValueEnum;
use comfy_table::{
    Cell, CellAlignment, ContentArrangement, ContentLineStyle, LineStyle, Table, TableStyle,
    presets,
};
use std::fmt;
use std::io::IsTerminal;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, ValueEnum)]
pub enum TableMode {
    #[default]
    Utf8Compact,
    AsciiFull,
    AsciiFullCondensed,
    AsciiBordersOnly,
    AsciiBordersOnlyCondensed,
    AsciiHorizontalOnly,
    AsciiMarkdown,
    AsciiNoBorders,
    Utf8Full,
    Utf8FullCondensed,
    Utf8BordersOnly,
    Utf8HorizontalOnly,
    Utf8NoBorders,
    Nothing,
}

/// Hand-port of the `"││──├─┼┤│    ┬┴┌┐└┘"` preset string used before
/// comfy-table 8 replaced string presets with [`TableStyle`]: a UTF8 table with
/// outer borders and a header separator, but no separators between content rows.
/// `test_utf8_compact_exact_rendering` pins the resulting output.
const UTF8_COMPACT: TableStyle = TableStyle::new()
    .top_border(LineStyle::new('┌', '─', '┬', '┐'))
    .header_lines(ContentLineStyle::new('│', '│', '│'))
    .header_separator(LineStyle::new('├', '─', '┼', '┤'))
    .content_lines(ContentLineStyle::new('│', '│', '│'))
    .bottom_border(LineStyle::new('└', '─', '┴', '┘'));

impl TableMode {
    fn as_style(self) -> TableStyle {
        match self {
            Self::AsciiBordersOnly => presets::ASCII_BORDERS_ONLY,
            Self::AsciiBordersOnlyCondensed => presets::ASCII_BORDERS_ONLY_CONDENSED,
            Self::AsciiFull => presets::ASCII_FULL,
            Self::AsciiFullCondensed => presets::ASCII_FULL_CONDENSED,
            Self::AsciiHorizontalOnly => presets::ASCII_HORIZONTAL_ONLY,
            Self::AsciiMarkdown => presets::ASCII_MARKDOWN,
            Self::AsciiNoBorders => presets::ASCII_NO_BORDERS,
            Self::Nothing => presets::NOTHING,
            Self::Utf8BordersOnly => presets::UTF8_BORDERS_ONLY,
            Self::Utf8Compact => UTF8_COMPACT,
            Self::Utf8Full => presets::UTF8_FULL,
            Self::Utf8FullCondensed => presets::UTF8_FULL_CONDENSED,
            Self::Utf8HorizontalOnly => presets::UTF8_HORIZONTAL_ONLY,
            Self::Utf8NoBorders => presets::UTF8_NO_BORDERS,
        }
    }
}

impl fmt::Display for TableMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            self.to_possible_value()
                .expect("no values are skipped")
                .get_name()
        )
    }
}

/// A user-configurable limit such as `.maxrows`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Limit {
    /// Not set: use the default for the current output.
    #[default]
    Default,
    /// Explicitly disabled.
    Unlimited,
    /// An explicit, non-zero limit.
    Value(usize),
}

impl Limit {
    /// Build a limit from a user-supplied count, where `0` disables the limit.
    pub fn from_count(count: usize) -> Self {
        if count == 0 {
            Limit::Unlimited
        } else {
            Limit::Value(count)
        }
    }

    /// The limit to apply, using `default` when the limit is not set.
    fn resolve(self, default: Option<usize>) -> Option<usize> {
        match self {
            Limit::Default => default,
            Limit::Unlimited => None,
            Limit::Value(count) => Some(count),
        }
    }
}

/// The width of the terminal, or `None` if stdout is not a terminal or its
/// size cannot be determined.
pub fn terminal_width() -> Option<usize> {
    if std::io::stdout().is_terminal() {
        Table::new().width().map(usize::from)
    } else {
        None
    }
}

/// User-configured truncation limits, as set by dot commands.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TruncationOptions {
    pub max_rows: Limit,
    pub max_width: Limit,
}

impl TruncationOptions {
    /// Resolve the limits for the current stdout. Defaults only apply when
    /// writing to a terminal so that piped output is never truncated unless
    /// explicitly requested.
    pub fn resolve(&self) -> RenderLimits {
        self.resolve_with(std::io::stdout().is_terminal(), terminal_width())
    }

    fn resolve_with(&self, is_tty: bool, terminal_width: Option<usize>) -> RenderLimits {
        RenderLimits {
            max_rows: self
                .max_rows
                .resolve(is_tty.then_some(truncate::DEFAULT_MAX_ROWS)),
            max_width: self.max_width.resolve(terminal_width),
        }
    }
}

/// Limits applied when rendering a table. `None` means unlimited, so the
/// default applies no limits.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RenderLimits {
    pub max_rows: Option<usize>,
    pub max_width: Option<usize>,
}

/// How to render a table. `hint` is shown in the footer when rows or columns
/// were hidden, e.g. to tell the user how to see the entire result.
#[derive(Debug, Clone, Copy, Default)]
pub struct RenderOptions<'a> {
    pub mode: TableMode,
    pub limits: RenderLimits,
    pub hint: Option<&'a str>,
}

/// A rendered table plus an optional footer describing hidden rows/columns.
#[derive(Debug)]
pub struct RenderedTable {
    table: Table,
    footer: Option<String>,
    shown_rows: usize,
    width: usize,
}

impl RenderedTable {
    /// Number of result rows rendered (excluding divider rows).
    pub fn shown_rows(&self) -> usize {
        self.shown_rows
    }

    /// Width of the rendered table in display columns.
    pub fn width(&self) -> usize {
        self.width
    }
}

impl fmt::Display for RenderedTable {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.table)?;
        if let Some(footer) = &self.footer {
            write!(f, "\n{footer}")?;
        }
        Ok(())
    }
}

/// Print results without truncation, as for `--query`, `--file` and stdin.
/// Like before truncation was added, long values wrap to fit the terminal.
pub fn print_batches(results: &[RecordBatch], options: RenderOptions) -> Result<(), ArrowError> {
    let mut rendered = render_table(results, options)?;
    if options.limits.max_width.is_none() {
        rendered
            .table
            .set_content_arrangement(ContentArrangement::Dynamic);
    }
    println!("{rendered}");
    Ok(())
}

/// Measure the width a table style adds around column contents.
fn measure_overhead(style: TableStyle) -> Overhead {
    let rendered_width = |columns: usize| {
        let mut table = Table::new();
        table.load_style(style);
        table.set_content_arrangement(ContentArrangement::Disabled);
        table.set_header(vec!["x"; columns]);
        table.add_row(vec!["x"; columns]);
        table
            .lines()
            .map(|line| truncate::display_width(&line))
            .max()
            .unwrap_or(0)
            .saturating_sub(columns)
    };
    let one = rendered_width(1);
    let two = rendered_width(2);
    Overhead {
        first_column: one,
        per_column: two.saturating_sub(one),
    }
}

/// Format the first `top` and last `bottom` rows of `results`, skipping
/// batches that lie entirely within the hidden rows.
fn collect_rows(
    results: &[RecordBatch],
    total_rows: usize,
    top: usize,
    bottom: usize,
) -> Result<Vec<Vec<String>>, ArrowError> {
    let bottom_start = total_rows - bottom;
    let mut rows = Vec::with_capacity(top + bottom);
    let mut offset = 0;
    for batch in results {
        let batch_start = offset;
        offset += batch.num_rows();
        if batch_start >= top && offset <= bottom_start {
            continue;
        }
        for row_idx in 0..batch.num_rows() {
            let global = batch_start + row_idx;
            if (top..bottom_start).contains(&global) {
                continue;
            }
            let row = batch
                .columns()
                .iter()
                .map(|column| array_value_to_string(column, row_idx))
                .collect::<Result<Vec<_>, _>>()?;
            rows.push(row);
        }
    }
    Ok(rows)
}

pub fn render_table(
    results: &[RecordBatch],
    options: RenderOptions,
) -> Result<RenderedTable, ArrowError> {
    let RenderOptions { mode, limits, hint } = options;
    let style = mode.as_style();
    let mut table = Table::new();
    table.load_style(style);
    // Column widths are computed up front, so comfy-table must not rearrange.
    table.set_content_arrangement(ContentArrangement::Disabled);

    let Some(first) = results.first() else {
        return Ok(RenderedTable {
            table,
            footer: None,
            shown_rows: 0,
            width: 0,
        });
    };

    let schema = first.schema();
    let column_count = schema.fields().len();
    for batch in results {
        if batch.num_columns() != column_count {
            return Err(ArrowError::InvalidArgumentError(format!(
                "Expected the same number of columns in a record batch ({}) as the number of fields ({}) in the schema",
                batch.num_columns(),
                column_count
            )));
        }
    }

    // Only format the rows that will actually be shown.
    let total_rows: usize = results.iter().map(RecordBatch::num_rows).sum();
    let (top, bottom) = truncate::split_rows(total_rows, limits.max_rows);
    let rows = collect_rows(results, total_rows, top, bottom)?;
    let shown_rows = rows.len();
    let rows_hidden = shown_rows < total_rows;

    let header: Vec<String> = schema.fields().iter().map(|f| f.name().clone()).collect();
    // comfy-table renders empty columns one wide, so never measure below 1.
    let mut natural: Vec<usize> = header
        .iter()
        .map(|h| truncate::display_width(h).max(1))
        .collect();
    for row in &rows {
        for (width, value) in natural.iter_mut().zip(row) {
            *width = (*width).max(truncate::display_width(value));
        }
    }

    let overhead = measure_overhead(style);
    let layout = truncate::layout_columns(&natural, limits.max_width, overhead);

    let make_row = |values: &[String]| -> Vec<Cell> {
        layout
            .iter()
            .map(|column| match *column {
                LayoutColumn::Data { index, width } => {
                    Cell::new(truncate::truncate(&values[index], width))
                }
                LayoutColumn::Ellipsis => Cell::new(truncate::ELLIPSIS),
            })
            .collect()
    };

    table.set_header(make_row(&header));
    for (i, row) in rows.iter().enumerate() {
        if rows_hidden && i == top {
            add_divider_rows(&mut table, &layout);
        }
        table.add_row(make_row(row));
    }
    if rows_hidden && top == shown_rows {
        // No bottom rows to show (e.g. max_rows = 1): put the divider last.
        add_divider_rows(&mut table, &layout);
    }

    let shown_columns = layout
        .iter()
        .filter(|c| matches!(c, LayoutColumn::Data { .. }))
        .count();
    let table_width = overhead.total(layout.len())
        + layout
            .iter()
            .map(|c| match *c {
                LayoutColumn::Data { width, .. } => width,
                LayoutColumn::Ellipsis => truncate::display_width(truncate::ELLIPSIS),
            })
            .sum::<usize>();
    let footer = truncate::footer(
        total_rows,
        shown_rows,
        column_count,
        shown_columns,
        table_width,
        hint,
    );

    Ok(RenderedTable {
        table,
        footer,
        shown_rows,
        width: table_width,
    })
}

/// Add the rows of dots that stand in for hidden rows.
fn add_divider_rows(table: &mut Table, layout: &[LayoutColumn]) {
    for _ in 0..truncate::DIVIDER_ROWS {
        let row: Vec<Cell> = layout
            .iter()
            .map(|column| match *column {
                LayoutColumn::Data { .. } => {
                    Cell::new(truncate::DOT).set_alignment(CellAlignment::Center)
                }
                LayoutColumn::Ellipsis => Cell::new(truncate::ELLIPSIS),
            })
            .collect();
        table.add_row(row);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{ArrayRef, Int32Array, StringArray};
    use arrow_schema::{DataType, Field, Schema};
    use std::sync::Arc;

    fn create_table(results: &[RecordBatch], mode: TableMode) -> Result<RenderedTable, ArrowError> {
        render_table(
            results,
            RenderOptions {
                mode,
                ..Default::default()
            },
        )
    }

    fn create_test_schema() -> Schema {
        Schema::new(vec![
            Field::new("id", DataType::Int32, false),
            Field::new("name", DataType::Utf8, false),
        ])
    }

    fn create_test_batch() -> RecordBatch {
        let schema = Arc::new(create_test_schema());
        let id_array: ArrayRef = Arc::new(Int32Array::from(vec![1, 2, 3]));
        let name_array: ArrayRef = Arc::new(StringArray::from(vec!["Alice", "Bob", "Charlie"]));

        RecordBatch::try_new(schema, vec![id_array, name_array]).unwrap()
    }

    #[test]
    fn test_create_table_empty() {
        let results: Vec<RecordBatch> = vec![];
        let table = create_table(&results, TableMode::default()).unwrap();
        // Empty table should be created successfully with no panic
        let _table_str = table.to_string();
    }

    #[test]
    fn test_create_table_single_batch() {
        let batch = create_test_batch();
        let results = vec![batch];
        let table = create_table(&results, TableMode::default()).unwrap();
        let table_str = table.to_string();

        // Should contain headers
        assert!(table_str.contains("id"));
        assert!(table_str.contains("name"));

        // Should contain data
        assert!(table_str.contains("1"));
        assert!(table_str.contains("Alice"));
        assert!(table_str.contains("2"));
        assert!(table_str.contains("Bob"));
        assert!(table_str.contains("3"));
        assert!(table_str.contains("Charlie"));
    }

    #[test]
    fn test_create_table_multiple_batches() {
        let batch1 = create_test_batch();
        let batch2 = create_test_batch();
        let results = vec![batch1, batch2];
        let table = create_table(&results, TableMode::default()).unwrap();
        let table_str = table.to_string();

        // Should have data from both batches (6 rows total)
        assert!(table_str.contains("Alice"));
        assert!(table_str.contains("Bob"));
        assert!(table_str.contains("Charlie"));
    }

    #[test]
    fn test_create_table_mismatched_columns() {
        let schema1 = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int32, false),
            Field::new("name", DataType::Utf8, false),
        ]));

        let schema2 = Arc::new(Schema::new(vec![Field::new("id", DataType::Int32, false)]));

        let batch1 = RecordBatch::try_new(
            schema1,
            vec![
                Arc::new(Int32Array::from(vec![1])),
                Arc::new(StringArray::from(vec!["Alice"])),
            ],
        )
        .unwrap();

        let batch2 =
            RecordBatch::try_new(schema2, vec![Arc::new(Int32Array::from(vec![2]))]).unwrap();

        let results = vec![batch1, batch2];
        let result = create_table(&results, TableMode::default());

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string()
                .contains("Expected the same number of columns")
        );
    }

    #[test]
    fn test_print_batches_empty() {
        let results: Vec<RecordBatch> = vec![];
        // Should not panic on empty results
        let result = print_batches(&results, RenderOptions::default());
        assert!(result.is_ok());
    }

    #[test]
    fn test_create_table_with_nulls() {
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int32, true),
            Field::new("name", DataType::Utf8, true),
        ]));

        let id_array: ArrayRef = Arc::new(Int32Array::from(vec![Some(1), None, Some(3)]));
        let name_array: ArrayRef =
            Arc::new(StringArray::from(vec![Some("Alice"), Some("Bob"), None]));

        let batch = RecordBatch::try_new(schema, vec![id_array, name_array]).unwrap();
        let results = vec![batch];
        let table = create_table(&results, TableMode::default()).unwrap();
        let table_str = table.to_string();

        // Should contain headers
        assert!(table_str.contains("id"));
        assert!(table_str.contains("name"));

        // Should contain data
        assert!(table_str.contains("1"));
        assert!(table_str.contains("Alice"));
        assert!(table_str.contains("Bob"));
    }

    #[test]
    fn test_table_mode_value_enum_valid() {
        use clap::ValueEnum;

        assert_eq!(
            TableMode::from_str("ascii-full", false).unwrap(),
            TableMode::AsciiFull
        );
        assert_eq!(
            TableMode::from_str("ascii-full-condensed", false).unwrap(),
            TableMode::AsciiFullCondensed
        );
        assert_eq!(
            TableMode::from_str("ascii-borders-only", false).unwrap(),
            TableMode::AsciiBordersOnly
        );
        assert_eq!(
            TableMode::from_str("ascii-borders-only-condensed", false).unwrap(),
            TableMode::AsciiBordersOnlyCondensed
        );
        assert_eq!(
            TableMode::from_str("ascii-horizontal-only", false).unwrap(),
            TableMode::AsciiHorizontalOnly
        );
        assert_eq!(
            TableMode::from_str("ascii-markdown", false).unwrap(),
            TableMode::AsciiMarkdown
        );
        assert_eq!(
            TableMode::from_str("ascii-no-borders", false).unwrap(),
            TableMode::AsciiNoBorders
        );
        assert_eq!(
            TableMode::from_str("utf8-full", false).unwrap(),
            TableMode::Utf8Full
        );
        assert_eq!(
            TableMode::from_str("utf8-full-condensed", false).unwrap(),
            TableMode::Utf8FullCondensed
        );
        assert_eq!(
            TableMode::from_str("utf8-borders-only", false).unwrap(),
            TableMode::Utf8BordersOnly
        );
        assert_eq!(
            TableMode::from_str("utf8-horizontal-only", false).unwrap(),
            TableMode::Utf8HorizontalOnly
        );
        assert_eq!(
            TableMode::from_str("utf8-no-borders", false).unwrap(),
            TableMode::Utf8NoBorders
        );
        assert_eq!(
            TableMode::from_str("nothing", false).unwrap(),
            TableMode::Nothing
        );
    }

    #[test]
    fn test_table_mode_value_enum_invalid() {
        use clap::ValueEnum;

        let result = TableMode::from_str("invalid_mode", false);
        assert!(result.is_err());
    }

    #[test]
    fn test_table_mode_display() {
        assert_eq!(TableMode::AsciiFull.to_string(), "ascii-full");
        assert_eq!(
            TableMode::Utf8FullCondensed.to_string(),
            "utf8-full-condensed"
        );
        assert_eq!(TableMode::Nothing.to_string(), "nothing");
    }

    #[test]
    fn test_truncation_defaults_only_apply_in_terminal() {
        let defaults = TruncationOptions::default();
        assert_eq!(
            defaults.resolve_with(true, Some(100)),
            RenderLimits {
                max_rows: Some(40),
                max_width: Some(100),
            }
        );
        assert_eq!(defaults.resolve_with(false, None), RenderLimits::default());
    }

    #[test]
    fn test_truncation_explicit_limits() {
        let explicit = TruncationOptions {
            max_rows: Limit::Value(5),
            max_width: Limit::Value(60),
        };
        let expected = RenderLimits {
            max_rows: Some(5),
            max_width: Some(60),
        };
        assert_eq!(explicit.resolve_with(true, Some(100)), expected);
        assert_eq!(explicit.resolve_with(false, None), expected);

        let unlimited = TruncationOptions {
            max_rows: Limit::Unlimited,
            max_width: Limit::Unlimited,
        };
        assert_eq!(
            unlimited.resolve_with(true, Some(100)),
            RenderLimits::default()
        );
    }

    #[test]
    fn test_rendered_width_matches_layout() {
        // Wide, multi-line, wide-character and empty columns in every style:
        // the computed width must match the real one and respect max_width.
        let long = "x".repeat(200);
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int32, false),
            Field::new("text", DataType::Utf8, false),
            Field::new("long", DataType::Utf8, false),
            Field::new("", DataType::Utf8, false),
        ]));
        let columns: Vec<ArrayRef> = vec![
            Arc::new(Int32Array::from((0..100).collect::<Vec<_>>())),
            Arc::new(StringArray::from(vec!["日本語です\nline2"; 100])),
            Arc::new(StringArray::from(vec![long.as_str(); 100])),
            Arc::new(StringArray::from(vec![""; 100])),
        ];
        let batch = RecordBatch::try_new(schema, columns).unwrap();

        for &mode in TableMode::value_variants() {
            for max_width in [Some(10), Some(40), Some(80), None] {
                let rendered = render_table(
                    std::slice::from_ref(&batch),
                    RenderOptions {
                        mode,
                        limits: RenderLimits {
                            max_rows: Some(5),
                            max_width,
                        },
                        hint: None,
                    },
                )
                .unwrap();
                let actual = rendered
                    .table
                    .lines()
                    .map(|line| truncate::display_width(&line))
                    .max()
                    .unwrap_or(0);
                assert_eq!(rendered.width(), actual, "{mode} {max_width:?}");
                if let Some(max_width) = max_width {
                    assert!(actual <= max_width, "{mode} {max_width}: {actual}");
                }
            }
        }
    }

    #[test]
    fn test_table_mode_default() {
        assert_eq!(TableMode::default(), TableMode::Utf8Compact);
    }

    #[test]
    fn test_create_table_with_different_modes() {
        let batch = create_test_batch();
        let results = vec![batch];

        // Test that different modes produce valid tables
        for mode in [
            TableMode::AsciiFull,
            TableMode::AsciiMarkdown,
            TableMode::Utf8Full,
            TableMode::Nothing,
        ] {
            let table = create_table(&results, mode).unwrap();
            let table_str = table.to_string();
            // All modes should still contain the data
            assert!(table_str.contains("Alice"), "Mode {:?} missing data", mode);
        }
    }

    /// `Utf8Compact` is the default mode and its style is defined by hand in this
    /// module rather than supplied by comfy-table, so assert the exact rendering to
    /// catch any accidental change to a border character.
    #[test]
    fn test_utf8_compact_exact_rendering() {
        let table = create_table(&[create_test_batch()], TableMode::Utf8Compact).unwrap();
        let expected = [
            "┌────┬─────────┐",
            "│ id │ name    │",
            "├────┼─────────┤",
            "│ 1  │ Alice   │",
            "│ 2  │ Bob     │",
            "│ 3  │ Charlie │",
            "└────┴─────────┘",
        ]
        .join("\n");
        assert_eq!(table.to_string(), expected);
    }
}
