// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::cli::ConnectionConfig;
use adbc_core::options::{AdbcVersion, OptionDatabase, OptionValue};
use adbc_core::{Connection, Database, Driver, LOAD_FLAG_DEFAULT, Statement};
use adbc_driver_manager::ManagedDriver;
use arrow_array::RecordBatch;

pub fn initialize_connection(config: ConnectionConfig) -> Result<impl Connection, String> {
    let mut driver = match ManagedDriver::load_from_name(
        &config.driver_name,
        None,
        AdbcVersion::default(),
        LOAD_FLAG_DEFAULT,
        None,
    ) {
        Ok(driver) => driver,
        Err(err) => return Err(format!("Failed to load driver: {err}")),
    };

    let mut options = Vec::new();

    if let Some(uri) = config.uri {
        options.push((OptionDatabase::Uri, OptionValue::String(uri)));
    }

    if let Some(username) = config.username {
        options.push((OptionDatabase::Username, OptionValue::String(username)));
    }

    if let Some(password) = config.password {
        options.push((OptionDatabase::Password, OptionValue::String(password)));
    }

    for (key, value) in config.options {
        options.push((OptionDatabase::Other(key), OptionValue::String(value)));
    }

    let database = match driver.new_database_with_opts(options) {
        Ok(database) => database,
        Err(err) => return Err(format!("Failed to create database handle: {err}")),
    };

    match database.new_connection() {
        Ok(connection) => Ok(connection),
        Err(err) => Err(format!("Failed to create connection: {err}")),
    }
}

pub fn execute_query(
    connection: &mut impl adbc_core::Connection,
    sql: &str,
) -> Result<Vec<RecordBatch>, String> {
    if sql.trim().is_empty() {
        return Ok(vec![]);
    }

    let mut statement = connection
        .new_statement()
        .map_err(|e| format!("Failed to create statement: {e}"))?;

    statement
        .set_sql_query(sql)
        .map_err(|e| format!("Failed to set SQL query: {e}"))?;

    let reader = statement
        .execute()
        .map_err(|e| format!("Failed to execute statement: {e}"))?;

    let batches: Vec<RecordBatch> = reader
        .collect::<Result<_, _>>()
        .map_err(|e| format!("Failed to collect batches: {e}"))?;

    Ok(batches)
}
