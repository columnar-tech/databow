// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

mod cli;
mod command;
mod database;
mod highlighter;
mod output;
mod repl;
mod table;

use cli::{AppConfig, QuerySource, parse_args};
use std::io::Read;
use std::process::exit;

fn main() {
    let AppConfig {
        connection,
        query_source,
        table_mode,
        output_path,
    } = parse_args();

    if matches!(query_source, QuerySource::Interactive) {
        let connection = database::initialize_connection(connection).unwrap_or_else(|e| {
            eprintln!("{e}");
            exit(1);
        });
        repl::run_repl(connection, table_mode);
        return;
    }

    let sql = match query_source {
        QuerySource::Query(sql) => sql,
        QuerySource::File(path) => std::fs::read_to_string(&path).unwrap_or_else(|e| {
            eprintln!("Failed to read file {}: {e}", path.display());
            exit(1);
        }),
        QuerySource::Stdin => {
            let mut sql = String::new();
            std::io::stdin()
                .read_to_string(&mut sql)
                .unwrap_or_else(|e| {
                    eprintln!("Failed to read from stdin: {e}");
                    exit(1);
                });
            sql
        }
        QuerySource::Interactive => unreachable!(),
    };

    let trimmed = sql.trim();
    if trimmed.starts_with(':') && trimmed.contains('\n') {
        eprintln!("Error: commands cannot be combined with SQL in --file or stdin input");
        exit(1);
    }

    let parsed = command::parse(&sql).unwrap_or_else(|e| {
        eprintln!("{e}");
        exit(1);
    });

    match parsed {
        command::Command::Help => {
            println!("{}", command::HELP);
            if output_path.is_some() {
                eprintln!("No output file was written: ':help' produces no results");
            }
        }
        command::Command::Quit => {}
        parsed => {
            let mut connection = database::initialize_connection(connection).unwrap_or_else(|e| {
                eprintln!("{e}");
                exit(1);
            });
            let batches = command::run(&mut connection, parsed).unwrap_or_else(|e| {
                eprintln!("{e}");
                exit(1);
            });
            if let Err(e) = output_results(&batches, table_mode, output_path.as_deref()) {
                eprintln!("{e}");
                exit(1);
            }
        }
    }
}

fn output_results(
    batches: &[arrow_array::RecordBatch],
    table_mode: table::TableMode,
    output_path: Option<&std::path::Path>,
) -> Result<(), String> {
    if let Some(path) = output_path {
        output::write_batches_to_file(batches, path)
            .map_err(|e| format!("Failed to write output file: {e}"))
    } else {
        table::print_batches(batches, table_mode)
            .map_err(|e| format!("Failed to print batches: {e}"))
    }
}
