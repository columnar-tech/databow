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
adbcli --driver duckdb
```

## --uri

Database uniform resource identifier

```sh
adbcli --driver mysql --uri root@tcp(localhost:33577)/test
```

## --username

Database user username

```sh
adbcli --driver flightsql --uri grpc://localhost:9408 --username root
```

## --password

Database user password

```sh
adbcli --driver flightsql --uri grpc://localhost:9408 --username root --password my_pwd
```

## --option

Driver-specific database option

```sh
adbcli --driver duckdb --option path=file.db
```

## --mode

Table display style

**Default:** `utf8_full_condensed`

All styling presets from [comfy_table](https://docs.rs/comfy-table/latest/comfy_table/presets/index.html) are supported:

| Mode | Description |
|------|-------------|
| `ascii_full` | ASCII table with all borders |
| `ascii_full_condensed` | ASCII table without row dividers |
| `ascii_borders_only` | ASCII table with outer borders only |
| `ascii_borders_only_condensed` | ASCII outer borders without row spacing |
| `ascii_horizontal_only` | ASCII with horizontal lines only |
| `ascii_markdown` | Markdown-compatible table format |
| `ascii_no_borders` | ASCII table without any borders |
| `utf8_full` | UTF-8 box drawing with all borders |
| `utf8_full_condensed` | UTF-8 box drawing without row dividers |
| `utf8_borders_only` | UTF-8 with outer borders only |
| `utf8_horizontal_only` | UTF-8 with horizontal lines only |
| `utf8_no_borders` | UTF-8 table without any borders |
| `nothing` | No borders or lines |

```sh
adbcli --driver duckdb --mode ascii_markdown
```

## --query

Execute query and exit

```sh
adbcli --driver duckdb --query "SELECT 42 AS the_answer"
```

## --file

Read and execute file and exit

```sh
adbcli --driver duckdb --file select_example.sql
```

## --help

Print the help message

```sh
adbcli --help
```

## --version

Print the version

```sh
adbcli --version
```
