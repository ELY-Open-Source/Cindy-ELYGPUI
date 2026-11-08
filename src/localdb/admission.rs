//! `checkMigrationCompatibility`: the database must match the reference commit, read only.

use std::path::Path;

use rusqlite::Connection;
use serde::Deserialize;

/// One migration of the reference commit (`MigrationRuntimeIdentity`).
#[derive(Deserialize, Debug)]
pub struct Identity {
    pub seq: i64,
    #[serde(rename = "fileName")]
    pub file_name: String,
    #[serde(rename = "sqlHash")]
    pub sql_hash: String,
    #[serde(rename = "scriptHash")]
    pub script_hash: Option<String>,
}

/// The reference commit's migrations, from `scripts/sync-golden.mjs`.
pub fn reference() -> Vec<Identity> {
    let text = include_str!("../../resources/migration-manifest.json");
    serde_json::from_str(text).expect("the bundled migration manifest parses")
}

/// `MigrationCompatibilityIssue`.
#[derive(Debug, PartialEq)]
pub enum Issue {
    VersionBehind,
    VersionAhead,
    HistoryUnavailable(String),
    RuntimeManifestUnavailable(String),
    RuntimeManifestMismatch,
    EntryMissing {
        seq: i64,
        file_name: String,
    },
    EntryUnexpected {
        seq: i64,
        file_name: String,
    },
    EntryMismatch {
        seq: i64,
        expected: String,
        actual: String,
        hash_matches: bool,
    },
}

impl Issue {
    /// `MigrationCompatibilityIssue['kind']`.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::VersionBehind => "schema-version-behind",
            Self::VersionAhead => "schema-version-ahead",
            Self::HistoryUnavailable(_) => "history-unavailable",
            Self::RuntimeManifestUnavailable(_) => "runtime-manifest-unavailable",
            Self::RuntimeManifestMismatch => "runtime-manifest-mismatch",
            Self::EntryMissing { .. } => "history-entry-missing",
            Self::EntryUnexpected { .. } => "history-entry-unexpected",
            Self::EntryMismatch { .. } => "history-entry-mismatch",
        }
    }
}

#[derive(Debug)]
pub struct Report {
    pub database_version: i64,
    pub checkout_version: i64,
    pub issues: Vec<Issue>,
}

/// `readSchemaVersionStrict`: a canonical decimal string, or nothing.
fn schema_version(db: &Connection) -> Option<i64> {
    let sql = "SELECT value FROM migration_meta WHERE key='schema_version'";
    let value: rusqlite::types::Value = db.query_row(sql, [], |row| row.get(0)).ok()?;
    let rusqlite::types::Value::Text(text) = value else {
        return None;
    };
    let canonical =
        text == "0" || (!text.starts_with('0') && text.bytes().all(|b| b.is_ascii_digit()));
    if text.is_empty() || !canonical {
        return None;
    }
    text.parse::<i64>()
        .ok()
        .filter(|v| *v <= 9_007_199_254_740_991)
}

/// `sameRuntimeIdentity` with `===` on each field of one manifest entry.
fn same(entry: &serde_json::Value, m: &Identity, script: Option<&str>) -> bool {
    let text = |key: &str, want: &str| entry.get(key).and_then(|v| v.as_str()) == Some(want);
    let seq = entry.get("seq").and_then(serde_json::Value::as_f64) == Some(m.seq as f64);
    let script_matches = match (entry.get("scriptHash"), script) {
        (Some(serde_json::Value::Null), None) => true,
        (Some(serde_json::Value::String(s)), Some(want)) => s == want,
        _ => false,
    };
    seq && text("fileName", &m.file_name) && text("sqlHash", &m.sql_hash) && script_matches
}

/// `runtimeIdentityMatches`, with 0062's one-way companion fingerprint fix.
fn matches(entry: &serde_json::Value, m: &Identity) -> bool {
    const SQL: &str = "77b8741ac31c159eb422746c0165d102ad65693236c80d0ff055fd70cd43fe68";
    const OLD: &str = "0ea82003cac0419a4a483b0afc1743d6fdba0b50085104720d5b2561e721072d";
    const NEW: &str = "0a72ba2d89237b4b7322ffbbeb644c94e01be7d159851e220f51c03edfa80b78";
    let repair = m.seq == 62
        && m.file_name == "0062_flaky_mimic.sql"
        && m.sql_hash == SQL
        && m.script_hash.as_deref() == Some(NEW)
        && same(entry, m, Some(OLD));
    same(entry, m, m.script_hash.as_deref()) || repair
}

/// `<db>.migration-runtime.json` must list the reference identities in order.
fn runtime_manifest(db_file: &Path, reference: &[Identity]) -> Option<Issue> {
    let path = format!("{}.migration-runtime.json", db_file.display());
    let parsed = std::fs::read_to_string(&path)
        .map_err(|e| e.to_string())
        .and_then(|text| crate::cindy_json::roundtrip(&text).map_err(|e| e.to_string()));
    let manifest = match parsed {
        Ok(serde_json::Value::Object(manifest)) => manifest,
        Ok(_) => {
            return Some(Issue::RuntimeManifestUnavailable(
                "not an object".to_owned(),
            ));
        }
        Err(error) => return Some(Issue::RuntimeManifestUnavailable(error)),
    };
    let version = manifest.get("version").and_then(serde_json::Value::as_f64) == Some(1.0);
    let listed = manifest
        .get("migrations")
        .and_then(serde_json::Value::as_array);
    let ok = version
        && listed.is_some_and(|applied| {
            applied.len() == reference.len()
                && applied.iter().zip(reference).all(|(a, m)| matches(a, m))
        });
    (!ok).then_some(Issue::RuntimeManifestMismatch)
}

pub fn check(db: &Connection, db_file: &Path) -> Report {
    let reference = reference();
    let strict = schema_version(db);
    let database_version = strict.unwrap_or(-1);
    let checkout_version = reference.last().map_or(-1, |m| m.seq);
    let mut issues = Vec::new();
    if strict.is_none() {
        let error = "migration_meta.schema_version is missing or invalid".to_owned();
        issues.push(Issue::HistoryUnavailable(error));
    }
    if database_version < checkout_version {
        issues.push(Issue::VersionBehind);
    } else if database_version > checkout_version {
        issues.push(Issue::VersionAhead);
    }
    let rows = db
        .prepare("SELECT seq, file_name, content_hash FROM migration_history ORDER BY seq")
        .and_then(|mut statement| {
            let rows = statement.query_map([], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, Option<String>>(1)?,
                    row.get::<_, Option<String>>(2)?,
                ))
            })?;
            rows.collect::<rusqlite::Result<Vec<_>>>()
        });
    let history = match rows {
        Ok(history) => history,
        Err(error) => {
            issues.push(Issue::HistoryUnavailable(error.to_string()));
            return Report {
                database_version,
                checkout_version,
                issues,
            };
        }
    };
    for migration in &reference {
        let Some((_, file_name, hash)) = history.iter().find(|(seq, ..)| *seq == migration.seq)
        else {
            issues.push(Issue::EntryMissing {
                seq: migration.seq,
                file_name: migration.file_name.clone(),
            });
            continue;
        };
        let hash_matches = hash.as_deref() == Some(migration.sql_hash.as_str());
        if file_name.as_deref() != Some(migration.file_name.as_str()) || !hash_matches {
            issues.push(Issue::EntryMismatch {
                seq: migration.seq,
                expected: migration.file_name.clone(),
                actual: file_name.clone().unwrap_or_default(),
                hash_matches,
            });
        }
    }
    for (seq, file_name, _) in &history {
        if !reference.iter().any(|m| m.seq == *seq) {
            issues.push(Issue::EntryUnexpected {
                seq: *seq,
                file_name: file_name.clone().unwrap_or_default(),
            });
        }
    }
    issues.extend(runtime_manifest(db_file, &reference));
    Report {
        database_version,
        checkout_version,
        issues,
    }
}
