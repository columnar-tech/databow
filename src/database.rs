// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::cli::ConnectionSource;
use adbc_core::options::{AdbcVersion, OptionDatabase, OptionValue};
use adbc_core::{Database, Driver, LOAD_FLAG_DEFAULT, Statement};
use adbc_driver_manager::profile::{
    ConnectionProfile, ConnectionProfileProvider, FilesystemProfileProvider, process_profile_value,
};
use adbc_driver_manager::{ManagedConnection, ManagedDatabase, ManagedDriver};
use arrow_array::RecordBatch;

pub fn initialize_connection(source: ConnectionSource) -> Result<ManagedConnection, String> {
    match source {
        ConnectionSource::Direct {
            driver_name,
            uri,
            username,
            password,
            options,
        } => initialize_direct_connection(driver_name, uri, username, password, options),
        ConnectionSource::Profile {
            profile,
            uri,
            username,
            password,
            options,
        } => initialize_profile_connection(profile, uri, username, password, options),
    }
}

fn initialize_direct_connection(
    driver_name: Option<String>,
    uri: Option<String>,
    username: Option<String>,
    password: Option<String>,
    options: Vec<(String, String)>,
) -> Result<ManagedConnection, String> {
    match driver_name {
        // Explicit driver: load it by name and hand it the URI as an option.
        Some(driver_name) => {
            let mut driver = ManagedDriver::load_from_name(
                &driver_name,
                None,
                AdbcVersion::default(),
                LOAD_FLAG_DEFAULT,
                None,
            )
            .map_err(|e| format!("Failed to load driver '{}': {}", driver_name, e))?;

            let db_options = build_database_options(uri, username, password, options);

            let database = driver
                .new_database_with_opts(db_options)
                .map_err(|e| format!("Failed to create database handle: {e}"))?;

            database
                .new_connection()
                .map_err(|e| format!("Failed to create connection: {e}"))
        }
        // No driver given: infer it from the URI scheme via the driver manager.
        None => {
            let uri = uri
                .ok_or_else(|| "Internal error: URI is required to infer the driver".to_string())?;

            // The URI itself selects the driver; the remaining CLI options are
            // layered on top (they override any options derived from the URI).
            let opts = build_database_options(None, username, password, options);

            let database = ManagedDatabase::from_uri_with_opts(
                &uri,
                None,
                AdbcVersion::default(),
                LOAD_FLAG_DEFAULT,
                None,
                opts,
            )
            .map_err(|e| format!("Failed to load driver from URI '{}': {}", uri, e))?;

            database
                .new_connection()
                .map_err(|e| format!("Failed to create connection: {e}"))
        }
    }
}

fn initialize_profile_connection(
    profile_name: String,
    uri_override: Option<String>,
    username_override: Option<String>,
    password_override: Option<String>,
    option_overrides: Vec<(String, String)>,
) -> Result<ManagedConnection, String> {
    // Load the profile using the filesystem provider
    // The provider searches standard locations for named profiles,
    // or treats the input as a file path if it has a .toml extension
    let provider = FilesystemProfileProvider::default();
    let profile = provider.get_profile(&profile_name).map_err(|e| {
        let lookup_type = if profile_name.contains('/') || profile_name.ends_with(".toml") {
            "file path"
        } else {
            "profile name"
        };
        format!(
            "Failed to load profile '{}' (interpreted as {}): {}",
            profile_name, lookup_type, e
        )
    })?;

    // Get the driver name from the profile
    let (driver_name, init_func) = profile.get_driver_name().map_err(|e| {
        format!(
            "Failed to get driver from profile '{}': {}",
            profile_name, e
        )
    })?;

    // Load the driver
    let mut driver = if let Some(init_func) = init_func {
        // Use static loading if an init function is provided
        ManagedDriver::load_static(init_func, AdbcVersion::default())
            .map_err(|e| format!("Failed to load driver '{}': {}", driver_name, e))?
    } else {
        // Load dynamically by name
        ManagedDriver::load_from_name(
            driver_name,
            None,
            AdbcVersion::default(),
            LOAD_FLAG_DEFAULT,
            None,
        )
        .map_err(|e| format!("Failed to load driver '{}': {}", driver_name, e))?
    };

    // Collect profile options, applying ADBC `{{ env_var(NAME) }}` substitution
    // on string values (matches the driver manager's `DriverLocator::Profile` path).
    let profile_options: Vec<(OptionDatabase, OptionValue)> = profile
        .get_options()
        .map_err(|e| {
            format!(
                "Failed to get options from profile '{}': {}",
                profile_name, e
            )
        })?
        .into_iter()
        .map(|(k, v)| -> Result<(OptionDatabase, OptionValue), String> {
            if let OptionValue::String(s) = v {
                let result = process_profile_value(&s).map_err(|e| {
                    format!(
                        "Failed to substitute env vars in profile '{}': {}",
                        profile_name, e
                    )
                })?;
                Ok((k, result))
            } else {
                Ok((k, v))
            }
        })
        .collect::<Result<Vec<_>, _>>()?;

    // Build override options from CLI (these take precedence)
    let override_options = build_database_options(
        uri_override,
        username_override,
        password_override,
        option_overrides,
    );

    // Merge options: profile first, then overrides (later values win)
    let merged_options = merge_options(profile_options, override_options);

    // Create database and connection
    let database = driver
        .new_database_with_opts(merged_options)
        .map_err(|e| format!("Failed to create database handle: {e}"))?;

    database
        .new_connection()
        .map_err(|e| format!("Failed to create connection: {e}"))
}

fn build_database_options(
    uri: Option<String>,
    username: Option<String>,
    password: Option<String>,
    options: Vec<(String, String)>,
) -> Vec<(OptionDatabase, OptionValue)> {
    let mut db_options = Vec::new();

    if let Some(uri) = uri {
        db_options.push((OptionDatabase::Uri, OptionValue::String(uri)));
    }

    if let Some(username) = username {
        db_options.push((OptionDatabase::Username, OptionValue::String(username)));
    }

    if let Some(password) = password {
        db_options.push((OptionDatabase::Password, OptionValue::String(password)));
    }

    for (key, value) in options {
        db_options.push((OptionDatabase::Other(key), OptionValue::String(value)));
    }

    db_options
}

/// Merge two sets of database options. Options from `overrides` take precedence.
fn merge_options(
    base: Vec<(OptionDatabase, OptionValue)>,
    overrides: Vec<(OptionDatabase, OptionValue)>,
) -> Vec<(OptionDatabase, OptionValue)> {
    // Simple approach: just append overrides after base
    // ADBC spec says later options override earlier ones
    let mut merged = base;
    merged.extend(overrides);
    merged
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_database_options_empty() {
        let options = build_database_options(None, None, None, vec![]);
        assert!(options.is_empty());
    }

    #[test]
    fn test_build_database_options_uri_only() {
        let options = build_database_options(Some("test://uri".to_string()), None, None, vec![]);
        assert_eq!(options.len(), 1);
        assert!(matches!(options[0].0, OptionDatabase::Uri));
        assert!(matches!(&options[0].1, OptionValue::String(s) if s == "test://uri"));
    }

    #[test]
    fn test_build_database_options_all_standard() {
        let options = build_database_options(
            Some("test://uri".to_string()),
            Some("user".to_string()),
            Some("pass".to_string()),
            vec![],
        );
        assert_eq!(options.len(), 3);
        assert!(matches!(options[0].0, OptionDatabase::Uri));
        assert!(matches!(options[1].0, OptionDatabase::Username));
        assert!(matches!(options[2].0, OptionDatabase::Password));
    }

    #[test]
    fn test_build_database_options_with_custom() {
        let options = build_database_options(
            None,
            None,
            None,
            vec![
                ("custom_key".to_string(), "custom_value".to_string()),
                ("another".to_string(), "option".to_string()),
            ],
        );
        assert_eq!(options.len(), 2);
        assert!(matches!(&options[0].0, OptionDatabase::Other(k) if k == "custom_key"));
        assert!(matches!(&options[1].0, OptionDatabase::Other(k) if k == "another"));
    }

    #[test]
    fn test_merge_options_empty_base() {
        let base = vec![];
        let overrides = vec![(OptionDatabase::Uri, OptionValue::String("uri".to_string()))];
        let merged = merge_options(base, overrides);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn test_merge_options_empty_overrides() {
        let base = vec![(OptionDatabase::Uri, OptionValue::String("uri".to_string()))];
        let overrides = vec![];
        let merged = merge_options(base, overrides);
        assert_eq!(merged.len(), 1);
    }

    #[test]
    fn test_merge_options_both_have_same_key() {
        // When both have the same option, overrides come after and should win
        let base = vec![(
            OptionDatabase::Uri,
            OptionValue::String("base_uri".to_string()),
        )];
        let overrides = vec![(
            OptionDatabase::Uri,
            OptionValue::String("override_uri".to_string()),
        )];
        let merged = merge_options(base, overrides);

        // Both should be present, with override second (ADBC applies in order)
        assert_eq!(merged.len(), 2);
        assert!(matches!(&merged[0].1, OptionValue::String(s) if s == "base_uri"));
        assert!(matches!(&merged[1].1, OptionValue::String(s) if s == "override_uri"));
    }

    #[test]
    fn test_merge_options_different_keys() {
        let base = vec![(
            OptionDatabase::Username,
            OptionValue::String("user".to_string()),
        )];
        let overrides = vec![(
            OptionDatabase::Password,
            OptionValue::String("pass".to_string()),
        )];
        let merged = merge_options(base, overrides);
        assert_eq!(merged.len(), 2);
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
