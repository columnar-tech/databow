// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

use arrow_array::RecordBatch;
use arrow_cast::display::array_value_to_string;
use arrow_schema::ArrowError;
use comfy_table::{Cell, ContentArrangement, Table};

pub fn print_batches(results: &[RecordBatch]) -> Result<(), ArrowError> {
    println!("{}", create_table(results)?);
    Ok(())
}

fn create_table(results: &[RecordBatch]) -> Result<Table, ArrowError> {
    let mut table = Table::new();
    table.load_preset("││──╞═╪╡│    ┬┴┌┐└┘");
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
        let table = create_table(&results).unwrap();
        // Empty table should be created successfully with no panic
        let _table_str = table.to_string();
    }

    #[test]
    fn test_create_table_single_batch() {
        let batch = create_test_batch();
        let results = vec![batch];
        let table = create_table(&results).unwrap();
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
        let table = create_table(&results).unwrap();
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
        let result = create_table(&results);

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
        let result = print_batches(&results);
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
        let table = create_table(&results).unwrap();
        let table_str = table.to_string();

        // Should contain headers
        assert!(table_str.contains("id"));
        assert!(table_str.contains("name"));

        // Should contain data
        assert!(table_str.contains("1"));
        assert!(table_str.contains("Alice"));
        assert!(table_str.contains("Bob"));
    }
}
