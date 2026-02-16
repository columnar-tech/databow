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
- **Fast and lightweight** - Built in Rust for high performance and minimal resource usage

## Installation

Clone the repository and install the binary:

```sh
git clone https://github.com/columnar-tech/adbcli.git
cargo install --path adbcli
```

## Getting Started

Install the DuckDB ADBC driver with [dbc](https://docs.columnar.tech/dbc/):

```console
$ dbc install duckdb
```

### Interactive Usage

Connect to DuckDB (in-memory):

```console
$ adbcli --driver duckdb
```

Execute SQL queries:

```
〉CREATE TABLE penguins AS FROM 'https://blobs.duckdb.org/data/penguins.csv';
┌───────┐
│ Count │
╞═══════╡
│ 344   │
└───────┘
〉SELECT * FROM penguins LIMIT 5;
┌─────────┬───────────┬────────────────┬───────────────┬───────────────────┬─────────────┬────────┬──────┐
│ species │ island    │ bill_length_mm │ bill_depth_mm │ flipper_length_mm │ body_mass_g │ sex    │ year │
╞═════════╪═══════════╪════════════════╪═══════════════╪═══════════════════╪═════════════╪════════╪══════╡
│ Adelie  │ Torgersen │ 39.1           │ 18.7          │ 181               │ 3750        │ male   │ 2007 │
│ Adelie  │ Torgersen │ 39.5           │ 17.4          │ 186               │ 3800        │ female │ 2007 │
│ Adelie  │ Torgersen │ 40.3           │ 18            │ 195               │ 3250        │ female │ 2007 │
│ Adelie  │ Torgersen │ NA             │ NA            │ NA                │ NA          │ NA     │ 2007 │
│ Adelie  │ Torgersen │ 36.7           │ 19.3          │ 193               │ 3450        │ female │ 2007 │
└─────────┴───────────┴────────────────┴───────────────┴───────────────────┴─────────────┴────────┴──────┘
```

### Non-interactive Usage

Execute a query directly and exit:

```console
$ adbcli --driver duckdb --query "SELECT 2 AS favorite_num"
┌──────────────┐
│ favorite_num │
╞══════════════╡
│ 2            │
└──────────────┘
```

Execute a query from a file and exit:

```console
$ adbcli --driver duckdb --file select_example.sql
┌────────────┐
│ the_answer │
╞════════════╡
│ 42         │
└────────────┘
```

Execute a query from stdin and exit:

```console
$ echo "SELECT 'Emil' AS name" | adbcli --driver duckdb
┌──────┐
│ name │
╞══════╡
│ Emil │
└──────┘
```

## Reference

```console
$ adbcli --help
Query databases via ADBC

Usage: adbcli [OPTIONS] --driver <driver>

Options:
      --driver <driver>      Driver name
      --uri <uri>            Database uniform resource identifier
      --username <username>  Database user username
      --password <password>  Database user password
      --option <option>      Driver-specific database option
      --mode <mode>          Table display style [default: utf8_full_condensed]
      --query <query>        Execute query and exit
      --file <file>          Read and execute file and exit
  -h, --help                 Print help
  -V, --version              Print version
```

## License

This project is licensed under [Apache-2.0](LICENSE).
