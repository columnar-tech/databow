// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

use arrow::csv::writer::Writer as CsvWriter;
use arrow::ipc::writer::{FileWriter as IpcWriter, StreamWriter as IpcStreamWriter};
use arrow::json::writer::{JsonArray, LineDelimited, Writer as JsonWriter};
use arrow_array::RecordBatch;
use arrow_schema::ArrowError;
use parquet::arrow::ArrowWriter;
use std::fs::File;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum OutputFormat {
    Json,
    Jsonl,
    Csv,
    Arrow,
    ArrowStream,
    Parquet,
}

impl OutputFormat {
    pub fn from_path(path: &Path) -> Result<Self, String> {
        match path.extension().and_then(|ext| ext.to_str()) {
            Some("json") => Ok(OutputFormat::Json),
            Some("jsonl") => Ok(OutputFormat::Jsonl),
            Some("csv") => Ok(OutputFormat::Csv),
            Some("arrow" | "ipc") => Ok(OutputFormat::Arrow),
            Some("arrows") => Ok(OutputFormat::ArrowStream),
            Some("parquet") => Ok(OutputFormat::Parquet),
            Some(ext) => Err(format!("Unsupported file extension: '.{ext}'")),
            None => Err("Cannot infer format: no file extension".to_string()),
        }
    }
}

pub fn write_batches_to_file(batches: &[RecordBatch], path: &Path) -> Result<(), ArrowError> {
    if batches.is_empty() {
        return Ok(());
    }

    let format = OutputFormat::from_path(path).map_err(ArrowError::InvalidArgumentError)?;
    let file = File::create(path).map_err(|e| ArrowError::IoError(e.to_string(), e))?;

    match format {
        OutputFormat::Json => write_json(batches, file),
        OutputFormat::Jsonl => write_jsonl(batches, file),
        OutputFormat::Csv => write_csv(batches, file),
        OutputFormat::Arrow => write_arrow_ipc(batches, file),
        OutputFormat::ArrowStream => write_arrow_stream(batches, file),
        OutputFormat::Parquet => write_parquet(batches, file),
    }
}

fn write_json(batches: &[RecordBatch], file: File) -> Result<(), ArrowError> {
    let mut writer = JsonWriter::<_, JsonArray>::new(file);
    for batch in batches {
        writer.write(batch)?;
    }
    writer.finish()?;

    Ok(())
}

fn write_jsonl(batches: &[RecordBatch], file: File) -> Result<(), ArrowError> {
    let mut writer = JsonWriter::<_, LineDelimited>::new(file);
    for batch in batches {
        writer.write(batch)?;
    }
    writer.finish()?;

    Ok(())
}

fn write_csv(batches: &[RecordBatch], file: File) -> Result<(), ArrowError> {
    let mut writer = CsvWriter::new(file);
    for batch in batches {
        writer.write(batch)?;
    }

    Ok(())
}

fn write_arrow_ipc(batches: &[RecordBatch], file: File) -> Result<(), ArrowError> {
    if batches.is_empty() {
        return Ok(());
    }
    let schema = batches[0].schema();
    let mut writer = IpcWriter::try_new(file, &schema)?;
    for batch in batches {
        writer.write(batch)?;
    }
    writer.finish()?;

    Ok(())
}

fn write_arrow_stream(batches: &[RecordBatch], file: File) -> Result<(), ArrowError> {
    if batches.is_empty() {
        return Ok(());
    }
    let schema = batches[0].schema();
    let mut writer = IpcStreamWriter::try_new(file, &schema)?;
    for batch in batches {
        writer.write(batch)?;
    }
    writer.finish()?;

    Ok(())
}

fn write_parquet(batches: &[RecordBatch], file: File) -> Result<(), ArrowError> {
    if batches.is_empty() {
        return Ok(());
    }
    let schema = batches[0].schema();
    let mut writer = ArrowWriter::try_new(file, schema, None)
        .map_err(|e| ArrowError::ExternalError(Box::new(e)))?;
    for batch in batches {
        writer
            .write(batch)
            .map_err(|e| ArrowError::ExternalError(Box::new(e)))?;
    }
    writer
        .close()
        .map_err(|e| ArrowError::ExternalError(Box::new(e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{ArrayRef, Int32Array, StringArray};
    use arrow_schema::{DataType, Field, Schema};
    use std::io::Read;
    use std::sync::Arc;
    use tempfile::tempdir;

    fn create_test_batch() -> RecordBatch {
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int32, false),
            Field::new("name", DataType::Utf8, false),
        ]));
        let id_array: ArrayRef = Arc::new(Int32Array::from(vec![1, 2, 3]));
        let name_array: ArrayRef = Arc::new(StringArray::from(vec!["Alice", "Bob", "Charlie"]));

        RecordBatch::try_new(schema, vec![id_array, name_array]).unwrap()
    }

    #[test]
    fn test_output_format_from_path_json() {
        let path = Path::new("output.json");
        assert_eq!(OutputFormat::from_path(path).unwrap(), OutputFormat::Json);
    }

    #[test]
    fn test_output_format_from_path_csv() {
        let path = Path::new("output.csv");
        assert_eq!(OutputFormat::from_path(path).unwrap(), OutputFormat::Csv);
    }

    #[test]
    fn test_output_format_from_path_arrow() {
        let path = Path::new("output.arrow");
        assert_eq!(OutputFormat::from_path(path).unwrap(), OutputFormat::Arrow);
    }

    #[test]
    fn test_output_format_from_path_ipc() {
        let path = Path::new("output.ipc");
        assert_eq!(OutputFormat::from_path(path).unwrap(), OutputFormat::Arrow);
    }

    #[test]
    fn test_output_format_from_path_unsupported() {
        let path = Path::new("output.txt");
        let result = OutputFormat::from_path(path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported file extension"));
    }

    #[test]
    fn test_output_format_from_path_no_extension() {
        let path = Path::new("output");
        let result = OutputFormat::from_path(path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("no file extension"));
    }

    #[test]
    fn test_write_json() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.json");
        let batch = create_test_batch();

        write_batches_to_file(&[batch], &path).unwrap();

        let mut file = File::open(&path).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();

        assert!(contents.contains("Alice"));
        assert!(contents.contains("Bob"));
        assert!(contents.contains("Charlie"));
        assert!(contents.starts_with('['));
        assert!(contents.trim().ends_with(']'));
    }

    #[test]
    fn test_write_csv() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.csv");
        let batch = create_test_batch();

        write_batches_to_file(&[batch], &path).unwrap();

        let mut file = File::open(&path).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();

        assert!(contents.contains("id,name"));
        assert!(contents.contains("1,Alice"));
        assert!(contents.contains("2,Bob"));
        assert!(contents.contains("3,Charlie"));
    }

    #[test]
    fn test_write_arrow_ipc() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.arrow");
        let batch = create_test_batch();

        write_batches_to_file(std::slice::from_ref(&batch), &path).unwrap();

        // Verify by reading it back
        let file = File::open(&path).unwrap();
        let reader = arrow::ipc::reader::FileReader::try_new(file, None).unwrap();
        let read_batches: Vec<RecordBatch> = reader.map(|r| r.unwrap()).collect();

        assert_eq!(read_batches.len(), 1);
        assert_eq!(read_batches[0].num_rows(), batch.num_rows());
        assert_eq!(read_batches[0].num_columns(), batch.num_columns());
    }

    #[test]
    fn test_write_arrow_ipc_empty_batches_direct() {
        // Test that write_arrow_ipc handles empty batches directly (defensive coding)
        // This is a regression test for the panic that would occur at src/output.rs:66
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.arrow");
        let file = File::create(&path).unwrap();

        // Direct call to write_arrow_ipc with empty batches - should not panic
        let result = write_arrow_ipc(&[], file);

        assert!(result.is_ok());
        // File should be created but empty since we opened it before the empty check
        // Actually, let's just verify it doesn't panic and returns Ok
    }

    #[test]
    fn test_write_empty_batches_json() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.json");

        // Empty batches should not create a file
        write_batches_to_file(&[], &path).unwrap();

        // Verify file was not created
        assert!(!path.exists());
    }

    #[test]
    fn test_write_empty_batches_arrow() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.arrow");

        // Empty batches should not create a file
        write_batches_to_file(&[], &path).unwrap();

        // Verify file was not created
        assert!(!path.exists());
    }

    #[test]
    fn test_write_empty_batches_csv() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.csv");

        // Empty batches should not create a file
        write_batches_to_file(&[], &path).unwrap();

        // Verify file was not created
        assert!(!path.exists());
    }

    #[test]
    fn test_output_format_from_path_arrows() {
        let path = Path::new("output.arrows");
        assert_eq!(
            OutputFormat::from_path(path).unwrap(),
            OutputFormat::ArrowStream
        );
    }

    #[test]
    fn test_output_format_from_path_parquet() {
        let path = Path::new("output.parquet");
        assert_eq!(
            OutputFormat::from_path(path).unwrap(),
            OutputFormat::Parquet
        );
    }

    #[test]
    fn test_output_format_from_path_jsonl() {
        let path = Path::new("output.jsonl");
        assert_eq!(OutputFormat::from_path(path).unwrap(), OutputFormat::Jsonl);
    }

    #[test]
    fn test_write_arrow_stream() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.arrows");
        let batch = create_test_batch();

        write_batches_to_file(std::slice::from_ref(&batch), &path).unwrap();

        // Verify by reading it back as an IPC stream
        let file = File::open(&path).unwrap();
        let reader = arrow::ipc::reader::StreamReader::try_new(file, None).unwrap();
        let read_batches: Vec<RecordBatch> = reader.map(|r| r.unwrap()).collect();

        assert_eq!(read_batches.len(), 1);
        assert_eq!(read_batches[0].num_rows(), batch.num_rows());
        assert_eq!(read_batches[0].num_columns(), batch.num_columns());
    }

    #[test]
    fn test_write_arrow_stream_empty_batches_direct() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.arrows");
        let file = File::create(&path).unwrap();

        let result = write_arrow_stream(&[], file);

        assert!(result.is_ok());
    }

    #[test]
    fn test_write_parquet() {
        use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

        let dir = tempdir().unwrap();
        let path = dir.path().join("output.parquet");
        let batch = create_test_batch();

        write_batches_to_file(std::slice::from_ref(&batch), &path).unwrap();

        // Verify by reading it back
        let file = File::open(&path).unwrap();
        let reader = ParquetRecordBatchReaderBuilder::try_new(file)
            .unwrap()
            .build()
            .unwrap();
        let read_batches: Vec<RecordBatch> = reader.map(|r| r.unwrap()).collect();

        let total_rows: usize = read_batches.iter().map(|b| b.num_rows()).sum();
        assert_eq!(total_rows, batch.num_rows());
        assert_eq!(read_batches[0].num_columns(), batch.num_columns());
    }

    #[test]
    fn test_write_parquet_empty_batches_direct() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.parquet");
        let file = File::create(&path).unwrap();

        let result = write_parquet(&[], file);

        assert!(result.is_ok());
    }

    #[test]
    fn test_write_jsonl() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.jsonl");
        let batch = create_test_batch();

        write_batches_to_file(std::slice::from_ref(&batch), &path).unwrap();

        let mut file = File::open(&path).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();

        // JSON lines: one object per line, not a wrapping array
        assert!(!contents.trim_start().starts_with('['));
        let lines: Vec<&str> = contents.lines().filter(|l| !l.is_empty()).collect();
        assert_eq!(lines.len(), 3);
        for line in &lines {
            assert!(line.trim_start().starts_with('{'));
            assert!(line.trim_end().ends_with('}'));
        }
        assert!(contents.contains("Alice"));
        assert!(contents.contains("Bob"));
        assert!(contents.contains("Charlie"));
    }

    #[test]
    fn test_write_multiple_batches() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.json");
        let batch1 = create_test_batch();
        let batch2 = create_test_batch();

        write_batches_to_file(&[batch1, batch2], &path).unwrap();

        let mut file = File::open(&path).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();

        // Should have 6 records total (3 from each batch)
        let count = contents.matches("Alice").count();
        assert_eq!(count, 2);
    }
}
