// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::commands::{self, Outcome, ReplState};
use crate::database;
use crate::highlighter::SyntectHighlighter;
use crate::pager::PagerSettings;
use crate::table::{RenderOptions, TableMode, TruncationOptions};
use adbc_core::Connection;
use reedline::{
    Prompt, PromptEditMode, PromptHistorySearch, Reedline, Signal, ValidationResult, Validator,
};
use std::borrow::Cow;

struct SqlValidator;

impl Validator for SqlValidator {
    fn validate(&self, line: &str) -> ValidationResult {
        let trimmed = line.trim();
        // Dot commands are a single line and need no terminating semicolon.
        if trimmed.is_empty() || trimmed.ends_with(';') || commands::is_command(trimmed) {
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

/// Build the REPL startup banner. The first line always shows `databow` and its
/// version. When the driver reports a vendor name, a second line shows the
/// connected database, including its version when available.
fn format_banner(version: &str, vendor: &database::VendorInfo) -> String {
    let mut banner = format!("databow {version}");
    if let Some(name) = &vendor.name {
        banner.push('\n');
        match &vendor.version {
            Some(vendor_version) => {
                banner.push_str(&format!("Connected to {name} {vendor_version}"))
            }
            None => banner.push_str(&format!("Connected to {name}")),
        }
    }
    banner
}

pub fn run_repl(
    mut connection: impl Connection,
    table_mode: TableMode,
    truncation: TruncationOptions,
) {
    let vendor = database::get_vendor_info(&connection);
    println!("{}", format_banner(env!("CARGO_PKG_VERSION"), &vendor));

    let mut line_editor = Reedline::create()
        .with_highlighter(Box::new(SyntectHighlighter::new()))
        .with_validator(Box::new(SqlValidator));
    let prompt = SqlPrompt;
    let mut ctrl_c_count: u8 = 0;
    let mut state = ReplState {
        table_mode,
        truncation,
        pager: PagerSettings::default(),
        last_result: None,
    };

    loop {
        let signal = line_editor.read_line(&prompt);
        match signal {
            Ok(Signal::Success(buffer)) => {
                ctrl_c_count = 0;

                if buffer.trim().is_empty() {
                    continue;
                }

                if commands::is_command(&buffer) {
                    match commands::run(&buffer, &mut state) {
                        Ok(Outcome::Continue) => {}
                        Ok(Outcome::Exit) => break,
                        Err(err) => eprintln!("{err}"),
                    }
                    continue;
                }

                let sql = buffer.trim_end().trim_end_matches(';');
                // Free the previous result before running the next query so
                // two full results are never held at once.
                state.last_result = None;
                let batches = match database::execute_query(&mut connection, sql) {
                    Ok(result) => result.batches,
                    Err(err) => {
                        eprintln!("{err}");
                        continue;
                    }
                };
                // Resolve per query so terminal resizes are picked up.
                let options = RenderOptions {
                    mode: state.table_mode,
                    limits: state.truncation.resolve(),
                    hint: Some(commands::LAST_HINT),
                };
                if let Err(err) = commands::display_result(&batches, options, &state.pager) {
                    eprintln!("{err}");
                }
                state.last_result = Some(batches);
            }
            Ok(Signal::CtrlC) => {
                if ctrl_c_count == 0 {
                    ctrl_c_count = 1;
                } else {
                    break;
                }
            }
            Ok(Signal::CtrlD) => {
                break;
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::database::VendorInfo;

    #[test]
    fn test_format_banner_version_only() {
        let vendor = VendorInfo::default();
        assert_eq!(format_banner("0.1.2", &vendor), "databow 0.1.2");
    }

    #[test]
    fn test_format_banner_with_vendor_name_and_version() {
        let vendor = VendorInfo {
            name: Some("DuckDB".to_string()),
            version: Some("v1.1.0".to_string()),
        };
        assert_eq!(
            format_banner("0.1.2", &vendor),
            "databow 0.1.2\nConnected to DuckDB v1.1.0"
        );
    }

    #[test]
    fn test_format_banner_with_vendor_name_only() {
        let vendor = VendorInfo {
            name: Some("PostgreSQL".to_string()),
            version: None,
        };
        assert_eq!(
            format_banner("0.1.2", &vendor),
            "databow 0.1.2\nConnected to PostgreSQL"
        );
    }

    #[test]
    fn test_format_banner_version_present_without_name_is_ignored() {
        // A version with no name should not produce a second line.
        let vendor = VendorInfo {
            name: None,
            version: Some("v1.1.0".to_string()),
        };
        assert_eq!(format_banner("0.1.2", &vendor), "databow 0.1.2");
    }
}
