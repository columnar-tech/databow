// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

//! Display large output through an external pager such as `less`, modeled on
//! the DuckDB CLI's `.pager` support.

use std::fmt;
use std::io::{self, Write};
use std::process::{Command, Stdio};

/// Default number of result rows that triggers the pager in automatic mode.
pub const DEFAULT_ROW_THRESHOLD: usize = 50;

/// Pager used when neither `DATABOW_PAGER` nor `PAGER` is set. `-S` chops long
/// lines so wide tables scroll horizontally, `-R` passes ANSI colors through,
/// and `-X` leaves the output on screen after quitting.
#[cfg(not(windows))]
const DEFAULT_PAGER: &str = "less -SRX";
#[cfg(windows)]
const DEFAULT_PAGER: &str = "more";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum PagerMode {
    /// Page when the output does not fit: too many rows or too wide.
    #[default]
    Automatic,
    /// Always page.
    On,
    /// Never page.
    Off,
}

impl fmt::Display for PagerMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            PagerMode::Automatic => "automatic",
            PagerMode::On => "on",
            PagerMode::Off => "off",
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PagerSettings {
    pub mode: PagerMode,
    /// Pager command set with `.pager COMMAND`; falls back to the environment.
    pub command: Option<String>,
    /// In automatic mode, page when more than this many rows are shown.
    pub row_threshold: usize,
}

impl Default for PagerSettings {
    fn default() -> Self {
        Self {
            mode: PagerMode::default(),
            command: None,
            row_threshold: DEFAULT_ROW_THRESHOLD,
        }
    }
}

impl PagerSettings {
    /// The pager command: `.pager COMMAND`, else `$DATABOW_PAGER`, else
    /// `$PAGER`, else the platform default.
    pub fn command(&self) -> String {
        self.command_with(|name| std::env::var(name).ok())
    }

    fn command_with(&self, env: impl Fn(&str) -> Option<String>) -> String {
        self.command
            .clone()
            .or_else(|| env("DATABOW_PAGER").filter(|v| !v.trim().is_empty()))
            .or_else(|| env("PAGER").filter(|v| !v.trim().is_empty()))
            .unwrap_or_else(|| DEFAULT_PAGER.to_string())
    }

    /// Decide whether output with `rows` result rows and `width` display
    /// columns should be paged. `terminal_width` is `None` when stdout is not
    /// a terminal, in which case paging is never used.
    pub fn should_page(&self, rows: usize, width: usize, terminal_width: Option<usize>) -> bool {
        let Some(terminal_width) = terminal_width else {
            return false;
        };
        match self.mode {
            PagerMode::Off => false,
            PagerMode::On => true,
            PagerMode::Automatic => rows > self.row_threshold || width > terminal_width,
        }
    }
}

/// Write `text` to the pager, falling back to stdout if the pager cannot be
/// started.
pub fn page(text: &str, command: &str) {
    if let Err(err) = run_pager(text, command) {
        eprintln!("Failed to run pager '{command}': {err}. Printing to stdout instead.");
        println!("{text}");
    }
}

fn run_pager(text: &str, command: &str) -> io::Result<()> {
    let mut cmd = shell_command(command);
    if let Some(less) = less_options(std::env::var_os("LESS").as_deref()) {
        cmd.env("LESS", less);
    }
    let mut child = cmd.stdin(Stdio::piped()).spawn()?;

    // Ctrl-C inside the pager is delivered to databow too, since both share
    // the terminal (the foreground process group on Unix, the console on
    // Windows). Ignore it while the pager runs so that it does not kill the
    // REPL. This is set after spawning so the pager keeps the default
    // handling.
    let _guard = IgnoreInterrupts::new();

    let mut stdin = child.stdin.take().expect("stdin was piped");
    let result = stdin
        .write_all(text.as_bytes())
        .and_then(|()| stdin.write_all(b"\n"));
    // Close the pipe so the pager sees end of input.
    drop(stdin);
    let status = child.wait()?;

    match result {
        // The user quit the pager before reading all output.
        Err(err) if err.kind() == io::ErrorKind::BrokenPipe => {}
        other => other?,
    }
    if status.code() == Some(COMMAND_NOT_FOUND) {
        return Err(io::Error::new(io::ErrorKind::NotFound, "command not found"));
    }
    Ok(())
}

/// Options for `less` passed via the `LESS` environment variable, so that a
/// plain `less` from `$PAGER` still behaves like the default pager. Like git,
/// this is only done when the user has not set `LESS` themselves.
const DEFAULT_LESS: &str = "SRX";

fn less_options(existing: Option<&std::ffi::OsStr>) -> Option<&'static str> {
    existing.is_none().then_some(DEFAULT_LESS)
}

/// Exit code the shell reports for a missing command.
#[cfg(not(windows))]
const COMMAND_NOT_FOUND: i32 = 127;
#[cfg(windows)]
const COMMAND_NOT_FOUND: i32 = 9009;

#[cfg(not(windows))]
fn shell_command(command: &str) -> Command {
    let mut cmd = Command::new("sh");
    cmd.args(["-c", command]);
    cmd
}

/// `cmd` does its own parsing of the command line instead of the C runtime's,
/// so the command must not be quoted by `Command::args`, which would pass
/// `"C:\Program Files\less.exe" -S` as `"\"C:\Program Files\less.exe\" -S"`.
/// With `/S`, `cmd` strips exactly one pair of outer quotes and runs the rest
/// verbatim, so wrapping the command in quotes passes it through unchanged.
#[cfg(windows)]
fn shell_command(command: &str) -> Command {
    use std::os::windows::process::CommandExt;
    let mut cmd = Command::new("cmd");
    cmd.raw_arg(format!("/S /C \"{command}\""));
    cmd
}

/// Ignores Ctrl-C for as long as it is alive, restoring the previous handling
/// on drop.
struct IgnoreInterrupts {
    /// The handler to restore, or `None` if installing `SIG_IGN` failed.
    #[cfg(unix)]
    previous: Option<libc::sighandler_t>,
    #[cfg(windows)]
    ignored: bool,
}

impl IgnoreInterrupts {
    fn new() -> Self {
        #[cfg(unix)]
        {
            // SAFETY: installing SIG_IGN is async-signal-safe and the previous
            // handler is restored on drop.
            let previous = unsafe { libc::signal(libc::SIGINT, libc::SIG_IGN) };
            Self {
                previous: (previous != libc::SIG_ERR).then_some(previous),
            }
        }
        #[cfg(windows)]
        {
            use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;
            // SAFETY: a null handler with TRUE makes this process ignore
            // Ctrl-C; it is undone on drop.
            let ignored = unsafe { SetConsoleCtrlHandler(None, 1) } != 0;
            Self { ignored }
        }
        #[cfg(not(any(unix, windows)))]
        Self {}
    }
}

impl Drop for IgnoreInterrupts {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(previous) = self.previous {
            // SAFETY: restores the handler returned by `signal` in `new`.
            unsafe {
                libc::signal(libc::SIGINT, previous);
            }
        }
        #[cfg(windows)]
        if self.ignored {
            use windows_sys::Win32::System::Console::SetConsoleCtrlHandler;
            // SAFETY: restores normal Ctrl-C processing, undoing `new`.
            unsafe {
                SetConsoleCtrlHandler(None, 0);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_should_page_automatic() {
        let settings = PagerSettings::default();
        assert!(!settings.should_page(10, 80, Some(100)));
        // Too many rows.
        assert!(!settings.should_page(50, 80, Some(100)));
        assert!(settings.should_page(51, 80, Some(100)));
        // Too wide.
        assert!(settings.should_page(10, 101, Some(100)));
    }

    #[test]
    fn test_should_page_modes() {
        let on = PagerSettings {
            mode: PagerMode::On,
            ..Default::default()
        };
        let off = PagerSettings {
            mode: PagerMode::Off,
            ..Default::default()
        };
        assert!(on.should_page(1, 1, Some(100)));
        assert!(!off.should_page(1000, 1000, Some(100)));
    }

    #[test]
    fn test_should_page_requires_terminal() {
        let on = PagerSettings {
            mode: PagerMode::On,
            ..Default::default()
        };
        assert!(!on.should_page(1000, 1000, None));
    }

    #[test]
    fn test_row_threshold() {
        let settings = PagerSettings {
            row_threshold: 5,
            ..Default::default()
        };
        assert!(!settings.should_page(5, 10, Some(100)));
        assert!(settings.should_page(6, 10, Some(100)));
    }

    #[test]
    fn test_command_resolution() {
        let env = |vars: &'static [(&'static str, &'static str)]| {
            move |name: &str| {
                vars.iter()
                    .find(|(key, _)| *key == name)
                    .map(|(_, value)| value.to_string())
            }
        };
        let settings = PagerSettings::default();
        assert_eq!(settings.command_with(env(&[])), DEFAULT_PAGER);
        assert_eq!(settings.command_with(env(&[("PAGER", "more")])), "more");
        assert_eq!(
            settings.command_with(env(&[("PAGER", "more"), ("DATABOW_PAGER", "bat")])),
            "bat"
        );
        // Empty values are ignored.
        assert_eq!(
            settings.command_with(env(&[("DATABOW_PAGER", " "), ("PAGER", "more")])),
            "more"
        );
        let explicit = PagerSettings {
            command: Some("most".to_string()),
            ..Default::default()
        };
        assert_eq!(
            explicit.command_with(env(&[("DATABOW_PAGER", "bat")])),
            "most"
        );
    }

    #[test]
    fn test_less_options() {
        use std::ffi::OsStr;
        assert_eq!(less_options(None), Some("SRX"));
        // The user's own LESS settings are left alone, even if empty.
        assert_eq!(less_options(Some(OsStr::new("FR"))), None);
        assert_eq!(less_options(Some(OsStr::new(""))), None);
    }

    #[cfg(unix)]
    #[test]
    fn test_run_pager_writes_all_output() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out.txt");
        run_pager("hello\nworld", &format!("cat > '{}'", path.display())).unwrap();
        assert_eq!(std::fs::read_to_string(path).unwrap(), "hello\nworld\n");
    }

    #[cfg(unix)]
    #[test]
    fn test_run_pager_quit_early() {
        // A pager that exits without reading must not be an error.
        let text = "x".repeat(1 << 20);
        run_pager(&text, "true").unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn test_run_pager_missing_command() {
        let err = run_pager("hello", "databow-no-such-pager 2>/dev/null").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[cfg(windows)]
    #[test]
    fn test_run_pager_writes_all_output() {
        // A quoted program path and a quoted output path with a space: cmd
        // must receive the command verbatim for this to work.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("out file.txt");
        let system_root = std::env::var("SystemRoot").unwrap();
        let command = format!(
            r#""{system_root}\System32\findstr.exe" "^" > "{}""#,
            path.display()
        );
        run_pager("hello\nworld", &command).unwrap();
        let output = std::fs::read_to_string(path).unwrap().replace("\r\n", "\n");
        assert_eq!(output, "hello\nworld\n");
    }

    #[cfg(windows)]
    #[test]
    fn test_run_pager_quit_early() {
        // A pager that exits without reading must not be an error.
        let text = "x".repeat(1 << 20);
        run_pager(&text, "exit 0").unwrap();
    }

    #[cfg(windows)]
    #[test]
    fn test_run_pager_missing_command() {
        // No `2>nul` here: with a redirection, cmd reports a missing command
        // with exit code 1 instead of 9009.
        let err = run_pager("hello", "databow-no-such-pager").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }
}
