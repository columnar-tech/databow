---
icon: lucide/terminal
---

<!--
Copyright 2026 Columnar Technologies Inc.
SPDX-License-Identifier: Apache-2.0
-->

# Reference

## --driver

Driver name

```sh
databow --driver duckdb
```

## --uri

Database uniform resource identifier

```sh
databow --driver mysql --uri root@tcp(localhost:33577)/test
```

## --username

Database user username

```sh
databow --driver flightsql --uri grpc://localhost:9408 --username root
```

## --password

Database user password

```sh
databow --driver flightsql --uri grpc://localhost:9408 --username root --password my_pwd
```

## --option

Driver-specific database option

```sh
databow --driver duckdb --option path=file.db
```

## --mode

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

## --query

Execute query and exit

```sh
databow --driver duckdb --query "SELECT 42 AS the_answer"
```

## --file

Read and execute file and exit

```sh
databow --driver duckdb --file select_example.sql
```

## --output

Write result to file

```sh
databow --driver duckdb --query "SELECT 42 AS the_answer" --output result.json
```

The output format is inferred from the file extension:

| Extension       | Format    |
|-----------------|-----------|
| `.json`         | JSON      |
| `.csv`          | CSV       |
| `.arrow`, `.ipc`| Arrow IPC |

## --help

Print the help message

```sh
databow --help
```

## --version

Print the version

```sh
databow --version
```
