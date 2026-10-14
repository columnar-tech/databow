// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

mod cli;
mod commands;
mod database;
mod highlighter;
mod output;
mod pager;
mod repl;
mod table;
mod truncate;

use cli::{AppConfig, QuerySource, parse_args};
use std::io::Read;
use std::process::exit;

fn main() {
    let AppConfig {
        connection,
        statement_options,
        query_source,
        table_mode,
        output_path,
    } = parse_args();
    let statement_options = database::build_statement_options(statement_options);

    if matches!(query_source, QuerySource::Interactive) {
        let connection = database::initialize_connection(connection).unwrap_or_else(|e| {
            eprintln!("{e}");
            exit(1);
        });
        repl::run_repl(
            connection,
            table_mode,
            table::TruncationOptions::default(),
            &statement_options,
        );
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

    // A blank query has no result schema, so there is nothing meaningful to
    // write; fail rather than silently producing an empty or garbage file.
    if output_path.is_some() && sql.trim().is_empty() {
        eprintln!("No query to execute; refusing to write an empty output file");
        exit(1);
    }

    let mut connection = database::initialize_connection(connection).unwrap_or_else(|e| {
        eprintln!("{e}");
        exit(1);
    });
    let result =
        database::execute_query(&mut connection, &sql, &statement_options).unwrap_or_else(|e| {
            eprintln!("{e}");
            exit(1);
        });
    if let Err(e) = output_results(&result, table_mode, output_path.as_deref()) {
        eprintln!("{e}");
        exit(1);
    }
}

fn output_results(
    result: &database::QueryResult,
    table_mode: table::TableMode,
    output_path: Option<&std::path::Path>,
) -> Result<(), String> {
    if let Some(path) = output_path {
        output::write_batches_to_file(&result.schema, &result.batches, path)
            .map_err(|e| format!("Failed to write output file: {e}"))
    } else {
        table::print_batches(
            &result.batches,
            table::RenderOptions {
                mode: table_mode,
                ..Default::default()
            },
        )
        .map_err(|e| format!("Failed to print batches: {e}"))
    }
}
