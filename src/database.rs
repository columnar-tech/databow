use crate::cli::DatabaseConfig;
use adbc_core::options::{AdbcVersion, OptionDatabase, OptionValue};
use adbc_core::{Connection, Database, Driver, LOAD_FLAG_DEFAULT};
use adbc_driver_manager::ManagedDriver;
use std::process::exit;

pub fn initialize_connection(config: DatabaseConfig) -> impl Connection {
    let mut driver = match ManagedDriver::load_from_name(
        &config.driver_name,
        None,
        AdbcVersion::default(),
        LOAD_FLAG_DEFAULT,
        None,
    ) {
        Ok(driver) => driver,
        Err(err) => {
            eprintln!("Failed to load driver: {err}");
            exit(1);
        }
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
        Err(err) => {
            eprintln!("Failed to create database handle: {err}");
            exit(1);
        }
    };

    match database.new_connection() {
        Ok(connection) => connection,
        Err(err) => {
            eprintln!("Failed to create connection: {err}");
            exit(1);
        }
    }
}
