---
icon: lucide/rocket
---

<!--
Copyright 2026 Columnar Technologies Inc.
SPDX-License-Identifier: Apache-2.0
-->

# adbcli

A command-line tool for querying databases via [ADBC](https://arrow.apache.org/adbc/current/index.html).

## Highlights

- **Multi-database support** - Connect to any database with a compatible ADBC driver
- **Interactive SQL shell** - Execute SQL queries with command history and intuitive navigation
- **Syntax highlighting** - SQL queries highlighted for improved readability
- **Formatted output** - Results displayed in clean, aligned tables with dynamic column width
- **File export** - Export query results to JSON, CSV, or Arrow IPC files
- **Fast and lightweight** - Built in Rust for high performance and minimal resource usage

## Getting Started

Install the DuckDB ADBC driver with [dbc](https://docs.columnar.tech/dbc/):

```sh
dbc install duckdb
```

!!! note

    adbcli can be used to query [many different databases](databases.md).
    This guide uses DuckDB for simplicity.

### Interactive Usage

Connect to DuckDB (in-memory):

```sh
adbcli --driver duckdb
```

Execute SQL queries:

```
> CREATE TABLE penguins AS FROM 'https://blobs.duckdb.org/data/penguins.csv';
┌───────┐
│ Count │
├───────┤
│ 344   │
└───────┘
> SELECT *
. FROM penguins
. LIMIT 5;
┌─────────┬───────────┬────────────────┬───────────────┬───────────────────┬─────────────┬────────┬──────┐
│ species │ island    │ bill_length_mm │ bill_depth_mm │ flipper_length_mm │ body_mass_g │ sex    │ year │
├─────────┼───────────┼────────────────┼───────────────┼───────────────────┼─────────────┼────────┼──────┤
│ Adelie  │ Torgersen │ 39.1           │ 18.7          │ 181               │ 3750        │ male   │ 2007 │
│ Adelie  │ Torgersen │ 39.5           │ 17.4          │ 186               │ 3800        │ female │ 2007 │
│ Adelie  │ Torgersen │ 40.3           │ 18            │ 195               │ 3250        │ female │ 2007 │
│ Adelie  │ Torgersen │ NA             │ NA            │ NA                │ NA          │ NA     │ 2007 │
│ Adelie  │ Torgersen │ 36.7           │ 19.3          │ 193               │ 3450        │ female │ 2007 │
└─────────┴───────────┴────────────────┴───────────────┴───────────────────┴─────────────┴────────┴──────┘
```

### Non-interactive Usage

Execute a query directly and exit:

```sh
adbcli --driver duckdb --query "SELECT 42 AS the_answer"
```

Execute a query from stdin and exit:

```sh
echo "SELECT 42 AS the_answer" | adbcli --driver duckdb
```

Execute a query from a file and exit:

```sh
adbcli --driver duckdb --file select_example.sql
```

Execute a query and output the result to a file:

```sh
adbcli --driver duckdb --query "SELECT 42 AS the_answer" --output result.json
adbcli --driver duckdb --query "SELECT 42 AS the_answer" --output result.csv
adbcli --driver duckdb --query "SELECT 42 AS the_answer" --output result.arrow
```
