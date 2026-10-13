---
icon: lucide/terminal
---

<!--
Copyright 2026 Columnar Technologies Inc.
SPDX-License-Identifier: Apache-2.0
-->

# Reference

## CLI Arguments

### --profile

Connection profile name or path

```sh
databow --profile warehouse
```

### --driver

Driver name

```sh
databow --driver duckdb
```

### --uri

Database uniform resource identifier

```sh
databow --driver mysql --uri root@tcp(localhost:33577)/test
```

### --username

Database user username

```sh
databow --driver flightsql --uri grpc://localhost:9408 --username root
```

### --password

Database user password

```sh
databow --driver flightsql --uri grpc://localhost:9408 --username root --password my_pwd
```

### --option

Driver-specific database option

```sh
databow --driver duckdb --option path=file.db
```

### --mode

Table display style

| Mode | Description |
|------|-------------|
| `ascii-full` | ASCII table with all borders |
| `ascii-full-condensed` | ASCII table without row dividers |
| `ascii-borders-only` | ASCII table with outer borders only |
| `ascii-borders-only-condensed` | ASCII outer borders without row spacing |
| `ascii-horizontal-only` | ASCII with horizontal lines only |
| `ascii-markdown` | Markdown-compatible table format |
| `ascii-no-borders` | ASCII table without any borders |
| `utf8-compact` | Compact UTF-8 table style (default) |
| `utf8-full` | UTF-8 box drawing with all borders |
| `utf8-full-condensed` | UTF-8 box drawing without row dividers |
| `utf8-borders-only` | UTF-8 with outer borders only |
| `utf8-horizontal-only` | UTF-8 with horizontal lines only |
| `utf8-no-borders` | UTF-8 table without any borders |
| `nothing` | No borders or lines |

```sh
databow --driver duckdb --mode ascii-markdown
```

### --query

Execute query and exit

```sh
databow --driver duckdb --query "SELECT 42 AS the_answer"
```

### --file

Read and execute file and exit

```sh
databow --driver duckdb --file select_example.sql
```

### --output

Write result to file

```sh
databow --driver duckdb --query "SELECT 42 AS the_answer" --output result.json
```

The output format is inferred from the file extension:

| Extension       | Format           |
|-----------------|------------------|
| `.json`         | JSON             |
| `.jsonl`        | JSON lines       |
| `.csv`          | CSV              |
| `.arrow`, `.ipc`| Arrow IPC file   |
| `.arrows`       | Arrow IPC stream |
| `.parquet`      | Parquet          |

### --help

Print the help message

```sh
databow --help
```

### --version

Print the version

```sh
databow --version
```

## Dot Commands

The interactive shell accepts dot commands in addition to SQL. Dot commands start with `.`, take up a single line, and do not need a terminating semicolon.

### .help

List the available dot commands

```
> .help
```

### .last

Show the last result in full, without truncation

```
> SELECT * FROM range(1000);
...
  1000 rows (40 shown)       use .last to show entire result       1 column
> .last
```

### .maxrows

Show or set the maximum number of rows to display

```
> .maxrows 100
```

When a result has more rows than `.maxrows`, the first and last rows are shown with the rows in between elided, and a footer reports how many rows were shown. The default is `40` when writing to a terminal.

| Argument | Description |
|----------|-------------|
| _(none)_ | Show the current value |
| `COUNT` | Show at most `COUNT` rows (`0` for no limit) |
| `default` | Restore the default |

Results of queries run with `--query`, `--file`, or stdin are never truncated.

### .maxwidth

Show or set the maximum table width

```
> .maxwidth 120
```

When a result is wider than `.maxwidth`, long values are truncated with `…`; if that is not enough, columns in the middle of the result are hidden and replaced with a single `…` column. The default is the terminal width.

| Argument | Description |
|----------|-------------|
| _(none)_ | Show the current value |
| `WIDTH` | Limit tables to `WIDTH` columns (`0` for no limit) |
| `default` | Restore the default |

Results of queries run with `--query`, `--file`, or stdin are never truncated, and long values wrap to fit the terminal instead.

### .mode

Show or set the table display style

```
> .mode ascii-markdown
```

Accepts the same modes as [`--mode`](#-mode), and starts from the value given by `--mode`. Run `.mode` without an argument to show the current mode and list the available modes.

### .pager

Show or configure paging of large results

```
> .pager on
> .pager set_row_threshold 100
> .pager less -SR
```

| Argument | Description |
|----------|-------------|
| _(none)_ | Show the current pager settings |
| `automatic` | Use the pager when the result doesn't fit on the screen (default) |
| `on` | Always use the pager |
| `off` | Never use the pager |
| `set_row_threshold COUNT` | In automatic mode, use the pager when more than `COUNT` rows are shown (default: `50`) |
| `COMMAND` | Set the pager command |

In `automatic` mode, the pager is used when more than 50 rows are shown or the result is wider than the terminal. Results that are truncated to fit the screen don't reach either threshold, so in practice the pager is used for `.last` and when the limits are raised with `.maxrows` or `.maxwidth`.

The pager command is taken from `.pager COMMAND`, then the `DATABOW_PAGER` and `PAGER` environment variables, and defaults to `less -SRX` (`more` on Windows). The `-S` option keeps long lines from wrapping so that wide tables can be scrolled horizontally. If the `LESS` environment variable is not set, databow sets it to `SRX` for the pager, so a plain `less` from `PAGER` behaves the same way. To use your own `less` options, set `LESS` or `DATABOW_PAGER`.

### .quit

Exit databow (alias: `.exit`)

```
> .quit
```
