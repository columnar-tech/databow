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
    // The shell's exit code for a missing command is not reliable (`cmd`
    // reports 1, the same as many failures), so look the program up first.
    if let Some(program) = program_name(command)
        && !program_exists(
            &program,
            std::env::var_os("PATH").as_deref(),
            std::env::var_os("PATHEXT").as_deref(),
        )
    {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("{program} not found"),
        ));
    }

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
    // The pager's exit status is ignored: pagers may exit non-zero when quit
    // early, and the output has been shown either way.
    child.wait()?;

    match result {
        // The user quit the pager before reading all output.
        Err(err) if err.kind() == io::ErrorKind::BrokenPipe => {}
        other => other?,
    }
    Ok(())
}

/// The program a shell command runs: its first word, which may be quoted to
/// contain spaces. Returns `None` when the program cannot be determined
/// without a full shell, for example when it involves variables or globs, in
/// which case the command is run without checking.
fn program_name(command: &str) -> Option<String> {
    let rest = command.trim_start();
    // Skip `NAME=value` environment assignments, as in `LESS=FR less`.
    #[cfg(not(windows))]
    let rest = {
        let mut rest = rest;
        while let Some((word, after)) = rest.split_once(char::is_whitespace) {
            let is_assignment = word.split_once('=').is_some_and(|(name, _)| {
                !name.is_empty()
                    && !name.starts_with(|c: char| c.is_ascii_digit())
                    && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
            });
            if !is_assignment || word.contains(['\'', '"']) {
                break;
            }
            rest = after.trim_start();
        }
        rest
    };

    #[cfg(not(windows))]
    const QUOTES: &[char] = &['\'', '"'];
    #[cfg(windows)]
    const QUOTES: &[char] = &['"'];
    #[cfg(not(windows))]
    const SPECIAL: &[char] = &['$', '`', '*', '?', '[', '~', '\\'];
    #[cfg(windows)]
    const SPECIAL: &[char] = &['%', '^', '*', '?'];
    #[cfg(not(windows))]
    const OPERATORS: &[char] = &[';', '&', '|', '<', '>', '(', ')'];
    #[cfg(windows)]
    const OPERATORS: &[char] = &['&', '|', '<', '>', '(', ')'];

    let program = match rest.chars().next() {
        Some(quote) if QUOTES.contains(&quote) => {
            let inner = &rest[1..];
            let end = inner.find(quote)?;
            // A quoted word continued by more text, like `"a"b`, is left to
            // the shell.
            let after = &inner[end + 1..];
            if !(after.is_empty() || after.starts_with(char::is_whitespace)) {
                return None;
            }
            &inner[..end]
        }
        _ => {
            let end = rest
                .find(|c: char| c.is_whitespace() || OPERATORS.contains(&c))
                .unwrap_or(rest.len());
            let word = &rest[..end];
            if word.contains(QUOTES) {
                return None;
            }
            word
        }
    };
    if program.is_empty() || program.contains(SPECIAL) || is_shell_builtin(program) {
        return None;
    }
    Some(program.to_string())
}

/// Whether `word` is run by the shell itself rather than found on `PATH`, like
/// `exec less` with `sh` or `type` with `cmd`.
fn is_shell_builtin(word: &str) -> bool {
    #[cfg(not(windows))]
    const BUILTINS: &[&str] = &[
        "!", ".", ":", "[[", "{", "alias", "break", "builtin", "case", "cd", "command", "continue",
        "eval", "exec", "exit", "export", "for", "hash", "if", "local", "readonly", "return",
        "set", "shift", "source", "time", "times", "trap", "type", "ulimit", "umask", "unset",
        "until", "wait", "while",
    ];
    #[cfg(windows)]
    const BUILTINS: &[&str] = &[
        "assoc", "break", "call", "cd", "chdir", "cls", "color", "copy", "date", "del", "dir",
        "echo", "endlocal", "erase", "exit", "for", "ftype", "goto", "if", "md", "mkdir", "mklink",
        "move", "path", "pause", "popd", "prompt", "pushd", "rd", "rem", "ren", "rename", "rmdir",
        "set", "setlocal", "shift", "start", "time", "title", "type", "ver", "verify", "vol",
    ];
    if cfg!(windows) {
        BUILTINS.iter().any(|b| b.eq_ignore_ascii_case(word))
    } else {
        BUILTINS.contains(&word)
    }
}

/// Whether `value` is a path (not a bare name) to an existing program.
pub fn is_program_path(value: &str) -> bool {
    has_separator(value) && program_exists(value, None, std::env::var_os("PATHEXT").as_deref())
}

/// Whether `program` names an executable, either as a path or by searching
/// `path` (and on Windows, the current directory, trying the extensions in
/// `pathext`).
fn program_exists(
    program: &str,
    path: Option<&std::ffi::OsStr>,
    pathext: Option<&std::ffi::OsStr>,
) -> bool {
    use std::path::{Path, PathBuf};

    let mut candidates: Vec<PathBuf> = Vec::new();
    if has_separator(program) {
        candidates.push(PathBuf::from(program));
    } else {
        // `cmd` looks in the current directory before `PATH`.
        if cfg!(windows) {
            candidates.push(PathBuf::from(program));
        }
        if let Some(path) = path {
            candidates.extend(std::env::split_paths(path).map(|dir| dir.join(program)));
        }
    }

    if cfg!(windows) {
        let pathext = pathext
            .and_then(|value| value.to_str())
            .filter(|value| !value.trim().is_empty())
            .unwrap_or(".COM;.EXE;.BAT;.CMD");
        let extensions: Vec<&str> = pathext.split(';').filter(|ext| !ext.is_empty()).collect();
        let has_extension = Path::new(program).extension().is_some();
        candidates.iter().any(|candidate| {
            (has_extension && candidate.is_file())
                || extensions.iter().any(|ext| {
                    let mut with_ext = candidate.clone().into_os_string();
                    with_ext.push(ext);
                    Path::new(&with_ext).is_file()
                })
        })
    } else {
        candidates.iter().any(|candidate| is_executable(candidate))
    }
}

/// Whether `program` is a path rather than a name to search for.
fn has_separator(program: &str) -> bool {
    if cfg!(windows) {
        program.contains(['/', '\\', ':'])
    } else {
        program.contains('/')
    }
}

#[cfg(unix)]
fn is_executable(path: &std::path::Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
}

#[cfg(not(unix))]
fn is_executable(path: &std::path::Path) -> bool {
    path.is_file()
}

/// Options for `less` passed via the `LESS` environment variable, so that a
/// plain `less` from `$PAGER` still behaves like the default pager. Like git,
/// this is only done when the user has not set `LESS` themselves.
const DEFAULT_LESS: &str = "SRX";

fn less_options(existing: Option<&std::ffi::OsStr>) -> Option<&'static str> {
    existing.is_none().then_some(DEFAULT_LESS)
}

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
        let err = run_pager("hello", "databow-no-such-pager 2>nul").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn test_program_name() {
        assert_eq!(program_name("less -SR").as_deref(), Some("less"));
        assert_eq!(program_name("  more").as_deref(), Some("more"));
        assert_eq!(program_name("cat>out.txt").as_deref(), Some("cat"));
        assert_eq!(
            program_name(r#""/opt/my pager/less" -S"#).as_deref(),
            Some("/opt/my pager/less")
        );
        assert_eq!(program_name(""), None);
        assert_eq!(program_name(r#""unterminated"#), None);
        assert_eq!(program_name(r#""a"b"#), None);
    }

    #[cfg(not(windows))]
    #[test]
    fn test_program_name_unix() {
        assert_eq!(program_name("LESS=FR less").as_deref(), Some("less"));
        assert_eq!(program_name("A=1 B=2 less -S").as_deref(), Some("less"));
        assert_eq!(program_name("'less' -S").as_deref(), Some("less"));
        assert_eq!(program_name("less;true").as_deref(), Some("less"));
        // Programs that need the shell to expand are not checked.
        assert_eq!(program_name("$PAGER"), None);
        assert_eq!(program_name("~/bin/pager"), None);
        // Shell builtins have no file to look up.
        assert_eq!(program_name("exec less"), None);
        assert_eq!(program_name("command less"), None);
    }

    #[cfg(windows)]
    #[test]
    fn test_program_name_windows() {
        assert_eq!(
            program_name(r#""C:\Program Files\less.exe" -S"#).as_deref(),
            Some(r"C:\Program Files\less.exe")
        );
        assert_eq!(
            program_name(r"C:\tools\less.exe -S").as_deref(),
            Some(r"C:\tools\less.exe")
        );
        assert_eq!(program_name("%PAGER%"), None);
        assert_eq!(program_name("TYPE"), None);
    }

    #[cfg(unix)]
    #[test]
    fn test_program_exists() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let exe = dir.path().join("my-pager");
        std::fs::write(&exe, "").unwrap();
        std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755)).unwrap();
        let plain = dir.path().join("not-executable");
        std::fs::write(&plain, "").unwrap();
        std::fs::set_permissions(&plain, std::fs::Permissions::from_mode(0o644)).unwrap();

        let path = std::env::join_paths([dir.path()]).unwrap();
        assert!(program_exists("my-pager", Some(&path), None));
        assert!(program_exists(exe.to_str().unwrap(), None, None));
        assert!(!program_exists("not-executable", Some(&path), None));
        assert!(!program_exists("no-such-pager", Some(&path), None));
        assert!(!program_exists("my-pager", None, None));
    }

    #[cfg(windows)]
    #[test]
    fn test_program_exists() {
        use std::ffi::OsStr;
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("my-pager.exe"), "").unwrap();
        std::fs::write(dir.path().join("other.txt"), "").unwrap();

        let path = std::env::join_paths([dir.path()]).unwrap();
        let pathext = OsStr::new(".COM;.EXE");
        assert!(program_exists("my-pager", Some(&path), Some(pathext)));
        assert!(program_exists("my-pager.exe", Some(&path), Some(pathext)));
        // Without PATHEXT, the default extensions are tried.
        assert!(program_exists("my-pager", Some(&path), None));
        assert!(!program_exists("other", Some(&path), Some(pathext)));
        assert!(!program_exists("no-such-pager", Some(&path), Some(pathext)));
        let full = dir.path().join("my-pager");
        assert!(program_exists(full.to_str().unwrap(), None, Some(pathext)));
    }
}
