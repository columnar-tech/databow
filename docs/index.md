---
icon: lucide/rocket
---

# adbcli

A command-line tool for querying databases via [ADBC](https://arrow.apache.org/adbc/current/index.html).

## Getting Started

Install the DuckDB ADBC driver with [dbc](https://docs.columnar.tech/dbc/):

```sh
dbc install duckdb
```

Connect to DuckDB (in-memory):

```sh
adbcli --driver duckdb
```

Run SQL queries:

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
