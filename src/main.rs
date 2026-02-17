// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

mod cli;
mod database;
mod highlighter;
mod output;
mod repl;
mod table;

use cli::{QuerySource, parse_args};
use std::io::Read;
use std::process::exit;

fn main() {
    let config = parse_args();
    let query_source = config.query_source.clone();
    let table_mode = config.table_mode;
    let output_path = config.output_path.clone();

    if matches!(query_source, QuerySource::Interactive) {
        let connection = database::initialize_connection(config);
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

    let mut connection = database::initialize_connection(config);
    let batches = database::execute_query(&mut connection, &sql).unwrap_or_else(|e| {
        eprintln!("{e}");
        exit(1);
    });
    if let Err(e) = output_results(&batches, table_mode, output_path.as_deref()) {
        eprintln!("{e}");
        exit(1);
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
