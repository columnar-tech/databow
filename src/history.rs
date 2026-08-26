// Copyright 2026 Columnar Technologies Inc.
// SPDX-License-Identifier: Apache-2.0

//! Persistent interactive history for databow.
//!
//! The history database uses reedline's schema directly.  The adapter is the
//! boundary between reedline's type-erased `History` trait and the typed
//! SQLite APIs, allowing connection identity to survive a save/load cycle.

use crate::cli::{self, ConnectionSource};
use chrono::Utc;
use reedline::{
    History, HistoryItem, HistoryItemExtraInfo, HistoryItemId, HistorySessionId, Result,
    SearchQuery, SqliteBackedHistory,
};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Map, Value};
use std::path::PathBuf;

const CONNECTION_KEY: &str = "connection";

/// Safe identity metadata for the connection associated with a history row.
///
/// This is deliberately separate from [`ConnectionSource`], which contains
/// credentials, URI values, and arbitrary driver options needed to connect.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ConnectionIdentity {
    Profile { profile: String },
    Direct { driver: Option<String> },
}

impl ConnectionIdentity {
    /// Build safe metadata before the connection source is consumed.
    pub fn from_source(source: &ConnectionSource) -> Self {
        match source {
            ConnectionSource::Profile { profile, .. } => Self::Profile {
                profile: profile.clone(),
            },
            ConnectionSource::Direct {
                driver_name, uri, ..
            } => Self::Direct {
                driver: driver_name.clone().or_else(|| {
                    uri.as_deref()
                        .and_then(cli::uri_driver_scheme)
                        .map(|scheme| scheme.to_ascii_lowercase())
                }),
            },
        }
    }
}

/// Lossless JSON object wrapper for databow history metadata.
///
/// Known fields are exposed through typed accessors while fields introduced by
/// newer versions are retained across updates.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HistoryExtraInfo {
    fields: Map<String, Value>,
}

impl HistoryExtraInfo {
    /// Return the connection identity when the stored value has the known shape.
    ///
    /// Keep this accessor available for future history consumers even though
    /// the initial REPL only writes the identity.
    #[allow(dead_code)]
    pub fn connection(&self) -> Option<ConnectionIdentity> {
        self.fields
            .get(CONNECTION_KEY)
            .and_then(|value| serde_json::from_value(value.clone()).ok())
    }

    pub fn set_connection(&mut self, identity: ConnectionIdentity) {
        self.fields.insert(
            CONNECTION_KEY.to_string(),
            serde_json::to_value(identity).expect("connection identity is serializable"),
        );
    }
}

impl Serialize for HistoryExtraInfo {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.fields.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for HistoryExtraInfo {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        match Value::deserialize(deserializer)? {
            Value::Object(fields) => Ok(Self { fields }),
            value => Err(serde::de::Error::custom(format!(
                "history metadata must be a JSON object, got {value}"
            ))),
        }
    }
}

impl HistoryItemExtraInfo for HistoryExtraInfo {}

/// A small adapter that adds connection identity to reedline history saves.
pub struct HistoryAdapter {
    backend: SqliteBackedHistory,
    identity: ConnectionIdentity,
    session: Option<HistorySessionId>,
}

impl HistoryAdapter {
    fn new(
        backend: SqliteBackedHistory,
        identity: ConnectionIdentity,
        session: Option<HistorySessionId>,
    ) -> Self {
        Self {
            backend,
            identity,
            session,
        }
    }

    pub fn persistent(
        path: PathBuf,
        identity: ConnectionIdentity,
        session: Option<HistorySessionId>,
    ) -> Result<Self> {
        Self::persistent_at(path, identity, session, Utc::now())
    }

    fn persistent_at(
        path: PathBuf,
        identity: ConnectionIdentity,
        session: Option<HistorySessionId>,
        session_timestamp: chrono::DateTime<Utc>,
    ) -> Result<Self> {
        Ok(Self::new(
            SqliteBackedHistory::with_file(path, session, Some(session_timestamp))?,
            identity,
            session,
        ))
    }

    pub fn in_memory(
        identity: ConnectionIdentity,
        session: Option<HistorySessionId>,
    ) -> Result<Self> {
        Ok(Self::new(
            SqliteBackedHistory::in_memory()?,
            identity,
            session,
        ))
    }

    /// Access the typed backend for tests and future metadata operations.
    #[cfg(test)]
    pub fn load_with_extra(&self, id: HistoryItemId) -> Result<HistoryItem<HistoryExtraInfo>> {
        self.backend.load_with_extra(id)
    }

    fn typed_item(&self, item: HistoryItem) -> HistoryItem<HistoryExtraInfo> {
        let existing: HistoryExtraInfo = item
            .id
            .and_then(|id| self.backend.load_with_extra(id).ok())
            .and_then(|loaded| loaded.more_info)
            .unwrap_or_default();
        let mut metadata = existing;
        metadata.set_connection(self.identity.clone());
        HistoryItem {
            id: item.id,
            start_timestamp: item.start_timestamp.or_else(|| Some(Utc::now())),
            command_line: item.command_line,
            session_id: item.session_id.or(self.session),
            hostname: item.hostname,
            cwd: item.cwd,
            duration: item.duration,
            exit_status: item.exit_status,
            more_info: Some(metadata),
        }
    }
}

fn erase_extra(item: HistoryItem<HistoryExtraInfo>) -> HistoryItem {
    HistoryItem {
        id: item.id,
        start_timestamp: item.start_timestamp,
        command_line: item.command_line,
        session_id: item.session_id,
        hostname: item.hostname,
        cwd: item.cwd,
        duration: item.duration,
        exit_status: item.exit_status,
        more_info: None,
    }
}

impl History for HistoryAdapter {
    fn save(&mut self, item: HistoryItem) -> Result<HistoryItem> {
        let original = item.clone();
        match self.backend.save_with_extra(self.typed_item(item)) {
            Ok(saved) => Ok(erase_extra(saved)),
            Err(error) => {
                // Reedline currently expects save to succeed and panics on an
                // error. Keep the REPL usable when persistent history fails.
                eprintln!("Warning: failed to save databow history: {error:?}");
                Ok(original)
            }
        }
    }

    fn load(&self, id: HistoryItemId) -> Result<HistoryItem> {
        self.backend.load(id)
    }

    fn count(&self, query: SearchQuery) -> Result<i64> {
        self.backend.count(query)
    }

    fn search(&self, query: SearchQuery) -> Result<Vec<HistoryItem>> {
        self.backend.search(query)
    }

    fn update(
        &mut self,
        id: HistoryItemId,
        updater: &dyn Fn(HistoryItem) -> HistoryItem,
    ) -> Result<()> {
        // reedline 0.51 preserves typed more_info on this type-erased path.
        self.backend.update(id, updater)
    }

    fn clear(&mut self) -> Result<()> {
        self.backend.clear()
    }

    fn delete(&mut self, id: HistoryItemId) -> Result<()> {
        self.backend.delete(id)
    }

    fn sync(&mut self) -> std::io::Result<()> {
        self.backend.sync()
    }

    fn session(&self) -> Option<HistorySessionId> {
        self.session
    }
}

/// Resolve the default OS-appropriate application data directory.
pub fn default_history_path() -> Option<PathBuf> {
    dirs::data_dir().map(|path| path.join("databow").join("history.sqlite3"))
}

/// Open persistent history, falling back to an in-memory SQLite backend.
pub fn initialize_history(
    identity: ConnectionIdentity,
    session: Option<HistorySessionId>,
) -> Box<dyn History> {
    initialize_history_at(default_history_path(), identity, session)
}

fn initialize_history_at(
    path: Option<PathBuf>,
    identity: ConnectionIdentity,
    session: Option<HistorySessionId>,
) -> Box<dyn History> {
    if let Some(path) = path {
        match HistoryAdapter::persistent(path.clone(), identity.clone(), session) {
            Ok(history) => return Box::new(history),
            Err(error) => eprintln!(
                "Warning: failed to initialize persistent databow history at {}: {error:?}; using in-memory history",
                path.display()
            ),
        }
    } else {
        eprintln!(
            "Warning: could not resolve a databow history directory; using in-memory history"
        );
    }

    match HistoryAdapter::in_memory(identity, session) {
        Ok(history) => Box::new(history),
        Err(error) => {
            eprintln!("Warning: failed to initialize in-memory databow history: {error:?}");
            // A backend failure is extraordinarily unlikely, but the REPL can
            // still operate with reedline's default in-memory implementation.
            Box::new(reedline::FileBackedHistory::default())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;
    use reedline::{History, SearchDirection, SearchFilter, SearchQuery};
    use tempfile::tempdir;

    fn item(command_line: &str) -> HistoryItem {
        HistoryItem::from_command_line(command_line)
    }

    fn profile_identity() -> ConnectionIdentity {
        ConnectionIdentity::Profile {
            profile: "production".to_string(),
        }
    }

    #[test]
    fn identity_excludes_credentials_options_and_uri_target() {
        let source = ConnectionSource::Direct {
            driver_name: None,
            uri: Some("postgresql://user:password@example/db?token=secret".to_string()),
            username: Some("user".to_string()),
            password: Some("password".to_string()),
            options: vec![("token".to_string(), "secret".to_string())],
        };
        let identity = ConnectionIdentity::from_source(&source);
        assert_eq!(
            identity,
            ConnectionIdentity::Direct {
                driver: Some("postgresql".to_string())
            }
        );
        let json = serde_json::to_string(&identity).unwrap();
        assert!(!json.contains("password"));
        assert!(!json.contains("secret"));
        assert!(!json.contains("example"));
    }

    #[test]
    fn direct_identity_is_persisted_without_sensitive_fields() {
        let source = ConnectionSource::Direct {
            driver_name: None,
            uri: Some("postgresql://user:password@example/db?token=secret".to_string()),
            username: Some("user".to_string()),
            password: Some("password".to_string()),
            options: vec![("token".to_string(), "secret".to_string())],
        };
        let identity = ConnectionIdentity::from_source(&source);
        let dir = tempdir().unwrap();
        let mut history =
            HistoryAdapter::persistent(dir.path().join("history.sqlite3"), identity, None).unwrap();
        let id = history.save(item("select 1")).unwrap().id.unwrap();
        let metadata = history.load_with_extra(id).unwrap().more_info.unwrap();
        let json = serde_json::to_value(metadata).unwrap();

        assert_eq!(
            json,
            serde_json::json!({
                "connection": {"kind": "direct", "driver": "postgresql"}
            })
        );
        let serialized = json.to_string();
        assert!(!serialized.contains("password"));
        assert!(!serialized.contains("secret"));
        assert!(!serialized.contains("example"));
    }

    #[test]
    fn profile_identity_stores_only_user_profile() {
        let source = ConnectionSource::Profile {
            profile: "production".to_string(),
            uri: Some("postgresql://user:password@host/db".to_string()),
            username: Some("user".to_string()),
            password: Some("password".to_string()),
            options: vec![("secret".to_string(), "value".to_string())],
        };
        assert_eq!(ConnectionIdentity::from_source(&source), profile_identity());
    }

    #[test]
    fn unknown_metadata_survives_connection_update() {
        let mut metadata: HistoryExtraInfo =
            serde_json::from_str(r#"{"connection":"future-format","future":{"value":1}}"#).unwrap();
        assert!(metadata.connection().is_none());
        metadata.set_connection(profile_identity());
        let value: Value = serde_json::to_value(metadata).unwrap();
        assert_eq!(value["future"]["value"], 1);
        assert_eq!(value["connection"]["kind"], "profile");
    }

    #[test]
    fn sqlite_history_round_trips_typed_connection_metadata() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.sqlite3");
        let session = reedline::Reedline::create_history_session_id();
        let mut history =
            HistoryAdapter::persistent(path.clone(), profile_identity(), session).unwrap();
        let saved = history.save(item("select 1")).unwrap();
        let id = saved.id.unwrap();
        let loaded = history.load_with_extra(id).unwrap();
        assert_eq!(loaded.command_line, "select 1");
        assert_eq!(loaded.session_id, session);
        assert!(loaded.start_timestamp.is_some());
        assert_eq!(
            loaded.more_info.unwrap().connection(),
            Some(profile_identity())
        );
        drop(history);

        let reopened = HistoryAdapter::persistent(path, profile_identity(), session).unwrap();
        let loaded = reopened.load_with_extra(id).unwrap();
        assert_eq!(
            loaded.more_info.unwrap().connection(),
            Some(profile_identity())
        );
    }

    #[test]
    fn timestamp_is_filled_without_overwriting_explicit_value() {
        let mut history = HistoryAdapter::in_memory(profile_identity(), None).unwrap();

        let saved = history.save(item("select 1")).unwrap();
        assert!(saved.start_timestamp.is_some());
        assert!(
            history
                .load_with_extra(saved.id.unwrap())
                .unwrap()
                .start_timestamp
                .is_some()
        );

        let explicit = Utc.timestamp_millis_opt(123_456).single().unwrap();
        let mut explicit_item = item("select 2");
        explicit_item.start_timestamp = Some(explicit);
        let explicit_saved = history.save(explicit_item).unwrap();
        assert_eq!(explicit_saved.start_timestamp, Some(explicit));
        assert_eq!(
            history
                .load_with_extra(explicit_saved.id.unwrap())
                .unwrap()
                .start_timestamp,
            Some(explicit)
        );
    }

    #[test]
    fn timestamped_rows_are_visible_to_a_later_session_cursor() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("history.sqlite3");
        let session_a = reedline::Reedline::create_history_session_id();
        let mut history_a =
            HistoryAdapter::persistent_at(path.clone(), profile_identity(), session_a, Utc::now())
                .unwrap();
        let saved = history_a.save(item("select from session_a")).unwrap();
        let saved_timestamp = saved.start_timestamp.expect("adapter fills timestamp");
        drop(history_a);

        let session_b = reedline::Reedline::create_history_session_id();
        let session_b_timestamp = saved_timestamp + chrono::Duration::milliseconds(1);
        let history_b =
            HistoryAdapter::persistent_at(path, profile_identity(), session_b, session_b_timestamp)
                .unwrap();
        // This is the initial backward query issued by reedline's
        // HistoryCursor::back for HistoryNavigationQuery::Normal. HistoryCursor
        // is not re-exported by reedline 0.51, so keep the regression test on
        // the exact public History query it constructs.
        let results = history_b
            .search(SearchQuery {
                start_id: None,
                end_id: None,
                start_time: None,
                end_time: None,
                direction: SearchDirection::Backward,
                limit: Some(1),
                filter: SearchFilter::anything(session_b),
            })
            .unwrap();
        assert_eq!(
            results.first().map(|item| item.command_line.as_str()),
            Some("select from session_a")
        );
    }

    #[test]
    fn multiple_saves_share_one_session() {
        let session = reedline::Reedline::create_history_session_id();
        let mut history = HistoryAdapter::in_memory(profile_identity(), session).unwrap();
        let first = history.save(item("select 1")).unwrap();
        let second = history.save(item("select 2")).unwrap();
        assert_eq!(first.session_id, session);
        assert_eq!(second.session_id, session);
        assert_eq!(history.session(), session);
        let query = SearchQuery::everything(SearchDirection::Forward, session);
        assert_eq!(history.count(query).unwrap(), 2);
    }

    #[test]
    fn persistent_open_failure_falls_back_to_memory() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("existing-directory");
        std::fs::create_dir(&path).unwrap();
        let session = reedline::Reedline::create_history_session_id();
        let mut history = initialize_history_at(Some(path), profile_identity(), session);
        let saved = history.save(item("select 1")).unwrap();
        assert!(saved.id.is_some());
        assert_eq!(saved.session_id, session);
        assert_eq!(history.session(), session);
        assert_eq!(history.count_all().unwrap(), 1);
    }

    #[test]
    fn default_history_path_has_expected_application_shape() {
        let Some(path) = default_history_path() else {
            return;
        };
        assert_eq!(
            path.file_name(),
            Some(std::ffi::OsStr::new("history.sqlite3"))
        );
        assert_eq!(
            path.parent().and_then(std::path::Path::file_name),
            Some(std::ffi::OsStr::new("databow"))
        );
    }

    #[test]
    fn type_erased_update_preserves_more_info() {
        let dir = tempdir().unwrap();
        let mut history = HistoryAdapter::persistent(
            dir.path().join("history.sqlite3"),
            profile_identity(),
            None,
        )
        .unwrap();
        let id = history.save(item("select 1")).unwrap().id.unwrap();
        history
            .update(id, &|mut item| {
                item.command_line = "select 2".to_string();
                item
            })
            .unwrap();
        let loaded = history.load_with_extra(id).unwrap();
        assert_eq!(loaded.command_line, "select 2");
        assert_eq!(
            loaded.more_info.unwrap().connection(),
            Some(profile_identity())
        );
    }

    #[test]
    fn saving_an_existing_item_preserves_unknown_metadata() {
        let dir = tempdir().unwrap();
        let mut history = HistoryAdapter::persistent(
            dir.path().join("history.sqlite3"),
            profile_identity(),
            None,
        )
        .unwrap();
        let id = history.save(item("select 1")).unwrap().id.unwrap();
        history
            .backend
            .update_with_extra::<HistoryExtraInfo>(id, &|mut item| {
                item.more_info.as_mut().unwrap().fields.insert(
                    "future_field".to_string(),
                    Value::String("preserve me".to_string()),
                );
                item
            })
            .unwrap();

        let mut updated = history.load(id).unwrap();
        updated.command_line = "select 2".to_string();
        history.save(updated).unwrap();

        let loaded = history.load_with_extra(id).unwrap();
        let metadata = loaded.more_info.unwrap();
        assert_eq!(
            metadata.fields.get("future_field"),
            Some(&Value::String("preserve me".to_string()))
        );
        assert_eq!(metadata.connection(), Some(profile_identity()));
    }
}
