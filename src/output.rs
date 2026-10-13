// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

use arrow::csv::writer::Writer as CsvWriter;
use arrow::ipc::writer::{FileWriter as IpcWriter, StreamWriter as IpcStreamWriter};
use arrow::json::writer::{JsonArray, LineDelimited, Writer as JsonWriter};
use arrow_array::RecordBatch;
use arrow_schema::{ArrowError, SchemaRef};
use parquet::arrow::ArrowWriter;
use parquet::basic::Compression;
use parquet::file::properties::WriterProperties;
use std::io::Write;
use std::path::{Path, PathBuf};
use tempfile::NamedTempFile;

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

/// Write query results to `path`, inferring the format from its extension.
///
/// Output is written to a temporary file in the same directory and atomically
/// renamed into place only after the write succeeds, so a failed write never
/// leaves a partial file behind or clobbers an existing one. If `path` is a
/// symlink, the file it points to is replaced and the link is left intact.
///
/// `schema` is used when there are no batches so that formats which can carry
/// a schema (Arrow IPC, Parquet, CSV header) still produce a valid, empty file.
pub fn write_batches_to_file(
    schema: &SchemaRef,
    batches: &[RecordBatch],
    path: &Path,
) -> Result<(), ArrowError> {
    let format = OutputFormat::from_path(path).map_err(ArrowError::InvalidArgumentError)?;

    // Prefer the schema of the actual batches: some writers (e.g. Parquet)
    // reject batches whose schema differs from the writer's schema, and the
    // reader-reported schema may differ in metadata.
    let schema = batches
        .first()
        .map(|b| b.schema())
        .unwrap_or_else(|| schema.clone());

    let target = resolve_symlinks(path).map_err(io_error)?;
    check_writable(&target).map_err(io_error)?;

    let mut tmp = create_temp_file(&target).map_err(io_error)?;
    let file = tmp.as_file_mut();

    match format {
        OutputFormat::Json => write_json(batches, &mut *file),
        OutputFormat::Jsonl => write_jsonl(batches, &mut *file),
        OutputFormat::Csv => write_csv(&schema, batches, &mut *file),
        OutputFormat::Arrow => write_arrow_ipc(&schema, batches, &mut *file),
        OutputFormat::ArrowStream => write_arrow_stream(&schema, batches, &mut *file),
        OutputFormat::Parquet => write_parquet(&schema, batches, &mut *file),
    }?;

    // Make sure the data is on disk before the rename makes it visible, so a
    // crash can't leave an empty or truncated file in place of the old one.
    file.sync_all().map_err(io_error)?;

    // On any error above, `tmp` is dropped and the temp file is removed.
    tmp.persist(&target).map_err(|e| io_error(e.error))?;

    Ok(())
}

fn io_error(e: std::io::Error) -> ArrowError {
    ArrowError::IoError(e.to_string(), e)
}

/// Follow `path` through any symlinks to the file that should actually be
/// replaced. The final target need not exist (a dangling symlink resolves to
/// the path it points at).
fn resolve_symlinks(path: &Path) -> std::io::Result<PathBuf> {
    // Same limit Linux uses (MAXSYMLINKS) to detect loops.
    const MAX_SYMLINKS: usize = 40;

    let mut current = path.to_path_buf();
    for _ in 0..MAX_SYMLINKS {
        match std::fs::symlink_metadata(&current) {
            Ok(meta) if meta.file_type().is_symlink() => {
                let link = std::fs::read_link(&current)?;
                current = match current.parent() {
                    Some(parent) if link.is_relative() => parent.join(link),
                    _ => link,
                };
            }
            _ => return Ok(current),
        }
    }
    Err(std::io::Error::other(format!(
        "Too many levels of symbolic links: {}",
        path.display()
    )))
}

/// Fail early if `path` exists but can't be written, matching what
/// `File::create` would do. Without this, renaming over a read-only file
/// would succeed on Unix, since rename only needs directory permissions.
fn check_writable(path: &Path) -> std::io::Result<()> {
    match std::fs::OpenOptions::new().write(true).open(path) {
        Ok(_) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Directory in which to create the temp file for `path`: its parent, or the
/// current directory for a bare file name.
fn temp_dir_for(path: &Path) -> &Path {
    match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    }
}

/// Create a temporary file next to `path` so it can be atomically renamed
/// over the destination.
fn create_temp_file(path: &Path) -> std::io::Result<NamedTempFile> {
    let file_name = path.file_name().unwrap_or_default();
    let mut prefix = std::ffi::OsString::from(".");
    prefix.push(file_name);
    prefix.push(".");

    let mut builder = tempfile::Builder::new();
    builder.prefix(&prefix).suffix(".tmp");

    // tempfile defaults to 0o600; match what `File::create` would produce
    // (0o666 minus umask), or keep the existing file's permissions.
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        builder.permissions(std::fs::Permissions::from_mode(0o666));
    }

    let tmp = builder.tempfile_in(temp_dir_for(path))?;

    #[cfg(unix)]
    if let Ok(metadata) = std::fs::metadata(path) {
        std::fs::set_permissions(tmp.path(), metadata.permissions())?;
    }

    Ok(tmp)
}

fn write_json<W: Write>(batches: &[RecordBatch], out: W) -> Result<(), ArrowError> {
    let mut writer = JsonWriter::<_, JsonArray>::new(out);
    for batch in batches {
        writer.write(batch)?;
    }
    writer.finish()?;

    Ok(())
}

fn write_jsonl<W: Write>(batches: &[RecordBatch], out: W) -> Result<(), ArrowError> {
    let mut writer = JsonWriter::<_, LineDelimited>::new(out);
    for batch in batches {
        writer.write(batch)?;
    }
    writer.finish()?;

    Ok(())
}

fn write_csv<W: Write>(
    schema: &SchemaRef,
    batches: &[RecordBatch],
    out: W,
) -> Result<(), ArrowError> {
    let mut writer = CsvWriter::new(out);
    if batches.is_empty() {
        // Writing an empty batch emits just the header row.
        writer.write(&RecordBatch::new_empty(schema.clone()))?;
    }
    for batch in batches {
        writer.write(batch)?;
    }

    Ok(())
}

fn write_arrow_ipc<W: Write>(
    schema: &SchemaRef,
    batches: &[RecordBatch],
    out: W,
) -> Result<(), ArrowError> {
    let mut writer = IpcWriter::try_new(out, schema)?;
    for batch in batches {
        writer.write(batch)?;
    }
    writer.finish()?;

    Ok(())
}

fn write_arrow_stream<W: Write>(
    schema: &SchemaRef,
    batches: &[RecordBatch],
    out: W,
) -> Result<(), ArrowError> {
    let mut writer = IpcStreamWriter::try_new(out, schema)?;
    for batch in batches {
        writer.write(batch)?;
    }
    writer.finish()?;

    Ok(())
}

fn write_parquet<W: Write + Send>(
    schema: &SchemaRef,
    batches: &[RecordBatch],
    out: W,
) -> Result<(), ArrowError> {
    let props = WriterProperties::builder()
        .set_compression(Compression::SNAPPY)
        .build();
    let mut writer = ArrowWriter::try_new(out, schema.clone(), Some(props))?;
    for batch in batches {
        writer.write(batch)?;
    }
    writer.close()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use arrow_array::{ArrayRef, Int32Array, StringArray};
    use arrow_schema::{DataType, Field, Schema};
    use std::fs::File;
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

        write_batches_to_file(&batch.schema(), &[batch], &path).unwrap();

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

        write_batches_to_file(&batch.schema(), &[batch], &path).unwrap();

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

        write_batches_to_file(&batch.schema(), std::slice::from_ref(&batch), &path).unwrap();

        // Verify by reading it back
        let file = File::open(&path).unwrap();
        let reader = arrow::ipc::reader::FileReader::try_new(file, None).unwrap();
        let read_batches: Vec<RecordBatch> = reader.map(|r| r.unwrap()).collect();

        assert_eq!(read_batches.len(), 1);
        assert_eq!(read_batches[0].num_rows(), batch.num_rows());
        assert_eq!(read_batches[0].num_columns(), batch.num_columns());
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

        write_batches_to_file(&batch.schema(), std::slice::from_ref(&batch), &path).unwrap();

        // Verify by reading it back as an IPC stream
        let file = File::open(&path).unwrap();
        let reader = arrow::ipc::reader::StreamReader::try_new(file, None).unwrap();
        let read_batches: Vec<RecordBatch> = reader.map(|r| r.unwrap()).collect();

        assert_eq!(read_batches.len(), 1);
        assert_eq!(read_batches[0].num_rows(), batch.num_rows());
        assert_eq!(read_batches[0].num_columns(), batch.num_columns());
    }

    #[test]
    fn test_write_parquet() {
        use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

        let dir = tempdir().unwrap();
        let path = dir.path().join("output.parquet");
        let batch = create_test_batch();

        write_batches_to_file(&batch.schema(), std::slice::from_ref(&batch), &path).unwrap();

        // Verify by reading it back
        let file = File::open(&path).unwrap();
        let builder = ParquetRecordBatchReaderBuilder::try_new(file).unwrap();
        for column in builder.metadata().row_group(0).columns() {
            assert_eq!(column.compression(), Compression::SNAPPY);
        }
        let reader = builder.build().unwrap();
        let read_batches: Vec<RecordBatch> = reader.map(|r| r.unwrap()).collect();

        let total_rows: usize = read_batches.iter().map(|b| b.num_rows()).sum();
        assert_eq!(total_rows, batch.num_rows());
        assert_eq!(read_batches[0].num_columns(), batch.num_columns());
    }

    #[test]
    fn test_write_jsonl() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.jsonl");
        let batch = create_test_batch();

        write_batches_to_file(&batch.schema(), std::slice::from_ref(&batch), &path).unwrap();

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

        write_batches_to_file(&batch1.schema(), &[batch1.clone(), batch2], &path).unwrap();

        let mut file = File::open(&path).unwrap();
        let mut contents = String::new();
        file.read_to_string(&mut contents).unwrap();

        // Should have 6 records total (3 from each batch)
        let count = contents.matches("Alice").count();
        assert_eq!(count, 2);
    }

    fn read_to_string(path: &Path) -> String {
        let mut contents = String::new();
        File::open(path)
            .unwrap()
            .read_to_string(&mut contents)
            .unwrap();
        contents
    }

    fn dir_entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn test_write_zero_rows_json() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.json");
        let schema = create_test_batch().schema();

        write_batches_to_file(&schema, &[], &path).unwrap();

        assert_eq!(read_to_string(&path).trim(), "[]");
    }

    #[test]
    fn test_write_zero_rows_jsonl() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.jsonl");
        let schema = create_test_batch().schema();

        write_batches_to_file(&schema, &[], &path).unwrap();

        assert!(path.exists());
        assert_eq!(read_to_string(&path), "");
    }

    #[test]
    fn test_write_zero_rows_csv() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.csv");
        let schema = create_test_batch().schema();

        write_batches_to_file(&schema, &[], &path).unwrap();

        assert_eq!(read_to_string(&path), "id,name\n");
    }

    #[test]
    fn test_write_zero_rows_arrow_ipc() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.arrow");
        let schema = create_test_batch().schema();

        write_batches_to_file(&schema, &[], &path).unwrap();

        let reader =
            arrow::ipc::reader::FileReader::try_new(File::open(&path).unwrap(), None).unwrap();
        assert_eq!(reader.schema(), schema);
        assert_eq!(reader.count(), 0);
    }

    #[test]
    fn test_write_zero_rows_arrow_stream() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.arrows");
        let schema = create_test_batch().schema();

        write_batches_to_file(&schema, &[], &path).unwrap();

        let reader =
            arrow::ipc::reader::StreamReader::try_new(File::open(&path).unwrap(), None).unwrap();
        assert_eq!(reader.schema(), schema);
        assert_eq!(reader.count(), 0);
    }

    #[test]
    fn test_write_zero_rows_parquet() {
        use parquet::arrow::arrow_reader::ParquetRecordBatchReaderBuilder;

        let dir = tempdir().unwrap();
        let path = dir.path().join("output.parquet");
        let schema = create_test_batch().schema();

        write_batches_to_file(&schema, &[], &path).unwrap();

        let builder = ParquetRecordBatchReaderBuilder::try_new(File::open(&path).unwrap()).unwrap();
        assert_eq!(builder.schema().fields(), schema.fields());
        assert_eq!(builder.metadata().file_metadata().num_rows(), 0);
    }

    #[test]
    fn test_write_zero_rows_replaces_existing_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.csv");
        let batch = create_test_batch();

        write_batches_to_file(&batch.schema(), std::slice::from_ref(&batch), &path).unwrap();
        assert!(read_to_string(&path).contains("Alice"));

        write_batches_to_file(&batch.schema(), &[], &path).unwrap();
        assert_eq!(read_to_string(&path), "id,name\n");
    }

    /// A batch the parquet crate cannot write (Interval(MonthDayNano) is NYI).
    fn create_unwritable_parquet_batch() -> RecordBatch {
        use arrow_array::IntervalMonthDayNanoArray;
        use arrow_array::types::IntervalMonthDayNano;

        let schema = Arc::new(Schema::new(vec![Field::new(
            "iv",
            DataType::Interval(arrow_schema::IntervalUnit::MonthDayNano),
            false,
        )]));
        let array: ArrayRef = Arc::new(IntervalMonthDayNanoArray::from(vec![
            IntervalMonthDayNano::new(0, 1, 0),
        ]));
        RecordBatch::try_new(schema, vec![array]).unwrap()
    }

    #[test]
    fn test_failed_write_preserves_existing_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("keep.parquet");
        let batch = create_test_batch();
        write_batches_to_file(&batch.schema(), std::slice::from_ref(&batch), &path).unwrap();
        let original = std::fs::read(&path).unwrap();

        let bad = create_unwritable_parquet_batch();
        let result = write_batches_to_file(&bad.schema(), &[bad], &path);

        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        // The temporary file is cleaned up.
        assert_eq!(dir_entries(dir.path()), vec!["keep.parquet"]);
    }

    #[test]
    fn test_failed_write_does_not_create_file() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("new.parquet");

        let bad = create_unwritable_parquet_batch();
        assert!(write_batches_to_file(&bad.schema(), &[bad], &path).is_err());
        assert!(dir_entries(dir.path()).is_empty());
    }

    #[test]
    fn test_write_read_only_file_fails_and_preserves_it() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("output.csv");
        let batch = create_test_batch();
        write_batches_to_file(&batch.schema(), std::slice::from_ref(&batch), &path).unwrap();
        let original = std::fs::read(&path).unwrap();
        let mut perms = std::fs::metadata(&path).unwrap().permissions();
        perms.set_readonly(true);
        std::fs::set_permissions(&path, perms).unwrap();
        if std::fs::OpenOptions::new().write(true).open(&path).is_ok() {
            // Running with privileges that bypass permissions (e.g. root).
            return;
        }

        let result = write_batches_to_file(&batch.schema(), &[], &path);

        assert!(result.is_err());
        assert_eq!(std::fs::read(&path).unwrap(), original);
        assert_eq!(dir_entries(dir.path()), vec!["output.csv"]);
    }

    #[cfg(unix)]
    #[test]
    fn test_write_preserves_existing_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let path = dir.path().join("output.csv");
        let batch = create_test_batch();
        write_batches_to_file(&batch.schema(), std::slice::from_ref(&batch), &path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();

        write_batches_to_file(&batch.schema(), std::slice::from_ref(&batch), &path).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o640);
    }

    #[cfg(unix)]
    #[test]
    fn test_write_new_file_matches_file_create_permissions() {
        use std::os::unix::fs::PermissionsExt;

        let dir = tempdir().unwrap();
        let path = dir.path().join("output.csv");
        let reference = dir.path().join("reference");
        File::create(&reference).unwrap();
        let batch = create_test_batch();

        write_batches_to_file(&batch.schema(), std::slice::from_ref(&batch), &path).unwrap();

        // Whatever the umask, the result should match `File::create` rather
        // than tempfile's owner-only default.
        let mode = |p: &Path| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&path), mode(&reference));
    }

    #[cfg(unix)]
    #[test]
    fn test_write_through_symlink_replaces_target() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join("real")).unwrap();
        let target = dir.path().join("real").join("output.csv");
        let link = dir.path().join("link.csv");
        let batch = create_test_batch();
        write_batches_to_file(&batch.schema(), &[], &target).unwrap();
        std::os::unix::fs::symlink(Path::new("real").join("output.csv"), &link).unwrap();

        write_batches_to_file(&batch.schema(), std::slice::from_ref(&batch), &link).unwrap();

        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(read_to_string(&target).contains("Alice"));
        // The temp file was created next to the target, not the link.
        assert_eq!(dir_entries(dir.path()), vec!["link.csv", "real"]);
        assert_eq!(dir_entries(&dir.path().join("real")), vec!["output.csv"]);
    }

    #[cfg(unix)]
    #[test]
    fn test_write_through_dangling_symlink_creates_target() {
        let dir = tempdir().unwrap();
        let target = dir.path().join("target.csv");
        let link = dir.path().join("link.csv");
        std::os::unix::fs::symlink("target.csv", &link).unwrap();
        let batch = create_test_batch();

        write_batches_to_file(&batch.schema(), std::slice::from_ref(&batch), &link).unwrap();

        assert!(
            std::fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(read_to_string(&target).contains("Alice"));
    }

    #[cfg(unix)]
    #[test]
    fn test_write_through_symlink_loop_fails() {
        let dir = tempdir().unwrap();
        let a = dir.path().join("a.csv");
        let b = dir.path().join("b.csv");
        std::os::unix::fs::symlink("b.csv", &a).unwrap();
        std::os::unix::fs::symlink("a.csv", &b).unwrap();
        let batch = create_test_batch();

        let result = write_batches_to_file(&batch.schema(), &[batch], &a);

        assert!(result.is_err());
        assert_eq!(dir_entries(dir.path()), vec!["a.csv", "b.csv"]);
    }

    #[test]
    fn test_temp_dir_for() {
        assert_eq!(temp_dir_for(Path::new("output.json")), Path::new("."));
        assert_eq!(temp_dir_for(Path::new("out/output.json")), Path::new("out"));
        assert_eq!(
            temp_dir_for(Path::new("/tmp/output.json")),
            Path::new("/tmp")
        );
    }
}
