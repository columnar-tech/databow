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
fn test_driver_inferred_from_uri_scheme() {
    // No --driver or --profile: the driver should be inferred from the
    // `sqlite:` URI scheme.
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "--uri",
            "sqlite::memory:",
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
fn test_no_driver_no_scheme_uri_errors() {
    // A URI with no scheme and no --driver/--profile should still error.
    let output = Command::new("cargo")
        .args(["run", "--", "--uri", "plain_path", "--query", "SELECT 1"])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--driver"));
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

#[test]
fn test_profile_and_driver_mutually_exclusive() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "--profile",
            "test_profile",
            "--driver",
            "duckdb",
            "--query",
            "SELECT 1",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot be used with") || stderr.contains("conflict"));
}

#[test]
fn test_profile_not_found_error() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "--profile",
            "nonexistent_profile_xyz123",
            "--query",
            "SELECT 1",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Failed to load profile") || stderr.contains("not found"),
        "Expected profile not found error, got: {}",
        stderr
    );
}

#[test]
fn test_profile_from_file_path() {
    use std::io::Write;

    // Create a temporary profile file
    let mut temp_file = NamedTempFile::with_suffix(".toml").expect("Failed to create temp file");
    let profile_path = temp_file.path().to_string_lossy().to_string();

    // Write a valid profile
    temp_file
        .write_all(
            br#"profile_version = 1
driver = "duckdb"

[Options]
"#,
        )
        .expect("Failed to write to temp file");

    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "--profile",
            &profile_path,
            "--query",
            "SELECT 42 AS answer",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Profile from file should work. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("answer"));
    assert!(stdout.contains("42"));
}

#[test]
fn test_uri_profile_scheme_from_file_path() {
    use std::io::Write;

    // A `profile://<path>` URI (no --driver/--profile) should be resolved by
    // the driver manager's `from_uri`, which loads the referenced profile.
    let mut temp_file = NamedTempFile::with_suffix(".toml").expect("Failed to create temp file");
    let profile_path = temp_file.path().to_string_lossy().to_string();

    temp_file
        .write_all(
            br#"profile_version = 1
driver = "duckdb"

[Options]
"#,
        )
        .expect("Failed to write to temp file");

    let uri = format!("profile://{profile_path}");
    let output = Command::new("cargo")
        .args(["run", "--", "--uri", &uri, "--query", "SELECT 42 AS answer"])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "profile:// URI should resolve the profile. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("answer"));
    assert!(stdout.contains("42"));
}

#[test]
fn test_profile_with_uri_override() {
    use std::io::Write;

    // Create a profile that doesn't specify a URI
    let mut temp_file = NamedTempFile::with_suffix(".toml").expect("Failed to create temp file");
    let profile_path = temp_file.path().to_string_lossy().to_string();

    temp_file
        .write_all(
            br#"profile_version = 1
driver = "duckdb"

[Options]
"#,
        )
        .expect("Failed to write to temp file");

    // Use --uri to override and specify in-memory database
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "--profile",
            &profile_path,
            "--uri",
            ":memory:",
            "--query",
            "SELECT 'overridden' AS source",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Profile with URI override should work. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("source"));
    assert!(stdout.contains("overridden"));
}

#[test]
fn test_profile_with_custom_options() {
    use std::io::Write;

    // Create a profile with custom DuckDB options
    let mut temp_file = NamedTempFile::with_suffix(".toml").expect("Failed to create temp file");
    let profile_path = temp_file.path().to_string_lossy().to_string();

    temp_file
        .write_all(
            br#"profile_version = 1
driver = "duckdb"

[Options]
uri = ":memory:"
"#,
        )
        .expect("Failed to write to temp file");

    // Just verify a profile can be loaded and a query executed
    // (option handling varies by driver, so we don't test specific options)
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "--profile",
            &profile_path,
            "--query",
            "SELECT 'profile_options_test' AS result",
        ])
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "Profile with options should work. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("profile_options_test"));
}

#[test]
fn test_profile_env_var_substitution() {
    use std::io::Write;

    // Profile options containing `{{ env_var(NAME) }}` must be expanded against
    // the process environment when the profile is loaded (per the ADBC connection-profile spec).
    let tmp_dir = tempfile::tempdir().expect("create tempdir");
    let canary = "databow_env_var_substitution_canary";
    let expanded_db_path = tmp_dir.path().join(format!("{canary}.duckdb"));
    let literal_db_path = tmp_dir
        .path()
        .join("{{ env_var(DATABOW_TEST_CANARY) }}.duckdb");

    let mut profile_file = NamedTempFile::with_suffix(".toml").expect("create profile temp file");
    let profile_path = profile_file.path().to_string_lossy().to_string();
    let uri_template = format!(
        "{}/{{{{ env_var(DATABOW_TEST_CANARY) }}}}.duckdb",
        tmp_dir.path().display()
    );
    let profile_contents = format!(
        "profile_version = 1\ndriver = \"duckdb\"\n\n[Options]\nuri = \"{uri_template}\"\n"
    );
    profile_file
        .write_all(profile_contents.as_bytes())
        .expect("write profile");

    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "--profile",
            &profile_path,
            "--query",
            "SELECT 'env_var_substituted' AS result",
        ])
        .env("DATABOW_TEST_CANARY", canary)
        .output()
        .expect("Failed to execute command");

    assert!(
        output.status.success(),
        "expected query to succeed after env_var substitution. stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        expanded_db_path.exists(),
        "expected DuckDB to have created the substituted path {:?}, but it does not exist; profile substitution did not occur",
        expanded_db_path
    );
    assert!(
        !literal_db_path.exists(),
        "unexpected: DuckDB created the literal-template path {:?}, which means env_var substitution did not happen",
        literal_db_path
    );
}

#[test]
fn test_timestamp_with_time_zone() {
    let output = Command::new("cargo")
        .args([
            "run",
            "--",
            "--driver",
            "duckdb",
            "--query",
            "SET TimeZone = 'America/Los_Angeles'; SELECT TIMESTAMPTZ '1992-09-20 12:30:00.123456789+01:00'",
        ])
        .output()
        .expect("Failed to execute command");

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "TIMESTAMPTZ query should succeed. stderr: {}",
        stderr
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("1992-09-20T04:30:00.123456-07:00"),
        "expected session-zone-rendered TIMESTAMPTZ in output. stdout: {}",
        stdout
    );
}
