## [0.1.0-alpha] - 2026-02-20

### 🚀 Features

- Replace arrow pretty print with custom table formatter
- Add automatic terminal theme detection to syntax highlighter
- Add dynamic content arrangement
- Add non-interactive usage modes
- Add custom catppuccin themes
- Add configurable table display styles
- Add utf8_compact table style
- Add file output
- Add multiline SQL input support in REPL
- Require double Ctrl+C to exit REPL

### 🐛 Bug Fixes

- Continue repl loop on empty input
- Prevent empty file creation on empty query results
- Print REPL errors to stderr instead of stdout

### 🚜 Refactor

- Replace hardcoded version with CARGO_PKG_VERSION macro
- Extract option parsing logic
- Use PathBuf value parser for file argument
- Reuse database::execute_query in REPL
- Separate connection config from application config
- Use clap ValueEnum for table mode parsing
- Simplify option handling in syntax lookup
- Return Result from initialize_connection

### 📚 Documentation

- Update examples to reflect new table formatter output
- Add highlights section
- Update installation guide
- Add contributing guide
- Fix security email
- Add logo
- Add databases page
- Add documentation for file export feature
- Clean up README and docs code examples

### 🧪 Testing

- Add unit tests for cli and table modules

### ⚙️ Miscellaneous Tasks

- Initial commit
- Update rust toolchain channel
- Remove prettyprint feature from arrow dependency
- Add dbc and duckdb driver installation to workflow
- Add license headers
- Add pyproject.toml file
- Fix clippy pedantic errors
- Add package metadata
