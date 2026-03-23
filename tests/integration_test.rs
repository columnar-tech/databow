// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

use std::process::Command;
use tempfile::NamedTempFile;

#[test]
fn test_cli_help() {
    let output = Command::new("cargo")
        .args(["run", "--", "--help"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("databow"));
    assert!(stdout.contains("-h, --help"));
}

#[test]
fn test_cli_version() {
    let output = Command::new("cargo")
        .args(["run", "--", "--version"])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn test_cli_requires_driver() {
    let output = Command::new("cargo")
        .args(["run"])
        .output()
        .expect("Failed to execute command");

    // Should fail without required driver argument
    assert!(!output.status.success());
}

#[test]
fn test_query_argument() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "--driver",
            "duckdb",
            "--query",
            "SELECT 42 AS answer",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("answer"));
    assert!(stdout.contains("42"));
}

#[test]
fn test_file_argument() {
    // Create a temporary SQL file
    let mut temp_file = NamedTempFile::new().expect("Failed to create temp file");
    let file_path = temp_file.path().to_string_lossy().to_string();

    // Write SQL query to file
    use std::io::Write;
    temp_file
        .write_all(b"SELECT 99 AS result;")
        .expect("Failed to write to temp file");

    let output = Command::new("cargo")
        .args(["run", "--", "--driver", "duckdb", "--file", &file_path])
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("result"));
    assert!(stdout.contains("99"));
}

#[test]
fn test_stdin_piping() {
    let output = Command::new("bash")
        .arg("-c")
        .arg("echo 'SELECT 77 AS value;' | cargo run -- --driver duckdb")
        .output()
        .expect("Failed to execute command");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("value"));
    assert!(stdout.contains("77"));
}

#[test]
fn test_file_not_found_error() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "--driver",
            "duckdb",
            "--file",
            "/nonexistent/file.sql",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Failed to read file") || stderr.contains("No such file"));
}

#[test]
fn test_invalid_sql_error() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "--driver",
            "duckdb",
            "--query",
            "INVALID SQL SYNTAX",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("Failed") || stderr.contains("error"));
}

#[test]
fn test_conflicting_query_and_file_arguments() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "--driver",
            "duckdb",
            "--query",
            "SELECT 1",
            "--file",
            "/tmp/test.sql",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot be used with") || stderr.contains("conflict"));
}
