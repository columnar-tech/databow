# adbcli

A command-line tool for querying databases via [ADBC](https://arrow.apache.org/adbc/current/index.html).

## Installation

Clone the repository and change the working directory:

```sh
git clone https://github.com/columnar-tech/adbcli.git
cd adbcli
```

Build the binary with [Cargo](https://doc.rust-lang.org/cargo/):

```sh
cargo build --release
```

The binary will be built to `./target/release/adbcli`.

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
+-------+
| Count |
+-------+
| 344   |
+-------+
〉SELECT * FROM penguins LIMIT 5;
+---------+-----------+----------------+---------------+-------------------+-------------+--------+------+
| species | island    | bill_length_mm | bill_depth_mm | flipper_length_mm | body_mass_g | sex    | year |
+---------+-----------+----------------+---------------+-------------------+-------------+--------+------+
| Adelie  | Torgersen | 39.1           | 18.7          | 181               | 3750        | male   | 2007 |
| Adelie  | Torgersen | 39.5           | 17.4          | 186               | 3800        | female | 2007 |
| Adelie  | Torgersen | 40.3           | 18            | 195               | 3250        | female | 2007 |
| Adelie  | Torgersen | NA             | NA            | NA                | NA          | NA     | 2007 |
| Adelie  | Torgersen | 36.7           | 19.3          | 193               | 3450        | female | 2007 |
+---------+-----------+----------------+---------------+-------------------+-------------+--------+------+
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
  -h, --help                 Print help
  -V, --version              Print version
```

## License

This project is licensed under [Apache-2.0](LICENSE).
