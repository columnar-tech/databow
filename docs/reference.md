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
