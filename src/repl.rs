// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::database;
use crate::highlighter::SyntectHighlighter;
use crate::table::{TableMode, print_batches};
use adbc_core::Connection;
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

                let batches = match database::execute_query(&mut connection, &buffer) {
                    Ok(batches) => batches,
                    Err(err) => {
                        eprintln!("{err}");
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
