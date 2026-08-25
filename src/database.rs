// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

use crate::cli::ConnectionSource;
use adbc_core::options::{AdbcVersion, InfoCode, OptionDatabase, OptionValue};
use adbc_core::{Connection, Database, Driver, LOAD_FLAG_DEFAULT, Statement};
use adbc_driver_manager::profile::{
    ConnectionProfile, ConnectionProfileProvider, FilesystemProfileProvider, process_profile_value,
};
use adbc_driver_manager::{ManagedConnection, ManagedDatabase, ManagedDriver};
use arrow_array::cast::AsArray;
use arrow_array::{Array, RecordBatch, UnionArray};
use std::collections::HashSet;

/// Vendor (database product) metadata reported by an ADBC driver.
#[derive(Debug, Default)]
pub struct VendorInfo {
    pub name: Option<String>,
    pub version: Option<String>,
}

/// Query the connected driver for the database vendor name and version.
///
/// This is best-effort: any failure (the driver does not implement `get_info`,
/// returns unexpected data, etc.) yields a `VendorInfo` with `None` fields so
/// that callers can continue without the metadata.
pub fn get_vendor_info(connection: &impl Connection) -> VendorInfo {
    let codes = HashSet::from([InfoCode::VendorName, InfoCode::VendorVersion]);
    let reader = match connection.get_info(Some(codes)) {
        Ok(reader) => reader,
        Err(_) => return VendorInfo::default(),
    };
    let batches: Vec<RecordBatch> = match reader.collect::<Result<_, _>>() {
        Ok(batches) => batches,
        Err(_) => return VendorInfo::default(),
    };
    parse_vendor_info(&batches)
}

/// Extract the vendor name and version from the record batches returned by
/// [`Connection::get_info`].
///
/// The batches follow the ADBC `get_info` schema: an `info_name` (`u32`) column
/// and an `info_value` dense-union column. Vendor name/version are utf8 values
/// stored in the union's `string_value` child (type id 0).
fn parse_vendor_info(batches: &[RecordBatch]) -> VendorInfo {
    // Derive the info codes from the same enum used in the `get_info` request
    // so the parse cannot drift from the query.
    let vendor_name_code = u32::from(&InfoCode::VendorName);
    let vendor_version_code = u32::from(&InfoCode::VendorVersion);

    let mut info = VendorInfo::default();

    for batch in batches {
        let Some(info_names) = batch
            .column_by_name("info_name")
            .and_then(|col| col.as_primitive_opt::<arrow_array::types::UInt32Type>())
        else {
            continue;
        };
        let Some(info_values) = batch
            .column_by_name("info_value")
            .and_then(|col| col.as_any().downcast_ref::<UnionArray>())
        else {
            continue;
        };

        for row in 0..batch.num_rows() {
            if info_names.is_null(row) {
                continue;
            }
            let code = info_names.value(row);
            if code != vendor_name_code && code != vendor_version_code {
                continue;
            }

            let value = info_values.value(row);
            let Some(strings) = value.as_string_opt::<i32>() else {
                continue;
            };
            if strings.is_empty() || strings.is_null(0) {
                continue;
            }
            let text = strings.value(0).to_string();

            // The guard above ensures `code` is one of the two vendor codes,
            // so `else` unambiguously means the version.
            if code == vendor_name_code {
                info.name = Some(text);
            } else {
                info.version = Some(text);
            }
        }
    }

    info
}

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
    use adbc_core::schemas::GET_INFO_SCHEMA;
    use arrow_array::{StringArray, UInt32Array, UnionArray};
    use arrow_buffer::ScalarBuffer;
    use arrow_schema::DataType;
    use std::sync::Arc;

    /// Build a RecordBatch matching the ADBC `get_info` schema from a list of
    /// `(info_name, string_value)` pairs. `None` values are stored as null
    /// entries in the union's `string_value` child.
    fn make_info_batch(rows: &[(u32, Option<&str>)]) -> RecordBatch {
        let info_names: Vec<u32> = rows.iter().map(|(name, _)| *name).collect();
        let string_values: Vec<Option<&str>> = rows.iter().map(|(_, value)| *value).collect();

        // The `info_value` union child fields, per GET_INFO_SCHEMA. All rows use
        // the `string_value` child (type_id 0) in this helper.
        let DataType::Union(union_fields, _) = GET_INFO_SCHEMA.field(1).data_type().clone() else {
            panic!("info_value must be a union");
        };

        let string_child = Arc::new(StringArray::from(string_values.clone()));
        let children = union_fields
            .iter()
            .map(|(type_id, field)| -> arrow_array::ArrayRef {
                if type_id == 0 {
                    string_child.clone()
                } else {
                    // Empty child of the correct type for the unused union branches.
                    arrow_array::new_empty_array(field.data_type())
                }
            })
            .collect::<Vec<_>>();

        let type_ids = ScalarBuffer::from(vec![0i8; rows.len()]);
        let offsets = ScalarBuffer::from((0..rows.len() as i32).collect::<Vec<_>>());
        let union = UnionArray::try_new(union_fields, type_ids, Some(offsets), children).unwrap();

        RecordBatch::try_new(
            GET_INFO_SCHEMA.clone(),
            vec![Arc::new(UInt32Array::from(info_names)), Arc::new(union)],
        )
        .unwrap()
    }

    #[test]
    fn test_parse_vendor_info_name_and_version() {
        let batch = make_info_batch(&[(0, Some("DuckDB")), (1, Some("v1.1.0"))]);
        let info = parse_vendor_info(&[batch]);
        assert_eq!(info.name.as_deref(), Some("DuckDB"));
        assert_eq!(info.version.as_deref(), Some("v1.1.0"));
    }

    #[test]
    fn test_parse_vendor_info_name_only() {
        let batch = make_info_batch(&[(0, Some("PostgreSQL"))]);
        let info = parse_vendor_info(&[batch]);
        assert_eq!(info.name.as_deref(), Some("PostgreSQL"));
        assert_eq!(info.version, None);
    }

    #[test]
    fn test_parse_vendor_info_ignores_other_codes() {
        // 100 = DriverName, 101 = DriverVersion; not vendor fields.
        let batch = make_info_batch(&[
            (100, Some("ADBC DuckDB Driver")),
            (1, Some("v1.1.0")),
            (0, Some("DuckDB")),
        ]);
        let info = parse_vendor_info(&[batch]);
        assert_eq!(info.name.as_deref(), Some("DuckDB"));
        assert_eq!(info.version.as_deref(), Some("v1.1.0"));
    }

    #[test]
    fn test_parse_vendor_info_null_value() {
        let batch = make_info_batch(&[(0, Some("DuckDB")), (1, None)]);
        let info = parse_vendor_info(&[batch]);
        assert_eq!(info.name.as_deref(), Some("DuckDB"));
        assert_eq!(info.version, None);
    }

    #[test]
    fn test_parse_vendor_info_empty() {
        let info = parse_vendor_info(&[]);
        assert_eq!(info.name, None);
        assert_eq!(info.version, None);
    }

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
