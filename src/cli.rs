// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::output::OutputFormat;
use crate::table::TableMode;
use clap::{Arg, ArgAction, Command, value_parser};
use clap_complete::engine::{
    ArgValueCompleter, CompletionCandidate, PathCompleter, ValueCompleter,
};
use clap_complete::env::Shells;
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::path::{MAIN_SEPARATOR, PathBuf};
use std::process::exit;

#[derive(Debug, Clone)]
pub enum QuerySource {
    Query(String),
    File(PathBuf),
    Stdin,
    Interactive,
}

#[derive(Debug, Clone)]
pub enum ConnectionSource {
    Direct {
        driver_name: Option<String>,
        uri: Option<String>,
        username: Option<String>,
        password: Option<String>,
        options: Vec<(String, String)>,
    },
    Profile {
        profile: String,
        uri: Option<String>,
        username: Option<String>,
        password: Option<String>,
        options: Vec<(String, String)>,
    },
}

pub struct AppConfig {
    pub connection: ConnectionSource,
    pub statement_options: Vec<(String, String)>,
    pub query_source: QuerySource,
    pub table_mode: TableMode,
    pub output_path: Option<PathBuf>,
}

fn is_stdin_piped() -> bool {
    use std::io::IsTerminal;
    !std::io::stdin().is_terminal()
}

pub fn build_command() -> Command {
    let arguments = [
        Arg::new("profile")
            .long("profile")
            .help("Connection profile name or path")
            .add(ArgValueCompleter::new(complete_profile))
            .conflicts_with("driver"),
        Arg::new("driver")
            .long("driver")
            .help("Driver name (inferred from --uri scheme or profile if not specified)"),
        Arg::new("uri")
            .long("uri")
            .help("Database uniform resource identifier"),
        Arg::new("username")
            .long("username")
            .help("Database user username"),
        Arg::new("password")
            .long("password")
            .help("Database user password"),
        Arg::new("option")
            .long("option")
            .value_name("key=value")
            .help("Driver-specific database option")
            .action(ArgAction::Append),
        Arg::new("statement-option")
            .long("statement-option")
            .value_name("key=value")
            .help("Driver-specific statement option")
            .action(ArgAction::Append),
        Arg::new("mode")
            .long("mode")
            .help("Table display style")
            .value_parser(value_parser!(TableMode))
            .default_value("utf8-compact")
            .hide_possible_values(true),
        Arg::new("query")
            .long("query")
            .help("Execute query and exit")
            .conflicts_with("file"),
        Arg::new("file")
            .long("file")
            .help("Read and execute file and exit")
            .value_parser(value_parser!(PathBuf))
            .conflicts_with("query"),
        Arg::new("output")
            .long("output")
            .help("Write result to file")
            .value_name("file")
            .value_parser(parse_output_path),
    ];
    let generate_shell_completion = Command::new("generate-shell-completion")
        .about("Generate shell completion")
        .alias("--generate-shell-completion")
        .hide(true)
        .arg(
            Arg::new("shell")
                .help("The shell to generate the completion script for")
                .required(true)
                .value_parser(["bash", "zsh", "fish", "powershell"]),
        );
    Command::new("databow")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Query databases via ADBC")
        .arg_required_else_help(true)
        .args(arguments)
        .subcommand(generate_shell_completion)
        .disable_help_subcommand(true)
}

pub fn parse_args() -> AppConfig {
    let matches = build_command().get_matches();

    if let Some(("generate-shell-completion", sub_matches)) = matches.subcommand() {
        let shell = sub_matches
            .get_one::<String>("shell")
            .expect("shell is required");
        print_completion_registration(shell);
        exit(0);
    }

    let profile = matches.get_one::<String>("profile").cloned();
    let driver_name = matches.get_one::<String>("driver").cloned();
    let uri = matches.get_one::<String>("uri").cloned();
    let username = matches.get_one::<String>("username").cloned();
    let password = matches.get_one::<String>("password").cloned();

    let options = collect_options(&matches, "option");
    let statement_options = collect_options(&matches, "statement-option");

    // Build ConnectionSource based on whether --profile or --driver was provided
    let connection = if let Some(profile) = profile {
        ConnectionSource::Profile {
            profile,
            uri,
            username,
            password,
            options,
        }
    } else if let Some(driver_name) = driver_name {
        ConnectionSource::Direct {
            driver_name: Some(driver_name),
            uri,
            username,
            password,
            options,
        }
    } else if uri.as_deref().is_some_and(uri_has_driver_scheme) {
        // The driver can be inferred from the URI scheme (e.g. `--uri duckdb://...`
        // implies the `duckdb` driver), so neither --driver nor --profile is
        // required. The raw URI is handed to `ManagedDatabase::from_uri`, which
        // performs the scheme parsing and driver selection.
        ConnectionSource::Direct {
            driver_name: None,
            uri,
            username,
            password,
            options,
        }
    } else {
        eprintln!(
            "Error: Provide --driver, --profile, or a --uri with a scheme (e.g. duckdb://...)"
        );
        exit(1);
    };

    let query_source = if let Some(query) = matches.get_one::<String>("query") {
        QuerySource::Query(query.clone())
    } else if let Some(file) = matches.get_one::<PathBuf>("file") {
        QuerySource::File(file.clone())
    } else if is_stdin_piped() {
        QuerySource::Stdin
    } else {
        QuerySource::Interactive
    };

    let table_mode = matches
        .get_one::<TableMode>("mode")
        .copied()
        .unwrap_or_default();

    let output_path = matches.get_one::<PathBuf>("output").cloned();
    if output_path.is_some() && matches!(query_source, QuerySource::Interactive) {
        eprintln!("Error: --output cannot be used in interactive mode");
        exit(1);
    }

    AppConfig {
        connection,
        statement_options,
        query_source,
        table_mode,
        output_path,
    }
}

/// Parse all `key=value` values supplied for the argument `id`, exiting with an
/// error message if any value is malformed.
fn collect_options(matches: &clap::ArgMatches, id: &str) -> Vec<(String, String)> {
    let Some(values) = matches.get_many::<String>(id) else {
        return Vec::new();
    };
    values
        .map(|option| {
            parse_option(option).unwrap_or_else(|err| {
                eprintln!("{err}");
                exit(1);
            })
        })
        .collect()
}

/// Print the script that registers dynamic completions for `shell`.
///
/// The script calls back into databow with `COMPLETE=<shell>` set, which is
/// handled by `clap_complete::CompleteEnv` in `main`.
fn print_completion_registration(shell: &str) {
    let shells = Shells::builtins();
    let completer = shells
        .completer(shell)
        .expect("shell is validated by the value parser");
    // Like `CompleteEnv`, call back into the binary the way it was invoked: a
    // bare name is resolved via PATH at completion time, a relative path is
    // made absolute.
    let mut bin = std::env::args_os()
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("databow"));
    if bin.components().count() > 1
        && let Ok(current_dir) = std::env::current_dir()
    {
        bin = current_dir.join(bin);
    }
    let mut stdout = std::io::stdout();
    completer
        .write_registration(
            "COMPLETE",
            "databow",
            "databow",
            &bin.to_string_lossy(),
            &mut stdout,
        )
        .unwrap_or_else(|e| {
            eprintln!("Failed to write completion script: {e}");
            exit(1);
        });
}

/// Complete `--profile` with the names of profiles found in the ADBC profile
/// search locations, falling back to file paths when the input looks like one.
fn complete_profile(current: &OsStr) -> Vec<CompletionCandidate> {
    let current_str = current.to_string_lossy();
    if current_str.starts_with('.') || current_str.contains(['/', MAIN_SEPARATOR]) {
        return PathCompleter::file().complete(current);
    }
    list_profiles(&profile_search_paths())
        .into_iter()
        .filter(|name| name.starts_with(current_str.as_ref()))
        .map(CompletionCandidate::new)
        .collect()
}

/// Directories searched for connection profiles, in priority order.
///
/// Mirrors the private `get_profile_search_paths` in `adbc_driver_manager`.
fn profile_search_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();
    if let Some(env_paths) = std::env::var_os("ADBC_PROFILE_PATH") {
        paths.extend(std::env::split_paths(&env_paths));
    }

    #[cfg(target_os = "windows")]
    let user_dir = std::env::var_os("LOCALAPPDATA")
        .map(|dir| PathBuf::from(dir).join("ADBC").join("Profiles"));
    #[cfg(target_os = "macos")]
    let user_dir = std::env::var_os("HOME").map(|home| {
        PathBuf::from(home)
            .join("Library")
            .join("Application Support")
            .join("ADBC")
            .join("Profiles")
    });
    #[cfg(all(unix, not(target_os = "macos")))]
    let user_dir = std::env::var_os("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|home| PathBuf::from(home).join(".config")))
        .map(|dir| dir.join("adbc").join("profiles"));

    paths.extend(user_dir);
    paths
}

/// Names (file stems) of the `.toml` profiles in `dirs`, sorted and deduplicated.
fn list_profiles(dirs: &[PathBuf]) -> BTreeSet<String> {
    dirs.iter()
        .filter_map(|dir| std::fs::read_dir(dir).ok())
        .flatten()
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| path.is_file() && path.extension().is_some_and(|ext| ext == "toml"))
        .filter_map(|path| path.file_stem()?.to_str().map(str::to_string))
        .collect()
}

fn parse_output_path(s: &str) -> Result<PathBuf, String> {
    let path = PathBuf::from(s);
    OutputFormat::from_path(&path)?;
    Ok(path)
}

fn uri_has_driver_scheme(uri: &str) -> bool {
    let Some(idx) = uri.find(':') else {
        return false;
    };
    idx != 0
}

fn parse_option(option: &str) -> Result<(String, String), String> {
    let parts: Vec<&str> = option.splitn(2, '=').collect();
    if parts.len() != 2 {
        return Err(format!(
            "Invalid option format (expected key=value): {option}"
        ));
    }
    let key = parts[0];
    if key.is_empty() {
        return Err(format!("Option key cannot be empty: {option}"));
    }
    Ok((key.to_string(), parts[1].to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_profiles() {
        let first = tempfile::tempdir().unwrap();
        let second = tempfile::tempdir().unwrap();
        std::fs::write(first.path().join("warehouse.toml"), "").unwrap();
        std::fs::write(first.path().join("notes.txt"), "").unwrap();
        std::fs::create_dir(first.path().join("subdir.toml")).unwrap();
        std::fs::write(second.path().join("warehouse.toml"), "").unwrap();
        std::fs::write(second.path().join("analytics.toml"), "").unwrap();

        let dirs = vec![
            first.path().to_path_buf(),
            second.path().to_path_buf(),
            PathBuf::from("/nonexistent/databow/profiles"),
        ];
        let names: Vec<String> = list_profiles(&dirs).into_iter().collect();
        assert_eq!(names, vec!["analytics", "warehouse"]);
    }

    #[test]
    fn test_parse_output_path_valid() {
        assert_eq!(
            parse_output_path("out.parquet").unwrap(),
            PathBuf::from("out.parquet")
        );
    }

    #[test]
    fn test_parse_output_path_unsupported_extension() {
        let err = parse_output_path("out.xyz").unwrap_err();
        assert!(err.contains("Unsupported file extension"));
    }

    #[test]
    fn test_parse_output_path_no_extension() {
        let err = parse_output_path("out").unwrap_err();
        assert!(err.contains("no file extension"));
    }

    #[test]
    fn test_connection_source_direct() {
        let source = ConnectionSource::Direct {
            driver_name: Some("duckdb".to_string()),
            uri: Some(":memory:".to_string()),
            username: None,
            password: None,
            options: vec![],
        };
        match source {
            ConnectionSource::Direct { driver_name, .. } => {
                assert_eq!(driver_name, Some("duckdb".to_string()));
            }
            _ => panic!("Expected Direct variant"),
        }
    }

    #[test]
    fn test_connection_source_profile() {
        let source = ConnectionSource::Profile {
            profile: "my_profile".to_string(),
            uri: Some("override_uri".to_string()),
            username: None,
            password: None,
            options: vec![],
        };
        match source {
            ConnectionSource::Profile { profile, uri, .. } => {
                assert_eq!(profile, "my_profile");
                assert_eq!(uri, Some("override_uri".to_string()));
            }
            _ => panic!("Expected Profile variant"),
        }
    }

    #[test]
    fn test_uri_has_driver_scheme_with_scheme() {
        assert!(uri_has_driver_scheme("duckdb://:memory:"));
        assert!(uri_has_driver_scheme("postgresql://host:5432/db"));
        assert!(uri_has_driver_scheme("sqlite:file::memory:"));
        assert!(uri_has_driver_scheme("sqlite::memory:"));
        // Bare `driver:` form: accepted, matching `parse_driver_uri`.
        assert!(uri_has_driver_scheme("duckdb:"));
    }

    #[test]
    fn test_uri_has_driver_scheme_arbitrary_scheme() {
        assert!(uri_has_driver_scheme("DuckDB://x"));
    }

    #[test]
    fn test_uri_has_driver_scheme_no_scheme() {
        // No colon at all.
        assert!(!uri_has_driver_scheme("plain_path"));
        // Empty scheme (leading colon).
        assert!(!uri_has_driver_scheme(":memory:"));
    }

    #[test]
    fn test_uri_has_driver_scheme_accepts_profile_scheme() {
        // `profile://` is a valid scheme; `from_uri` resolves it to a profile.
        assert!(uri_has_driver_scheme("profile://my_database"));
        assert!(uri_has_driver_scheme("PROFILE://my_database"));
    }

    #[test]
    fn test_connection_source_profile_with_path() {
        let source = ConnectionSource::Profile {
            profile: "/path/to/profile.toml".to_string(),
            uri: None,
            username: None,
            password: None,
            options: vec![("key".to_string(), "value".to_string())],
        };
        match source {
            ConnectionSource::Profile {
                profile, options, ..
            } => {
                assert_eq!(profile, "/path/to/profile.toml");
                assert_eq!(options.len(), 1);
            }
            _ => panic!("Expected Profile variant"),
        }
    }

    #[test]
    fn test_parse_option_valid() {
        let result = parse_option("key=value");
        assert_eq!(result, Ok(("key".to_string(), "value".to_string())));
    }

    #[test]
    fn test_parse_option_with_equals_in_value() {
        let result = parse_option("key=val=ue");
        assert_eq!(result, Ok(("key".to_string(), "val=ue".to_string())));
    }

    #[test]
    fn test_parse_option_no_equals() {
        let result = parse_option("keyvalue");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid option format"));
    }

    #[test]
    fn test_parse_option_empty_value() {
        let result = parse_option("key=");
        assert_eq!(result, Ok(("key".to_string(), "".to_string())));
    }

    #[test]
    fn test_parse_option_empty_key() {
        let result = parse_option("=value");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("key cannot be empty"));
    }

    #[test]
    fn test_app_config_with_direct_connection() {
        let config = AppConfig {
            connection: ConnectionSource::Direct {
                driver_name: Some("test_driver".to_string()),
                uri: Some("test_uri".to_string()),
                username: Some("test_user".to_string()),
                password: Some("test_pass".to_string()),
                options: vec![("key1".to_string(), "val1".to_string())],
            },
            statement_options: vec![],
            query_source: QuerySource::Interactive,
            table_mode: TableMode::default(),
            output_path: None,
        };

        match &config.connection {
            ConnectionSource::Direct {
                driver_name,
                uri,
                username,
                password,
                options,
            } => {
                assert_eq!(driver_name, &Some("test_driver".to_string()));
                assert_eq!(*uri, Some("test_uri".to_string()));
                assert_eq!(*username, Some("test_user".to_string()));
                assert_eq!(*password, Some("test_pass".to_string()));
                assert_eq!(options.len(), 1);
                assert_eq!(options[0], ("key1".to_string(), "val1".to_string()));
            }
            _ => panic!("Expected Direct variant"),
        }
    }

    #[test]
    fn test_app_config_with_profile_connection() {
        let config = AppConfig {
            connection: ConnectionSource::Profile {
                profile: "my_postgres".to_string(),
                uri: None,
                username: None,
                password: None,
                options: vec![],
            },
            statement_options: vec![],
            query_source: QuerySource::Interactive,
            table_mode: TableMode::AsciiMarkdown,
            output_path: None,
        };

        match &config.connection {
            ConnectionSource::Profile { profile, .. } => {
                assert_eq!(profile, "my_postgres");
            }
            _ => panic!("Expected Profile variant"),
        }
        assert_eq!(config.table_mode, TableMode::AsciiMarkdown);
    }

    #[test]
    fn test_app_config_with_profile_and_overrides() {
        let config = AppConfig {
            connection: ConnectionSource::Profile {
                profile: "/path/to/config.toml".to_string(),
                uri: Some("override://uri".to_string()),
                username: Some("override_user".to_string()),
                password: None,
                options: vec![("extra".to_string(), "option".to_string())],
            },
            statement_options: vec![],
            query_source: QuerySource::Query("SELECT 1".to_string()),
            table_mode: TableMode::default(),
            output_path: Some(PathBuf::from("output.json")),
        };

        match &config.connection {
            ConnectionSource::Profile {
                profile,
                uri,
                username,
                options,
                ..
            } => {
                assert_eq!(profile, "/path/to/config.toml");
                assert_eq!(*uri, Some("override://uri".to_string()));
                assert_eq!(*username, Some("override_user".to_string()));
                assert_eq!(options.len(), 1);
            }
            _ => panic!("Expected Profile variant"),
        }
        assert_eq!(config.output_path, Some(PathBuf::from("output.json")));
    }
}
