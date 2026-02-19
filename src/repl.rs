// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::database;
use crate::highlighter::SyntectHighlighter;
use crate::table::{TableMode, print_batches};
use adbc_core::Connection;
use reedline::{
    Prompt, PromptEditMode, PromptHistorySearch, Reedline, Signal, ValidationResult, Validator,
};
use std::borrow::Cow;

struct SqlValidator;

impl Validator for SqlValidator {
    fn validate(&self, line: &str) -> ValidationResult {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.ends_with(';') {
            ValidationResult::Complete
        } else {
            ValidationResult::Incomplete
        }
    }
}

struct SqlPrompt;

impl Prompt for SqlPrompt {
    fn render_prompt_left(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_right(&self) -> Cow<'_, str> {
        Cow::Borrowed("")
    }

    fn render_prompt_indicator(&self, _edit_mode: PromptEditMode) -> Cow<'_, str> {
        Cow::Borrowed("> ")
    }

    fn render_prompt_multiline_indicator(&self) -> Cow<'_, str> {
        Cow::Borrowed(". ")
    }

    fn render_prompt_history_search_indicator(
        &self,
        history_search: PromptHistorySearch,
    ) -> Cow<'_, str> {
        Cow::Owned(format!("(search: {}) ", history_search.term))
    }
}

pub fn run_repl(mut connection: impl Connection, table_mode: TableMode) {
    let mut line_editor = Reedline::create()
        .with_highlighter(Box::new(SyntectHighlighter::new()))
        .with_validator(Box::new(SqlValidator));
    let prompt = SqlPrompt;

    loop {
        let signal = line_editor.read_line(&prompt);
        match signal {
            Ok(Signal::Success(buffer)) => {
                if buffer.trim().is_empty() {
                    continue;
                }

                let sql = buffer.trim_end().trim_end_matches(';');
                let batches = match database::execute_query(&mut connection, sql) {
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
