use crate::database;
use adbc_core::Connection;
use arrow_array::RecordBatch;

pub const HELP: &str = "\
Commands:
  <query>                      Execute a SQL query
  :get-objects, :go [<a.b.c>]  List catalogs, schemas and tables, optionally
                               filtered by an identifier. Filters are ADBC
                               search patterns, where % and _ are wildcards.
  :get-schema, :gs <a.b.c>     Show the columns of a table. The table name
                               must match exactly.
  :help, :h                    Show this message
  :quit, :q                    Exit databow (Ctrl-D also works)";

#[derive(Debug, PartialEq)]
pub enum Command {
    Query(String),
    GetObjects {
        catalog: Option<String>,
        db_schema: Option<String>,
        table: Option<String>,
    },
    GetSchema {
        catalog: Option<String>,
        db_schema: Option<String>,
        table: String,
    },
    Help,
    Quit,
}

pub fn parse(line: &str) -> Result<Command, String> {
    let trimmed = line.trim_end().trim_end_matches(';');

    let Some(rest) = trimmed.trim_start().strip_prefix(':') else {
        return Ok(Command::Query(trimmed.to_string()));
    };

    let (name, argument) = match rest.split_once(char::is_whitespace) {
        Some((name, argument)) => (name, argument.trim()),
        None => (rest, ""),
    };

    if argument.split_whitespace().count() > 1 {
        return Err(format!(
            "Command ':{name}' takes a single identifier, got '{argument}'"
        ));
    }

    match name {
        "help" | "h" => Ok(Command::Help),
        "quit" | "q" => Ok(Command::Quit),
        "get-objects" | "go" => {
            let (catalog, db_schema, table) = parse_identifier(argument)?;
            Ok(Command::GetObjects {
                catalog,
                db_schema,
                table,
            })
        }
        "get-schema" | "gs" => {
            let (catalog, db_schema, table) = parse_identifier(argument)?;
            let Some(table) = table else {
                return Err(
                    "Command ':get-schema' requires a table name, e.g. ':get-schema my_table'"
                        .to_string(),
                );
            };
            Ok(Command::GetSchema {
                catalog,
                db_schema,
                table,
            })
        }
        _ => Err(format!(
            "Unknown command ':{name}'. Type :help for a list of commands."
        )),
    }
}

type Identifier = (Option<String>, Option<String>, Option<String>);

fn parse_identifier(argument: &str) -> Result<Identifier, String> {
    if argument.is_empty() {
        return Ok((None, None, None));
    }

    let parts: Vec<&str> = argument.split('.').collect();
    let (catalog, db_schema, table) = match parts.as_slice() {
        [table] => ("", "", *table),
        [db_schema, table] => ("", *db_schema, *table),
        [catalog, db_schema, table] => (*catalog, *db_schema, *table),
        _ => {
            return Err(format!(
                "Invalid identifier '{argument}': expected [catalog.][schema.]table. Identifiers containing '.' are not supported."
            ));
        }
    };

    Ok((optional(catalog), optional(db_schema), optional(table)))
}

fn optional(part: &str) -> Option<String> {
    if part.is_empty() {
        None
    } else {
        Some(part.to_string())
    }
}

pub fn run(connection: &mut impl Connection, command: Command) -> Result<Vec<RecordBatch>, String> {
    match command {
        Command::Query(sql) => database::execute_query(connection, &sql),
        Command::GetObjects {
            catalog,
            db_schema,
            table,
        } => database::get_objects(
            connection,
            catalog.as_deref(),
            db_schema.as_deref(),
            table.as_deref(),
        ),
        Command::GetSchema {
            catalog,
            db_schema,
            table,
        } => {
            database::get_table_schema(connection, catalog.as_deref(), db_schema.as_deref(), &table)
        }
        Command::Help | Command::Quit => Ok(Vec::new()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn objects(catalog: Option<&str>, db_schema: Option<&str>, table: Option<&str>) -> Command {
        Command::GetObjects {
            catalog: catalog.map(str::to_string),
            db_schema: db_schema.map(str::to_string),
            table: table.map(str::to_string),
        }
    }

    #[test]
    fn test_parse_query() {
        assert_eq!(
            parse("SELECT 1;").unwrap(),
            Command::Query("SELECT 1".to_string())
        );
    }

    #[test]
    fn test_parse_query_with_cast_operator() {
        assert_eq!(
            parse("SELECT 1::int").unwrap(),
            Command::Query("SELECT 1::int".to_string())
        );
    }

    #[test]
    fn test_parse_query_with_leading_whitespace() {
        assert_eq!(
            parse("  SELECT 1;").unwrap(),
            Command::Query("  SELECT 1".to_string())
        );
    }

    #[test]
    fn test_parse_commented_out_command_is_a_query() {
        assert_eq!(
            parse("-- :help").unwrap(),
            Command::Query("-- :help".to_string())
        );
    }

    #[test]
    fn test_parse_help_aliases() {
        assert_eq!(parse(":help").unwrap(), Command::Help);
        assert_eq!(parse(":h").unwrap(), Command::Help);
        assert_eq!(parse("  :help  ").unwrap(), Command::Help);
        assert_eq!(parse(":help;").unwrap(), Command::Help);
    }

    #[test]
    fn test_parse_quit_aliases() {
        assert_eq!(parse(":quit").unwrap(), Command::Quit);
        assert_eq!(parse(":q").unwrap(), Command::Quit);
    }

    #[test]
    fn test_parse_get_objects_without_identifier() {
        assert_eq!(parse(":get-objects").unwrap(), objects(None, None, None));
        assert_eq!(parse(":go").unwrap(), objects(None, None, None));
    }

    #[test]
    fn test_parse_get_objects_identifier_arities() {
        assert_eq!(parse(":go t").unwrap(), objects(None, None, Some("t")));
        assert_eq!(
            parse(":go s.t").unwrap(),
            objects(None, Some("s"), Some("t"))
        );
        assert_eq!(
            parse(":go c.s.t").unwrap(),
            objects(Some("c"), Some("s"), Some("t"))
        );
    }

    #[test]
    fn test_parse_get_objects_empty_segments_are_none() {
        assert_eq!(
            parse(":go .s.t").unwrap(),
            objects(None, Some("s"), Some("t"))
        );
        assert_eq!(
            parse(":go c..t").unwrap(),
            objects(Some("c"), None, Some("t"))
        );
        assert_eq!(parse(":go s.").unwrap(), objects(None, Some("s"), None));
    }

    #[test]
    fn test_parse_get_objects_too_many_parts() {
        let err = parse(":go a.b.c.d").unwrap_err();
        assert!(err.contains("a.b.c.d"), "{err}");
    }

    #[test]
    fn test_parse_get_schema_identifier_arities() {
        assert_eq!(
            parse(":get-schema t").unwrap(),
            Command::GetSchema {
                catalog: None,
                db_schema: None,
                table: "t".to_string(),
            }
        );
        assert_eq!(
            parse(":gs c.s.t").unwrap(),
            Command::GetSchema {
                catalog: Some("c".to_string()),
                db_schema: Some("s".to_string()),
                table: "t".to_string(),
            }
        );
    }

    #[test]
    fn test_parse_get_schema_requires_a_table() {
        assert!(
            parse(":get-schema")
                .unwrap_err()
                .contains("requires a table")
        );
        assert!(parse(":gs").unwrap_err().contains("requires a table"));
        assert!(parse(":gs s.").unwrap_err().contains("requires a table"));
    }

    #[test]
    fn test_parse_unknown_command() {
        let err = parse(":nope").unwrap_err();
        assert!(err.contains(":nope"), "{err}");
        assert!(err.contains(":help"), "{err}");
    }

    #[test]
    fn test_parse_does_not_prefix_match_aliases() {
        assert!(parse(":g").is_err());
        assert!(parse(":get").is_err());
    }

    #[test]
    fn test_parse_rejects_multiple_arguments() {
        let err = parse(":go a b").unwrap_err();
        assert!(err.contains(":go"), "{err}");
    }

    #[test]
    fn test_help_lists_every_command_and_alias() {
        for name in [
            ":get-objects",
            ":go",
            ":get-schema",
            ":gs",
            ":help",
            ":h",
            ":quit",
            ":q",
        ] {
            assert!(HELP.contains(name), "help is missing {name}");
        }
    }
}
