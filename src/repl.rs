// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::highlighter::SyntectHighlighter;
use crate::table::{TableMode, print_batches};
use adbc_core::{Connection, Statement};
use arrow_array::RecordBatch;
use reedline::{DefaultPrompt, DefaultPromptSegment, Reedline, Signal};

pub fn run_repl(mut connection: impl Connection, table_mode: TableMode) {
    let mut line_editor = Reedline::create().with_highlighter(Box::new(SyntectHighlighter::new()));
    let prompt = DefaultPrompt::new(DefaultPromptSegment::Empty, DefaultPromptSegment::Empty);

    loop {
        let signal = line_editor.read_line(&prompt);
        match signal {
            Ok(Signal::Success(buffer)) => {
                if buffer.trim().is_empty() {
                    continue;
                }

                let mut statement = match connection.new_statement() {
                    Ok(statement) => statement,
                    Err(err) => {
                        eprintln!("Failed to create statement: {err}");
                        continue;
                    }
                };

                if let Err(err) = statement.set_sql_query(buffer) {
                    eprintln!("Failed to set SQL query: {err}");
                    continue;
                }

                let reader = match statement.execute() {
                    Ok(reader) => reader,
                    Err(err) => {
                        eprintln!("Failed to execute statement: {err}");
                        continue;
                    }
                };

                let batches: Vec<RecordBatch> = match reader.collect::<Result<_, _>>() {
                    Ok(batches) => batches,
                    Err(err) => {
                        eprintln!("Failed to collect batches: {err}");
                        continue;
                    }
                };
                if let Err(err) = print_batches(&batches, table_mode) {
                    eprintln!("Failed to print batches: {err}");
                }
            }
            Ok(Signal::CtrlD | Signal::CtrlC) => {
                break;
            }
            _ => {}
        }
    }
}
