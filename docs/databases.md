---
icon: lucide/database
---

<!--
Copyright 2026 Columnar Technologies Inc.
SPDX-License-Identifier: Apache-2.0
-->

# Databases

databow can connect to any database with a compatible ADBC driver.

## Amazon Redshift

Install the Amazon Redshift driver with dbc:

```sh
dbc install redshift
```

Connect to Amazon Redshift:

```sh
databow --driver redshift --uri postgresql://localhost:5439 --option redshift.cluster_type=redshift-serverless --option redshift.workgroup_name=<WORKGROUP_NAME> --option redshift.db_name=sample_data_dev
```

## Apache Doris

Install the Arrow Flight SQL driver with dbc:

```sh
dbc install flightsql
```

Connect to Apache Doris:

```sh
databow --driver flightsql --uri grpc://localhost:8070 --username root
```

## BigQuery

Install the BigQuery driver with dbc:

```sh
dbc install bigquery
```

Connect to BigQuery:

```sh
databow --driver bigquery --option adbc.bigquery.sql.project_id=my-gcp-project --option adbc.bigquery.sql.dataset_id=bigquery-public-data
```

## Citus

Install the PostgreSQL driver with dbc:

```sh
dbc install postgresql
```

Connect to Citus:

```sh
databow --driver postgresql --uri postgresql://postgres:password@localhost:5432/postgres
```

## Databricks

Install the PostgreSQL driver with dbc:

```sh
dbc install databricks
```

Connect to Databricks:

```sh
databow --driver databricks --uri databricks://token:<personal-access-token>@<server-hostname>:<port-number>/<http-path>
```

## Dremio

Install the Arrow Flight SQL driver with dbc:

```sh
dbc install flightsql
```

Connect to Dremio:

```sh
databow --driver flightsql --uri grpc+tcp://localhost:32010 --username admin --password password1
```

## DuckDB

Install the DuckDB driver with dbc:

```sh
dbc install duckdb
```

Connect to DuckDB (in-memory):

```sh
databow --driver duckdb
```

Connect to DuckDB (persistent):

```sh
databow --driver duckdb --option path=data.db
```

## GreptimeDB

Install the MySQL driver with dbc:

```sh
dbc install mysql
```

Connect to GreptimeDB:

```sh
databow --driver mysql --uri 'root:@tcp(localhost:4002)/public'
```

## MariaDB

Install the MySQL driver with dbc:

```sh
dbc install mysql
```

Connect to MariaDB:

```sh
databow --driver mysql --uri 'root:my-secret-pw@tcp(localhost:3306)/sys'
```

## Microsoft SQL Server

Install the Microsoft SQL Server driver with dbc:

```sh
dbc install mssql
```

Connect to Microsoft SQL Server:

```sh
databow --driver mssql --uri sqlserver://sa:pwd@localhost:1433?database=demo
```

## MotherDuck

Install the DuckDB driver with dbc:

```sh
dbc install duckdb
```

Connect to MotherDuck:

```sh
databow --driver duckdb --option path=md:sample_data
```

## MySQL

Install the MySQL driver with dbc:

```sh
dbc install mysql
```

Connect to MySQL:

```sh
databow --driver mysql --uri 'root:my-secret-pw@tcp(localhost:3306)/sys'
```

## Neon

Install the PostgreSQL driver with dbc:

```sh
dbc install postgresql
```

Connect to Neon:

```sh
databow --driver postgresql --uri postgresql://cloud_admin:cloud_admin@localhost:55433/postgres
```

## OceanBase Database

Install the MySQL driver with dbc:

```sh
dbc install mysql
```

Connect to OceanBase Database:

```sh
databow --driver mysql --uri 'root@tcp(localhost:2881)/oceanbase'
```

## Oracle Database

Install the Oracle Database driver with dbc:

```sh
dbc install oracle
```

Connect to Oracle Database:

```sh
databow --driver oracle --uri oracle://system:password@localhost:1521/FREEPDB1
```

## ParadeDB

Install the PostgreSQL driver with dbc:

```sh
dbc install postgresql
```

Connect to ParadeDB:

```sh
databow --driver postgresql --uri postgresql://postgres:password@localhost:5432/postgres
```

## PostgreSQL

Install the PostgreSQL driver with dbc:

```sh
dbc install postgresql
```

Connect to PostgreSQL:

```sh
databow --driver postgresql --uri postgresql://postgres:password@localhost:5432/postgres
```

## SingleStore

Install the MySQL driver with dbc:

```sh
dbc install mysql
```

Connect to SingleStore:

```sh
databow --driver mysql --uri 'root:YOUR_ROOT_PASSWORD@tcp(localhost:3306)/memsql'
```

## Snowflake

Install the Snowflake driver with dbc:

```sh
dbc install snowflake
```

Connect to Snowflake:

```sh
databow --driver snowflake --uri snowflake://user:pwd@myorg-account1/ANALYTICS_DB/SALES_DATA?warehouse=WH_XL&role=ANALYST
```

## SQLite

Install the SQLite driver with dbc:

```sh
dbc install sqlite
```

Connect to SQLite (in-memory):

```sh
databow --driver sqlite
```

Connect to SQLite (persistent):

```sh
databow --driver sqlite --uri data.db
```

## StarRocks

Install the Arrow Flight SQL driver with dbc:

```sh
dbc install flightsql
```

Connect to StarRocks:

```sh
databow --driver flightsql --uri grpc://localhost:9408 --username root
```

## Teradata

Install the Teradata driver with dbc:

```sh
dbc install teradata
```

Connect to Teradata:

```sh
databow --driver teradata --uri teradata://YOUR_USERNAME:YOUR_PASSWORD@YOUR_HOST:1025
```

## TiDB

Install the MySQL driver with dbc:

```sh
dbc install mysql
```

Connect to TiDB:

```sh
databow --driver mysql --uri 'root@tcp(localhost:4000)/test'
```

## TimescaleDB

Install the PostgreSQL driver with dbc:

```sh
dbc install postgresql
```

Connect to TimescaleDB:

```sh
databow --driver postgresql --uri postgresql://postgres:password@localhost:5432/postgres
```

## Trino

Install the Trino driver with dbc:

```sh
dbc install trino
```

Connect to Trino:

```sh
databow --driver postgresql --uri http://user@localhost:8080?catalog=tcph&schema=tiny
```

## Vitess

Install the MySQL driver with dbc:

```sh
dbc install mysql
```

Connect to Vitess:

```sh
databow --driver mysql --uri 'root@tcp(localhost:33577)/test'
```

## Yellowbrick

Install the PostgreSQL driver with dbc:

```sh
dbc install postgresql
```

Connect to Yellowbrick:

```sh
databow --driver postgresql --uri postgresql://ybdadmin:ybdadmin@localhost:5432/yellowbrick
```

## YugabyteDB

Install the PostgreSQL driver with dbc:

```sh
dbc install postgresql
```

Connect to YugabyteDB:

```sh
databow --driver postgresql --uri postgresql://yugabyte@localhost:5433/yugabyte
```
