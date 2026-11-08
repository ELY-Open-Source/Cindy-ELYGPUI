//! The lock's records as Cindy reads them: the advisory `{pid, nonce}`, the full gate record
//! and the gate's `.released` marker.

use std::{
    io,
    path::{Path, PathBuf},
    sync::LazyLock,
};

use regex::Regex;

use super::{fsops::missing, identity};
use crate::cindy_json::{self, Fields};

const RECORD_MAX_BYTES: u64 = 4 * 1024;

static NONCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$")
        .expect("valid pattern")
});

/// `AdvisoryLockRecord`.
#[derive(Clone, Debug, PartialEq)]
pub struct Advisory {
    pub pid: i64,
    pub nonce: Option<String>,
}

/// `isValidNonce`: a v4 UUID.
pub fn valid_nonce(value: &str) -> bool {
    NONCE.is_match(value)
}

/// `JSON.parse` of an object, numbers past f64 kept as Infinity.
fn object(bytes: &[u8]) -> Option<Fields> {
    cindy_json::object(&String::from_utf8_lossy(bytes)).ok()
}

/// A field as `typeof` sees it.
enum Field {
    Absent,
    Null,
    Text(String),
    Other,
}

fn field(fields: &Fields, key: &str) -> Field {
    match fields.get(key) {
        None => Field::Absent,
        Some(raw) if raw.get() == "null" => Field::Null,
        Some(raw) => cindy_json::string(Some(raw)).map_or(Field::Other, Field::Text),
    }
}

fn text(fields: &Fields, key: &str) -> Option<String> {
    match field(fields, key) {
        Field::Text(s) => Some(s),
        _ => None,
    }
}

/// `Number.isInteger(pid) && pid > 0`.
fn pid(fields: &Fields) -> Option<i64> {
    let n = cindy_json::number(fields.get("pid").map(|v| &**v))?;
    (n > 0.0 && n.fract() == 0.0 && n <= i64::MAX as f64).then_some(n as i64)
}

/// `readAdvisoryLockRecord`: `Ok(None)` for a missing or malformed file, `Err` when unreadable.
pub fn read_advisory(path: &Path) -> io::Result<Option<Advisory>> {
    let raw = match std::fs::read(path) {
        Ok(raw) => raw,
        Err(error) if missing(&error) => return Ok(None),
        Err(error) => return Err(error),
    };
    Ok(object(&raw).and_then(|value| {
        let pid = pid(&value)?;
        let nonce = text(&value, "nonce").filter(|n| !n.is_empty());
        Some(Advisory { pid, nonce })
    }))
}

/// `LockRecord`.
#[derive(Clone, Debug, PartialEq)]
pub struct Full {
    pub pid: i64,
    pub started_at: f64,
    pub nonce: Option<String>,
    pub identity: Option<String>,
    pub released: bool,
}

/// `ReadLockRecord`.
#[derive(Debug, PartialEq)]
pub enum Read {
    Missing,
    Malformed,
    Unreadable,
    Record(Full),
}

/// The bytes of a file inside its real directory, as `readBoundedFileNoFollow` reads them.
fn read_contained(path: &Path) -> Result<Option<Vec<u8>>, Read> {
    let (Some(dir), Some(name)) = (path.parent(), path.file_name()) else {
        return Err(Read::Unreadable);
    };
    let real: PathBuf = match std::fs::canonicalize(dir) {
        Ok(real) => real,
        Err(error) if missing(&error) => return Err(Read::Missing),
        Err(_) => return Err(Read::Unreadable),
    };
    match crate::bounded_read::read(&real.join(name), RECORD_MAX_BYTES) {
        Ok(bytes) => Ok(bytes),
        Err(error) => match error.downcast_ref::<io::Error>() {
            Some(io) if missing(io) => Err(Read::Missing),
            _ => Err(Read::Unreadable),
        },
    }
}

/// `readLockRecord`; `complete` also demands the nonce, identity and state.
pub fn read_full(path: &Path, complete: bool) -> Read {
    let bytes = match read_contained(path) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return Read::Unreadable,
        Err(read) => return read,
    };
    let Some(value) = object(&bytes) else {
        return Read::Malformed;
    };
    let Some(pid) = pid(&value) else {
        return Read::Malformed;
    };
    let started_at = cindy_json::number(value.get("startedAt").map(|v| &**v));
    let Some(started_at) = started_at.filter(|n| n.is_finite()) else {
        return Read::Malformed;
    };
    let (nonce, identity, state) = (
        field(&value, "nonce"),
        field(&value, "processStartIdentity"),
        field(&value, "state"),
    );
    let identity_ok = |v: &str| !v.is_empty() && identity::parse_recorded_start_ms(v).is_some();
    let state_ok = matches!(&state, Field::Text(s) if s == "held" || s == "released");
    if complete
        && !(matches!(&nonce, Field::Text(n) if valid_nonce(n))
            && matches!(&identity, Field::Text(i) if identity_ok(i))
            && state_ok)
    {
        return Read::Malformed;
    }
    let nullable_ok = |f: &Field, ok: &dyn Fn(&str) -> bool| match f {
        Field::Absent | Field::Null => true,
        Field::Text(s) => ok(s),
        Field::Other => false,
    };
    if !nullable_ok(&nonce, &|n| !n.is_empty())
        || !nullable_ok(&identity, &identity_ok)
        || (!matches!(state, Field::Absent) && !state_ok)
    {
        return Read::Malformed;
    }
    let text_of = |f: Field| match f {
        Field::Text(s) if !s.is_empty() => Some(s),
        _ => None,
    };
    let released = matches!(&state, Field::Text(s) if s == "released");
    Read::Record(Full {
        pid,
        started_at,
        nonce: text_of(nonce),
        identity: text_of(identity),
        released,
    })
}

/// `gateReleaseMarkerPath`.
pub fn marker_path(gate: &Path) -> PathBuf {
    super::fsops::append(gate, ".released")
}

/// `publishGateReleaseMarker`: `{gateFile, nonce}`, created once and synced.
pub fn publish_marker(gate: &Path, nonce: &str) -> io::Result<()> {
    use std::io::Write as _;
    let name = gate
        .file_name()
        .and_then(|n| n.to_str())
        .expect("gate names are ASCII");
    let created = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(marker_path(gate));
    match created {
        Ok(mut file) => {
            let text = serde_json::json!({ "gateFile": name, "nonce": nonce }).to_string();
            file.write_all(text.as_bytes())?;
            file.sync_all()
        }
        Err(error)
            if error.kind() == io::ErrorKind::AlreadyExists && valid_marker(gate, Some(nonce)) =>
        {
            Ok(())
        }
        Err(error) => Err(error),
    }
}

/// `hasValidGateReleaseMarker`: names this gate, with a valid nonce, the expected one if given.
pub fn valid_marker(gate: &Path, expected: Option<&str>) -> bool {
    let Ok(Some(bytes)) = read_contained(&marker_path(gate)) else {
        return false;
    };
    let Some(value) = object(&bytes) else {
        return false;
    };
    let name = gate.file_name().and_then(|n| n.to_str());
    let nonce = text(&value, "nonce");
    text(&value, "gateFile").as_deref() == name
        && nonce.as_deref().is_some_and(valid_nonce)
        && expected.is_none_or(|e| nonce.as_deref() == Some(e))
}
