// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

use arrow_array::RecordBatch;
use arrow_cast::display::array_value_to_string;
use arrow_schema::ArrowError;
use clap::ValueEnum;
use comfy_table::{Cell, ContentArrangement, Table, presets};
use std::fmt;

#[derive(Debug, Clone, Copy, Default, PartialEq, ValueEnum)]
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

impl TableMode {
    fn as_preset(self) -> &'static str {
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
            Self::Utf8Compact => "││──├─┼┤│    ┬┴┌┐└┘",
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

pub fn print_batches(results: &[RecordBatch], mode: TableMode) -> Result<(), ArrowError> {
    println!("{}", create_table(results, mode)?);
    Ok(())
}

fn create_table(results: &[RecordBatch], mode: TableMode) -> Result<Table, ArrowError> {
    let mut table = Table::new();
    table.load_preset(mode.as_preset());
    table.set_content_arrangement(ContentArrangement::Dynamic);

    if results.is_empty() {
        return Ok(table);
    }

    let schema = results[0].schema();
    let mut header = Vec::new();
    for field in schema.fields() {
        header.push(Cell::new(field.name()));
    }
    table.set_header(header);

    for batch in results {
        if batch.columns().len() != schema.fields().len() {
            return Err(ArrowError::InvalidArgumentError(format!(
                "Expected the same number of columns in a record batch ({}) as the number of fields ({}) in the schema",
                batch.columns().len(),
                schema.fields().len()
            )));
        }

        for row_idx in 0..batch.num_rows() {
            let mut row = Vec::new();
            for col_idx in 0..batch.num_columns() {
                let column = batch.column(col_idx);
                let cell_value = array_value_to_string(column, row_idx)?;
                row.push(Cell::new(cell_value));
            }
            table.add_row(row);
        }
    }

    Ok(table)
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{ArrayRef, Int32Array, StringArray};
    use arrow_schema::{DataType, Field, Schema};
    use std::sync::Arc;

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
        let result = print_batches(&results, TableMode::default());
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
}
