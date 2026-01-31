---
icon: lucide/terminal
---

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
