---
icon: lucide/book-text
---

<!--
Copyright 2026 Columnar Technologies Inc.
SPDX-License-Identifier: Apache-2.0
-->

# Tutorial

This tutorial will walk you through databow usage.

## Drivers

databow connects to databases and query engines via [ADBC](https://arrow.apache.org/adbc/current/index.html) (Arrow Database Connectivity) drivers.

We recommend using the [dbc](https://docs.columnar.tech/dbc/) command-line tool to install drivers. dbc makes it easy to install the right drivers for your operating system and architecture, in the correct location where databow can automatically find them.

With dbc, installing a driver is as easy as:


```sh
dbc install duckdb
```

## Connection

There are two methods available for specifying the driver and database credentials.

### CLI Arguments

CLI arguments can be used to directly specify the driver, database URI, and credentials:

```sh
databow --driver duckdb --uri warehouse.db
```

In addition to the `--driver` and `--uri` arguments shown above, `--username`, `--password` and `--option` arguments are also available. See [Reference](/reference) for more information.

### Connection Profiles

[ADBC connection profiles](https://arrow.apache.org/adbc/current/format/connection_profiles.html) can be used to store driver and connection options in files. For example:

```toml title="~/.config/adbc/profiles/warehouse.toml"
profile_version = 1
driver = "duckdb"

[Options]
uri = "path/to/warehouse.db"
```

If a connection profile is in a [profile search location](https://arrow.apache.org/adbc/current/format/connection_profiles.html#profile-search-locations), just the profile name can be specified:

```sh
databow --profile warehouse
```

Alternatively, a file path can be provided:

```sh
databow --profile path/to/warehouse.toml
```

## Interactive Usage

Once the connection details have been specified, an interactive SQL shell starts:

```console
$ databow --profile warehouse
>
```

Enter a SQL query terminated with a semicolon and press the `Enter` key to run it:

```console
$ databow --profile warehouse
> CREATE TABLE IF NOT EXISTS penguins AS
. FROM read_csv('https://blobs.duckdb.org/data/penguins.csv', nullstr = 'NA');
┌───────┐
│ Count │
├───────┤
│ 344   │
└───────┘
> SELECT * FROM penguins LIMIT 5;
┌─────────┬───────────┬────────────────┬───────────────┬───────────────────┬─────────────┬────────┬──────┐
│ species │ island    │ bill_length_mm │ bill_depth_mm │ flipper_length_mm │ body_mass_g │ sex    │ year │
├─────────┼───────────┼────────────────┼───────────────┼───────────────────┼─────────────┼────────┼──────┤
│ Adelie  │ Torgersen │ 39.1           │ 18.7          │ 181               │ 3750        │ male   │ 2007 │
│ Adelie  │ Torgersen │ 39.5           │ 17.4          │ 186               │ 3800        │ female │ 2007 │
│ Adelie  │ Torgersen │ 40.3           │ 18.0          │ 195               │ 3250        │ female │ 2007 │
│ Adelie  │ Torgersen │                │               │                   │             │        │ 2007 │
│ Adelie  │ Torgersen │ 36.7           │ 19.3          │ 193               │ 3450        │ female │ 2007 │
└─────────┴───────────┴────────────────┴───────────────┴───────────────────┴─────────────┴────────┴──────┘
```

Query results are displayed in clean, aligned tables with dynamic column width. The [`--mode` argument](/reference/#-mode) can be used to specify the table display style:

```console
$ databow --profile warehouse --mode ascii-markdown
> SELECT species, COUNT(*)
. FROM penguins
. GROUP BY species;
| species   | count_star() |
|-----------|--------------|
| Chinstrap | 68           |
| Gentoo    | 124          |
| Adelie    | 152          |
```

## Non-interactive Usage

The [`--query` argument](/reference/#-query) can be used to execute a query and exit:

```console
$ databow --profile warehouse --query "SELECT AVG(body_mass_g) FROM penguins"
┌───────────────────┐
│ avg(body_mass_g)  │
├───────────────────┤
│ 4201.754385964912 │
└───────────────────┘
```

Queries can also be passed from standard input (stdin):

```console
$ echo "SELECT AVG(body_mass_g) FROM penguins" | databow --profile warehouse
┌───────────────────┐
│ avg(body_mass_g)  │
├───────────────────┤
│ 4201.754385964912 │
└───────────────────┘
```

Or from a file using the [`--file` argument](/reference/#-file):

```console
$ databow --profile warehouse --file query.sql
┌───────────────────┐
│ avg(body_mass_g)  │
├───────────────────┤
│ 4201.754385964912 │
└───────────────────┘
```

Instead of printing query results to stdout, the [`--output` argument](/reference/#-output) can be used to write results to JSON, JSON lines, CSV, Arrow IPC, Arrow IPC stream, or Parquet files:

```console
$ databow --profile warehouse --query "SELECT * FROM penguins" --output penguins.csv
$ cat penguins.csv
species,island,bill_length_mm,bill_depth_mm,flipper_length_mm,body_mass_g,sex,year
Adelie,Torgersen,39.1,18.7,181,3750,male,2007
Adelie,Torgersen,39.5,17.4,186,3800,female,2007
Adelie,Torgersen,40.3,18.0,195,3250,female,2007
Adelie,Torgersen,,,,,,2007
Adelie,Torgersen,36.7,19.3,193,3450,female,2007
...
```
