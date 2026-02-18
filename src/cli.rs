// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::table::TableMode;
use clap::{Arg, ArgAction, Command, value_parser};
use std::path::PathBuf;
use std::process::exit;

#[derive(Debug, Clone)]
pub enum QuerySource {
    Query(String),
    File(PathBuf),
    Stdin,
    Interactive,
}

pub struct DatabaseConfig {
    pub driver_name: String,
    pub uri: Option<String>,
    pub username: Option<String>,
    pub password: Option<String>,
    pub options: Vec<(String, String)>,
    pub query_source: QuerySource,
    pub table_mode: TableMode,
    pub output_path: Option<PathBuf>,
}

fn is_stdin_piped() -> bool {
    use std::io::IsTerminal;
    !std::io::stdin().is_terminal()
}

pub fn parse_args() -> DatabaseConfig {
    let arguments = [
        Arg::new("driver")
            .long("driver")
            .help("Driver name")
            .required(true),
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
            .help("Driver-specific database option")
            .action(ArgAction::Append),
        Arg::new("mode")
            .long("mode")
            .help("Table display style")
            .default_value("utf8_compact"),
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
            .value_name("file"),
    ];
    let command = Command::new("adbcli")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Query databases via ADBC")
        .arg_required_else_help(true)
        .args(arguments);
    let matches = command.get_matches();

    let driver_name = if let Some(name) = matches.get_one::<String>("driver") {
        name.clone()
    } else {
        eprintln!("Driver name is required");
        exit(1);
    };

    let uri = matches.get_one::<String>("uri").cloned();
    let username = matches.get_one::<String>("username").cloned();
    let password = matches.get_one::<String>("password").cloned();

    let mut options = Vec::new();
    if let Some(option_values) = matches.get_many::<String>("option") {
        for option in option_values {
            match parse_option(option) {
                Ok((key, value)) => options.push((key, value)),
                Err(err) => {
                    eprintln!("{err}");
                    exit(1);
                }
            }
        }
    }

    let query_source = if let Some(query) = matches.get_one::<String>("query") {
        QuerySource::Query(query.clone())
    } else if let Some(file) = matches.get_one::<PathBuf>("file") {
        QuerySource::File(file.clone())
    } else if is_stdin_piped() {
        QuerySource::Stdin
    } else {
        QuerySource::Interactive
    };

    let table_mode = match matches.get_one::<String>("mode") {
        Some(mode_str) => match mode_str.parse::<TableMode>() {
            Ok(mode) => mode,
            Err(err) => {
                eprintln!("{err}");
                exit(1);
            }
        },
        None => TableMode::default(),
    };

    let output_path = matches.get_one::<String>("output").map(PathBuf::from);
    if output_path.is_some() && matches!(query_source, QuerySource::Interactive) {
        eprintln!("Error: --output cannot be used in interactive mode");
        exit(1);
    }

    DatabaseConfig {
        driver_name,
        uri,
        username,
        password,
        options,
        query_source,
        table_mode,
        output_path,
    }
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
    fn test_database_config_creation() {
        let config = DatabaseConfig {
            driver_name: "test_driver".to_string(),
            uri: Some("test_uri".to_string()),
            username: Some("test_user".to_string()),
            password: Some("test_pass".to_string()),
            options: vec![("key1".to_string(), "val1".to_string())],
            query_source: QuerySource::Interactive,
            table_mode: TableMode::default(),
            output_path: None,
        };

        assert_eq!(config.driver_name, "test_driver");
        assert_eq!(config.uri, Some("test_uri".to_string()));
        assert_eq!(config.username, Some("test_user".to_string()));
        assert_eq!(config.password, Some("test_pass".to_string()));
        assert_eq!(config.options.len(), 1);
        assert_eq!(config.options[0], ("key1".to_string(), "val1".to_string()));
    }

    #[test]
    fn test_database_config_with_none_fields() {
        let config = DatabaseConfig {
            driver_name: "test_driver".to_string(),
            uri: None,
            username: None,
            password: None,
            options: vec![],
            query_source: QuerySource::Interactive,
            table_mode: TableMode::AsciiMarkdown,
            output_path: None,
        };

        assert_eq!(config.driver_name, "test_driver");
        assert_eq!(config.uri, None);
        assert_eq!(config.username, None);
        assert_eq!(config.password, None);
        assert!(config.options.is_empty());
        assert_eq!(config.table_mode, TableMode::AsciiMarkdown);
    }

    #[test]
    fn test_database_config_with_output_path() {
        let config = DatabaseConfig {
            driver_name: "test_driver".to_string(),
            uri: None,
            username: None,
            password: None,
            options: vec![],
            query_source: QuerySource::Query("SELECT 1".to_string()),
            table_mode: TableMode::default(),
            output_path: Some(PathBuf::from("output.json")),
        };

        assert_eq!(config.output_path, Some(PathBuf::from("output.json")));
    }
}
