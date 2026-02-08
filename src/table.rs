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
