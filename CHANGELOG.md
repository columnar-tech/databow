## [unreleased]

### 🚀 Features

- Infer driver from URI scheme (#29)

### 📚 Documentation

- Remove --pre from dbc install clickhouse (#25)
- Add Homebrew installation instructions (#28)
- Add chdb example (#30)

### ⚙️ Miscellaneous Tasks

- Limit pypi workflow to release and manual dispatch triggers
## [0.1.2] - 2026-07-08

### 🐛 Bug Fixes

- Expand env_var substitution in profile option values (#20)

### 📚 Documentation

- Add GizmoSQL setup instructions to database documentation (#24)

### ⚙️ Miscellaneous Tasks

- Trigger releases on GitHub Release published
- Prevent changelog workflow loop without [skip ci]
- Allow workflow_dispatch to publish to PyPI
- Add workflow_dispatch trigger and concurrency to docs deploy
- Run workflow on pull requests
- Skip changelog-update commits in git-cliff config
- Add missing license header
- Bump version to 0.1.2
## [0.1.1] - 2026-06-29

### 🐛 Bug Fixes

- Render timestamp with time zone data

### 📚 Documentation

- Fix database driver examples (#7)
- Add datafusion, spark, and clickhouse examples (#13)

### ⚙️ Miscellaneous Tasks

- Add workflow to auto-update changelog via git-cliff
- Add crates.io release workflow
- Bump version to 0.1.1
## [0.1.0] - 2026-05-28

### 📚 Documentation

- Update installation guide
- Update demo gif url

### ⚙️ Miscellaneous Tasks

- Bump version
## [0.1.0-beta.2] - 2026-05-08

### ⚙️ Miscellaneous Tasks

- Use trusted publishing for PyPI releases
## [0.1.0-beta.1] - 2026-05-07

### ⚙️ Miscellaneous Tasks

- Enable trusted publishing
- Bump version to 0.1.0-beta.1
## [0.1.0-beta] - 2026-05-07

### 🚀 Features

- Add connection profile support

### 📚 Documentation

- Add crates.io installation instructions
- Add repository url
- Set repo name and icon
- Rename images directory
- Add databow badge
- Fix databow badge url
- Add demo gif
- Add tutorial page

### ⚙️ Miscellaneous Tasks

- Add badges
- Rename project
- Add workflow for docs deployment
- Upgrade dependencies
- Upgrade rust toolchain channel
- Extend python configuration
- Add pypi workflow
- Remove vscode config
- Bump version to 0.1.0-beta
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
- Add changelog
