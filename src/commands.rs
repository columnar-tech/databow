// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

//! REPL dot commands such as `.last` and `.maxrows`, modeled on the DuckDB CLI.

use crate::pager::{self, PagerMode, PagerSettings};
use crate::table::{
    Limit, RenderOptions, TableMode, TruncationOptions, render_table, terminal_width,
};
use arrow_array::RecordBatch;
use clap::ValueEnum;

/// Hint shown in the table footer when the REPL hides rows or columns.
pub const LAST_HINT: &str = "use .last to show entire result";

/// Mutable REPL settings and state that dot commands act on.
pub struct ReplState {
    pub table_mode: TableMode,
    pub truncation: TruncationOptions,
    pub pager: PagerSettings,
    /// The most recent query result, kept so `.last` can show it in full.
    pub last_result: Option<Vec<RecordBatch>>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Help,
    Last,
    /// `None` shows the current value.
    MaxRows(Option<Limit>),
    /// `None` shows the current value.
    MaxWidth(Option<Limit>),
    /// `None` shows the current mode and the available modes.
    Mode(Option<TableMode>),
    Pager(PagerArg),
    Quit,
}

/// Argument to `.pager`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PagerArg {
    /// Show the current pager settings.
    Show,
    Mode(PagerMode),
    RowThreshold(usize),
    Command(String),
}

/// What the REPL should do after running a command.
#[derive(Debug, PartialEq, Eq)]
pub enum Outcome {
    Continue,
    Exit,
}

struct CommandInfo {
    names: &'static [&'static str],
    usage: &'static str,
    description: &'static str,
}

const COMMANDS: &[CommandInfo] = &[
    CommandInfo {
        names: &["help"],
        usage: "",
        description: "Show this help",
    },
    CommandInfo {
        names: &["last"],
        usage: "",
        description: "Show the last result in full, without truncation",
    },
    CommandInfo {
        names: &["maxrows"],
        usage: "?COUNT?",
        description: "Show or set the maximum number of rows to display (0 for no limit)",
    },
    CommandInfo {
        names: &["maxwidth"],
        usage: "?WIDTH?",
        description: "Show or set the maximum table width (0 for no limit)",
    },
    CommandInfo {
        names: &["mode"],
        usage: "?MODE?",
        description: "Show or set the table display style",
    },
    CommandInfo {
        names: &["pager"],
        usage: "?OPTIONS?",
        description: "Show or configure paging of large results",
    },
    CommandInfo {
        names: &["quit", "exit"],
        usage: "",
        description: "Exit databow",
    },
];

/// Whether a line of REPL input is a dot command rather than SQL.
pub fn is_command(input: &str) -> bool {
    input.trim_start().starts_with('.')
}

/// Parse a dot command. A trailing `;` is tolerated.
pub fn parse(input: &str) -> Result<Command, String> {
    let input = input.trim().trim_end_matches(';').trim_end();
    let mut words = input.split_whitespace();
    let name = words
        .next()
        .and_then(|word| word.strip_prefix('.'))
        .unwrap_or_default();
    let args: Vec<&str> = words.collect();

    let info = COMMANDS
        .iter()
        .find(|info| info.names.contains(&name))
        .ok_or_else(|| format!("Unknown command: .{name} (use .help to list commands)"))?;
    let usage = || {
        // `.help` shows `.pager ?OPTIONS?` to stay compact, but errors spell
        // out the accepted options.
        let args = match name {
            "pager" => "?on|off|automatic|set_row_threshold COUNT|COMMAND?",
            _ => info.usage,
        };
        let usage = format!(".{} {args}", info.names[0]);
        format!("Usage: {}", usage.trim_end())
    };

    let command = match (name, args.as_slice()) {
        ("help", []) => Command::Help,
        ("last", []) => Command::Last,
        ("maxrows", []) => Command::MaxRows(None),
        ("maxrows", [value]) => Command::MaxRows(Some(parse_limit(value).ok_or_else(usage)?)),
        ("maxwidth", []) => Command::MaxWidth(None),
        ("maxwidth", [value]) => Command::MaxWidth(Some(parse_limit(value).ok_or_else(usage)?)),
        ("mode", []) => Command::Mode(None),
        ("mode", [value]) => {
            Command::Mode(Some(TableMode::from_str(value, true).map_err(|_| {
                format!("Unknown mode: {value} (use .mode to list modes)")
            })?))
        }
        ("pager", []) => Command::Pager(PagerArg::Show),
        ("pager", [value]) if parse_pager_mode(value).is_some() => {
            Command::Pager(PagerArg::Mode(parse_pager_mode(value).unwrap()))
        }
        ("pager", [keyword, value]) if is_row_threshold(keyword) => {
            Command::Pager(PagerArg::RowThreshold(value.parse().map_err(|_| usage())?))
        }
        ("pager", [keyword, ..])
            if is_row_threshold(keyword) || parse_pager_mode(keyword).is_some() =>
        {
            return Err(usage());
        }
        // Anything else is a pager command, which may contain spaces and
        // quotes, so take it verbatim from the input.
        ("pager", [_, ..]) => {
            let command = input
                .trim_start()
                .strip_prefix(".pager")
                .unwrap_or_default()
                .trim();
            Command::Pager(PagerArg::Command(unquote(command)))
        }
        ("quit" | "exit", []) => Command::Quit,
        _ => return Err(usage()),
    };
    Ok(command)
}

/// Strip one pair of matching surrounding quotes, so that DuckDB-style
/// `.pager 'less -S'` works. Values like `'a' 'b'`, where the outer quotes
/// belong to separate quoted words, are left alone. A quoted path to a
/// program, like `"C:\Program Files\less.exe"`, keeps its quotes so that it
/// reaches the shell as one word; `cmd` only understands double quotes, so on
/// Windows single quotes are replaced.
fn unquote(value: &str) -> String {
    for quote in ['\'', '"'] {
        if let Some(inner) = value
            .strip_prefix(quote)
            .and_then(|rest| rest.strip_suffix(quote))
            .filter(|inner| !inner.contains(quote))
        {
            if !pager::is_program_path(inner) {
                return inner.to_string();
            }
            if cfg!(windows) {
                return format!("\"{inner}\"");
            }
            break;
        }
    }
    value.to_string()
}

fn is_row_threshold(keyword: &str) -> bool {
    keyword.eq_ignore_ascii_case("set_row_threshold")
}

fn parse_pager_mode(value: &str) -> Option<PagerMode> {
    match value.to_ascii_lowercase().as_str() {
        "on" => Some(PagerMode::On),
        "off" => Some(PagerMode::Off),
        "automatic" | "auto" => Some(PagerMode::Automatic),
        _ => None,
    }
}

fn parse_limit(value: &str) -> Option<Limit> {
    if value.eq_ignore_ascii_case("default") {
        Some(Limit::Default)
    } else {
        value.parse().ok().map(Limit::from_count)
    }
}

/// Parse and run a dot command against the REPL state.
pub fn run(input: &str, state: &mut ReplState) -> Result<Outcome, String> {
    let (outcome, message) = execute(parse(input)?, state)?;
    if let Some(message) = message {
        println!("{message}");
    }
    Ok(outcome)
}

/// Apply a command, returning any message to print.
fn execute(command: Command, state: &mut ReplState) -> Result<(Outcome, Option<String>), String> {
    let message = match command {
        Command::Help => Some(help()),
        Command::Last => {
            let Some(batches) = &state.last_result else {
                return Err("No previous result".to_string());
            };
            let options = RenderOptions {
                mode: state.table_mode,
                ..Default::default()
            };
            display_result(batches, options, &state.pager)?;
            None
        }
        Command::MaxRows(Some(limit)) => {
            state.truncation.max_rows = limit;
            None
        }
        Command::MaxRows(None) => Some(describe_max_rows(
            state.truncation.max_rows,
            state.truncation.resolve().max_rows,
        )),
        Command::MaxWidth(Some(limit)) => {
            state.truncation.max_width = limit;
            None
        }
        Command::MaxWidth(None) => Some(describe_max_width(
            state.truncation.max_width,
            state.truncation.resolve().max_width,
        )),
        Command::Mode(Some(mode)) => {
            state.table_mode = mode;
            None
        }
        Command::Mode(None) => Some(describe_mode(state.table_mode)),
        Command::Pager(PagerArg::Show) => Some(describe_pager(&state.pager)),
        Command::Pager(PagerArg::Mode(mode)) => {
            state.pager.mode = mode;
            None
        }
        Command::Pager(PagerArg::RowThreshold(rows)) => {
            state.pager.row_threshold = rows;
            None
        }
        Command::Pager(PagerArg::Command(command)) => {
            // Like DuckDB, setting a command does not change the mode.
            state.pager.command = Some(command);
            None
        }
        Command::Quit => return Ok((Outcome::Exit, None)),
    };
    Ok((Outcome::Continue, message))
}

/// Render a query result and print it, through the pager if the result does
/// not fit on the screen (subject to the pager settings).
pub fn display_result(
    batches: &[RecordBatch],
    options: RenderOptions,
    pager: &PagerSettings,
) -> Result<(), String> {
    let rendered =
        render_table(batches, options).map_err(|err| format!("Failed to print batches: {err}"))?;
    if pager.should_page(rendered.shown_rows(), rendered.width(), terminal_width()) {
        pager::page(&rendered.to_string(), &pager.command());
    } else {
        println!("{rendered}");
    }
    Ok(())
}

fn help() -> String {
    let entries: Vec<(String, &str)> = COMMANDS
        .iter()
        .map(|info| {
            let names = info
                .names
                .iter()
                .map(|name| format!(".{name}"))
                .collect::<Vec<_>>()
                .join(", ");
            let usage = format!("{names} {}", info.usage);
            (usage.trim_end().to_string(), info.description)
        })
        .collect();
    let width = entries
        .iter()
        .map(|(usage, _)| usage.len())
        .max()
        .unwrap_or(0);
    entries
        .iter()
        .map(|(usage, description)| format!("{usage:width$}  {description}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn describe_max_rows(setting: Limit, resolved: Option<usize>) -> String {
    let value = resolved.map_or_else(|| "no limit".to_string(), |rows| rows.to_string());
    match setting {
        Limit::Default => format!("max rows: {value} (default)"),
        _ => format!("max rows: {value}"),
    }
}

fn describe_max_width(setting: Limit, resolved: Option<usize>) -> String {
    let value = resolved.map_or_else(|| "no limit".to_string(), |width| width.to_string());
    match setting {
        Limit::Default => format!("max width: {value} (default: terminal width)"),
        _ => format!("max width: {value}"),
    }
}

fn describe_pager(pager: &PagerSettings) -> String {
    let mut lines = vec![format!("pager mode: {}", pager.mode)];
    match pager.mode {
        PagerMode::Automatic => lines.push(format!(
            "pager is used when more than {} rows are shown or the result is wider than the terminal",
            pager.row_threshold
        )),
        PagerMode::On => lines.push("pager is always used".to_string()),
        PagerMode::Off => {}
    }
    lines.push(format!("pager command: {}", pager.command()));
    lines.join("\n")
}

fn describe_mode(current: TableMode) -> String {
    let modes = TableMode::value_variants()
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    format!("mode: {current}\navailable modes: {modes}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> ReplState {
        ReplState {
            table_mode: TableMode::default(),
            truncation: TruncationOptions::default(),
            pager: PagerSettings::default(),
            last_result: None,
        }
    }

    #[test]
    fn test_every_listed_command_parses() {
        for info in COMMANDS {
            for name in info.names {
                assert!(parse(&format!(".{name}")).is_ok(), "cannot parse .{name}");
            }
        }
    }

    #[test]
    fn test_is_command() {
        assert!(is_command(".last"));
        assert!(is_command("  .maxrows 10"));
        assert!(!is_command("SELECT '.last';"));
        assert!(!is_command(""));
    }

    #[test]
    fn test_parse_commands() {
        assert_eq!(parse(".help"), Ok(Command::Help));
        assert_eq!(parse(".last"), Ok(Command::Last));
        assert_eq!(parse(".last;"), Ok(Command::Last));
        assert_eq!(parse(".quit"), Ok(Command::Quit));
        assert_eq!(parse(".exit"), Ok(Command::Quit));
        assert_eq!(parse(".maxrows"), Ok(Command::MaxRows(None)));
        assert_eq!(
            parse(".maxrows 100"),
            Ok(Command::MaxRows(Some(Limit::Value(100))))
        );
        assert_eq!(
            parse("  .maxrows   0  "),
            Ok(Command::MaxRows(Some(Limit::Unlimited)))
        );
        assert_eq!(
            parse(".maxwidth default"),
            Ok(Command::MaxWidth(Some(Limit::Default)))
        );
        assert_eq!(
            parse(".mode ascii-markdown"),
            Ok(Command::Mode(Some(TableMode::AsciiMarkdown)))
        );
    }

    #[test]
    fn test_parse_errors() {
        assert!(
            parse(".bogus")
                .unwrap_err()
                .contains("Unknown command: .bogus")
        );
        assert!(parse(".").unwrap_err().contains("Unknown command"));
        assert_eq!(
            parse(".maxrows lots"),
            Err("Usage: .maxrows ?COUNT?".to_string())
        );
        assert_eq!(
            parse(".maxrows -1"),
            Err("Usage: .maxrows ?COUNT?".to_string())
        );
        assert_eq!(
            parse(".maxrows 1 2"),
            Err("Usage: .maxrows ?COUNT?".to_string())
        );
        assert_eq!(parse(".last now"), Err("Usage: .last".to_string()));
        assert!(parse(".mode fancy").unwrap_err().contains("Unknown mode"));
    }

    #[test]
    fn test_execute_sets_limits() {
        let mut state = state();
        execute(Command::MaxRows(Some(Limit::Value(10))), &mut state).unwrap();
        execute(Command::MaxWidth(Some(Limit::Unlimited)), &mut state).unwrap();
        assert_eq!(state.truncation.max_rows, Limit::Value(10));
        assert_eq!(state.truncation.max_width, Limit::Unlimited);

        execute(Command::MaxRows(Some(Limit::Default)), &mut state).unwrap();
        assert_eq!(state.truncation.max_rows, Limit::Default);
    }

    #[test]
    fn test_execute_sets_mode() {
        let mut state = state();
        execute(Command::Mode(Some(TableMode::Utf8Full)), &mut state).unwrap();
        assert_eq!(state.table_mode, TableMode::Utf8Full);
        let (_, message) = execute(Command::Mode(None), &mut state).unwrap();
        assert!(message.unwrap().starts_with("mode: utf8-full\n"));
    }

    #[test]
    fn test_parse_pager() {
        assert_eq!(parse(".pager"), Ok(Command::Pager(PagerArg::Show)));
        assert_eq!(
            parse(".pager on"),
            Ok(Command::Pager(PagerArg::Mode(PagerMode::On)))
        );
        assert_eq!(
            parse(".pager off;"),
            Ok(Command::Pager(PagerArg::Mode(PagerMode::Off)))
        );
        assert_eq!(
            parse(".pager automatic"),
            Ok(Command::Pager(PagerArg::Mode(PagerMode::Automatic)))
        );
        assert_eq!(
            parse(".pager set_row_threshold 100"),
            Ok(Command::Pager(PagerArg::RowThreshold(100)))
        );
        assert_eq!(
            parse(".pager On"),
            Ok(Command::Pager(PagerArg::Mode(PagerMode::On)))
        );
        assert_eq!(
            parse(".pager AUTO"),
            Ok(Command::Pager(PagerArg::Mode(PagerMode::Automatic)))
        );
        assert_eq!(
            parse(".pager SET_ROW_THRESHOLD 3"),
            Ok(Command::Pager(PagerArg::RowThreshold(3)))
        );
        assert!(parse(".pager set_row_threshold").is_err());
        assert!(parse(".pager set_row_threshold many").is_err());
        assert_eq!(
            parse(".pager less -SR"),
            Ok(Command::Pager(PagerArg::Command("less -SR".to_string())))
        );
        assert_eq!(
            parse(".pager 'pspg --csv'"),
            Ok(Command::Pager(PagerArg::Command("pspg --csv".to_string())))
        );
        assert_eq!(
            parse(".pager 'a' 'b'"),
            Ok(Command::Pager(PagerArg::Command("'a' 'b'".to_string())))
        );
        // A quoted path to an existing program keeps its quotes.
        let dir = tempfile::tempdir().unwrap();
        let program = dir.path().join(if cfg!(windows) {
            "my pager.exe"
        } else {
            "my pager"
        });
        std::fs::write(&program, "").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&program, std::fs::Permissions::from_mode(0o755)).unwrap();
        }
        let quoted = format!("\"{}\"", program.display());
        assert_eq!(
            parse(&format!(".pager {quoted}")),
            Ok(Command::Pager(PagerArg::Command(quoted.clone())))
        );
        let single = format!("'{}'", program.display());
        let expected = if cfg!(windows) {
            quoted
        } else {
            single.clone()
        };
        assert_eq!(
            parse(&format!(".pager {single}")),
            Ok(Command::Pager(PagerArg::Command(expected)))
        );
        assert_eq!(
            parse(r#".pager "C:\Program Files\less.exe" -S"#),
            Ok(Command::Pager(PagerArg::Command(
                r#""C:\Program Files\less.exe" -S"#.to_string()
            )))
        );
        assert!(parse(".pager on extra").is_err());
        assert!(parse(".pager off now").is_err());
    }

    #[test]
    fn test_execute_pager() {
        let mut state = state();
        execute(Command::Pager(PagerArg::Mode(PagerMode::Off)), &mut state).unwrap();
        execute(Command::Pager(PagerArg::RowThreshold(7)), &mut state).unwrap();
        execute(
            Command::Pager(PagerArg::Command("most".to_string())),
            &mut state,
        )
        .unwrap();
        assert_eq!(
            state.pager,
            PagerSettings {
                mode: PagerMode::Off,
                command: Some("most".to_string()),
                row_threshold: 7,
            }
        );
        let (_, message) = execute(Command::Pager(PagerArg::Show), &mut state).unwrap();
        assert_eq!(message.unwrap(), "pager mode: off\npager command: most");
    }

    #[test]
    fn test_execute_quit() {
        assert_eq!(
            execute(Command::Quit, &mut state()).unwrap().0,
            Outcome::Exit
        );
    }

    #[test]
    fn test_last_without_result() {
        assert_eq!(
            run(".last", &mut state()),
            Err("No previous result".to_string())
        );
    }

    #[test]
    fn test_describe_limits() {
        assert_eq!(
            describe_max_rows(Limit::Default, Some(40)),
            "max rows: 40 (default)"
        );
        assert_eq!(
            describe_max_rows(Limit::Unlimited, None),
            "max rows: no limit"
        );
        assert_eq!(describe_max_rows(Limit::Value(5), Some(5)), "max rows: 5");
        assert_eq!(
            describe_max_width(Limit::Default, Some(120)),
            "max width: 120 (default: terminal width)"
        );
        assert_eq!(
            describe_max_width(Limit::Unlimited, None),
            "max width: no limit"
        );
    }

    #[test]
    fn test_help_lists_commands() {
        let help = help();
        for name in [
            ".help",
            ".last",
            ".maxrows",
            ".maxwidth",
            ".mode",
            ".pager",
            ".quit, .exit",
        ] {
            assert!(help.contains(name), "missing {name} in:\n{help}");
        }
    }
}
