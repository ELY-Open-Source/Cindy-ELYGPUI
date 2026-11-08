//! Whether a lock's owner still runs: `process.kill(pid, 0)`, then its start time from `ps`, or
//! from PowerShell's CIM query and WMIC on Windows.

use std::{
    process::{Command, Stdio},
    sync::LazyLock,
    thread,
    time::{Duration, Instant},
};

use regex::Regex;

use super::{fsops::now_ms, record::Full};

const PID_REUSE_TOLERANCE_MS: f64 = 2_000.0;
const COMMAND_TIMEOUT: Duration = Duration::from_secs(5);
/// Milliseconds from .NET's year 1 to 1970.
const TICKS_EPOCH_MS: f64 = 62_135_596_800_000.0;

/// When this process started, as `Date.now() - process.uptime()` estimates it.
static STARTED_MS: LazyLock<f64> = LazyLock::new(now_ms);

/// Fixes the start time; `main` calls it first.
pub fn note_process_start() {
    LazyLock::force(&STARTED_MS);
}

/// `estimateCurrentProcessIdentity`'s key.
pub fn estimate_self() -> String {
    format!("start-ms:{}", STARTED_MS.round() as i64)
}

/// `process.kill(pid, 0)`'s outcome.
#[derive(Debug, PartialEq, Eq)]
pub enum Kill {
    Alive,
    /// `ESRCH`.
    Missing,
    /// `EPERM` or `EACCES`.
    Denied,
    Other,
}

#[cfg(unix)]
pub fn kill0(pid: i64) -> Kill {
    let Ok(pid) = libc::pid_t::try_from(pid) else {
        return Kill::Other;
    };
    // SAFETY: signal 0 sends nothing; it only checks the pid.
    if unsafe { libc::kill(pid, 0) } == 0 {
        return Kill::Alive;
    }
    match std::io::Error::last_os_error().raw_os_error() {
        Some(libc::ESRCH) => Kill::Missing,
        Some(libc::EPERM | libc::EACCES) => Kill::Denied,
        _ => Kill::Other,
    }
}

/// libuv's `uv_kill` with signal 0: open the process, then ask whether it has exited.
#[cfg(windows)]
pub fn kill0(pid: i64) -> Kill {
    use windows::Win32::{
        Foundation::{CloseHandle, ERROR_ACCESS_DENIED, ERROR_INVALID_PARAMETER, STILL_ACTIVE},
        System::Threading::{
            GetExitCodeProcess, OpenProcess, PROCESS_QUERY_INFORMATION, PROCESS_SYNCHRONIZE,
            PROCESS_TERMINATE,
        },
    };
    let Ok(pid) = u32::try_from(pid) else {
        return Kill::Other;
    };
    let access = PROCESS_TERMINATE | PROCESS_QUERY_INFORMATION | PROCESS_SYNCHRONIZE;
    // SAFETY: the handle is closed below; the exit code is a local.
    unsafe {
        let handle = match OpenProcess(access, false, pid) {
            Ok(handle) => handle,
            Err(error) if error.code() == ERROR_INVALID_PARAMETER.to_hresult() => {
                return Kill::Missing;
            }
            Err(error) if error.code() == ERROR_ACCESS_DENIED.to_hresult() => return Kill::Denied,
            Err(_) => return Kill::Other,
        };
        let mut code = 0u32;
        let result = match GetExitCodeProcess(handle, &mut code) {
            Err(_) => Kill::Other,
            Ok(()) if code != STILL_ACTIVE.0 as u32 => Kill::Missing,
            Ok(()) => Kill::Alive,
        };
        if let Err(error) = CloseHandle(handle) {
            log::warn!(target: "cross-process-lock", "process handle close failed: {error}");
        }
        result
    }
}

/// `ProcessIdentity`.
#[derive(Debug, PartialEq)]
pub struct Identity {
    pub key: String,
    pub started_ms: f64,
}

/// `ProcessIdentityProbe`.
#[derive(Debug, PartialEq)]
pub enum Probe {
    Found(Identity),
    Missing,
    Unavailable,
}

/// `processIdentityFromStartMs`.
fn from_start_ms(started_ms: f64, key: Option<String>) -> Option<Identity> {
    if !started_ms.is_finite() || started_ms <= 0.0 {
        return None;
    }
    let started_ms = started_ms.round();
    Some(Identity {
        key: key.unwrap_or_else(|| format!("start-ms:{}", started_ms as i64)),
        started_ms,
    })
}

/// `isRecordOwnerActive`: only a definite answer frees a lock; anything unclear keeps it.
pub fn owner_active(record: &Full) -> bool {
    match kill0(record.pid) {
        Kill::Missing => return false,
        Kill::Denied => return true,
        Kill::Alive | Kill::Other => {}
    }
    let current = match probe(record.pid) {
        Probe::Missing => return false,
        Probe::Unavailable => return true,
        Probe::Found(current) => current,
    };
    match &record.identity {
        Some(recorded) => identities_match(recorded, &current),
        None => true,
    }
}

/// `processIdentitiesMatch`.
pub fn identities_match(recorded: &str, current: &Identity) -> bool {
    recorded == current.key
        || parse_recorded_start_ms(recorded)
            .is_some_and(|ms| (ms - current.started_ms).abs() <= PID_REUSE_TOLERANCE_MS)
}

static START_MS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"^start-ms:(\d+)$").expect("valid pattern"));
static TICKS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"^\d{15,}$").expect("valid pattern"));

/// `parseRecordedProcessStartMs`: `start-ms:N`, .NET ticks, or `ps`'s `lstart` text in UTC.
pub fn parse_recorded_start_ms(key: &str) -> Option<f64> {
    if let Some(c) = START_MS.captures(key) {
        let value: f64 = c[1].parse().ok()?;
        return (value > 0.0 && value.is_finite()).then_some(value);
    }
    if TICKS.is_match(key) {
        let value = key.parse::<f64>().ok()? / 10_000.0 - TICKS_EPOCH_MS;
        return (value.is_finite() && value > 0.0).then_some(value);
    }
    parse_lstart(key)
}

/// `Date.parse(\`${lstart} UTC\`)` for `ps -o lstart=` output, `Sun Nov 8 14:00:00 2026`.
fn parse_lstart(text: &str) -> Option<f64> {
    let civil = jiff::civil::DateTime::strptime("%a %b %e %H:%M:%S %Y", text).ok()?;
    let at = civil.to_zoned(jiff::tz::TimeZone::UTC).ok()?;
    Some(at.timestamp().as_millisecond() as f64)
}

/// `probeProcessIdentity`.
pub fn probe(pid: i64) -> Probe {
    if cfg!(windows) {
        return match powershell(pid) {
            Probe::Unavailable => wmic(pid),
            found => found,
        };
    }
    let Some(out) = run(Command::new("ps")
        .args(["-p", &pid.to_string(), "-o", "lstart="])
        .env("LC_ALL", "C")
        .env("TZ", "UTC0"))
    else {
        return Probe::Unavailable;
    };
    let value = out.split_whitespace().collect::<Vec<_>>().join(" ");
    if value.is_empty() {
        return Probe::Unavailable;
    }
    let Some(ms) = parse_lstart(&value) else {
        return Probe::Unavailable;
    };
    from_start_ms(ms, Some(value)).map_or(Probe::Unavailable, Probe::Found)
}

fn system_root() -> String {
    std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into())
}

/// `readWindowsIdentityWithPowershell`.
fn powershell(pid: i64) -> Probe {
    let exe = format!(
        r"{}\System32\WindowsPowerShell\v1.0\powershell.exe",
        system_root()
    );
    let script = format!(
        "$p = Get-CimInstance Win32_Process -Filter \"ProcessId = {pid}\" -ErrorAction Stop; \
         if ($null -eq $p) {{ 'MISSING' }} else {{ $p.CreationDate.ToUniversalTime().Ticks }}"
    );
    let args = [
        "-NoProfile",
        "-NonInteractive",
        "-WindowStyle",
        "Hidden",
        "-Command",
        &script,
    ];
    let Some(out) = run(hidden(Command::new(exe).args(args))) else {
        return Probe::Unavailable;
    };
    let value = out.trim();
    if value == "MISSING" {
        return Probe::Missing;
    }
    if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
        return Probe::Unavailable;
    }
    let Ok(ticks) = value.parse::<f64>() else {
        return Probe::Unavailable;
    };
    from_start_ms(ticks / 10_000.0 - TICKS_EPOCH_MS, None).map_or(Probe::Unavailable, Probe::Found)
}

static CREATION: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)CreationDate=(\d{14})\.(\d{1,6})([+-]\d{3})").expect("valid pattern")
});
static NO_INSTANCE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)^\s*No Instance\(s\) Available\.\s*$").expect("valid pattern")
});

/// `readWindowsIdentityWithWmic`.
fn wmic(pid: i64) -> Probe {
    let exe = format!(r"{}\System32\wbem\WMIC.exe", system_root());
    let filter = format!("ProcessId={pid}");
    let args = ["process", "where", &filter, "get", "CreationDate", "/value"];
    let Some(out) = run(hidden(Command::new(exe).args(args))) else {
        return Probe::Unavailable;
    };
    let Some(c) = CREATION.captures(&out) else {
        return if NO_INSTANCE.is_match(&out) {
            Probe::Missing
        } else {
            Probe::Unavailable
        };
    };
    wmic_start_ms(&c[1], &c[2], &c[3])
        .and_then(|ms| from_start_ms(ms, None))
        .map_or(Probe::Unavailable, Probe::Found)
}

/// `Date.UTC` of WMIC's local stamp, less its offset in minutes.
fn wmic_start_ms(stamp: &str, micros: &str, offset: &str) -> Option<f64> {
    let n = |range: std::ops::Range<usize>| stamp[range].parse::<i64>().ok();
    let millis = format!("{micros:0<6}")[..3].parse::<i64>().ok()?;
    let date = jiff::civil::date(n(0..4)? as i16, n(4..6)? as i8, n(6..8)? as i8);
    let time = jiff::civil::time(n(8..10)? as i8, n(10..12)? as i8, n(12..14)? as i8, 0);
    let utc = date
        .to_datetime(time)
        .to_zoned(jiff::tz::TimeZone::UTC)
        .ok()?;
    let offset: i64 = offset.parse().ok()?;
    Some((utc.timestamp().as_millisecond() + millis - offset * 60_000) as f64)
}

/// `windowsHide`: no console window.
fn hidden(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    std::os::windows::process::CommandExt::creation_flags(command, 0x0800_0000);
    command
}

/// `execFile` with a 5 s timeout: stdout of a zero exit, else `None`.
fn run(command: &mut Command) -> Option<String> {
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let deadline = Instant::now() + COMMAND_TIMEOUT;
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Ok(None) | Err(_) => {
                if let Err(error) = child.kill() {
                    log::warn!(target: "cross-process-lock", "a timed-out probe would not die: {error}");
                }
                if let Err(error) = child.wait() {
                    log::warn!(target: "cross-process-lock", "a killed probe was not reaped: {error}");
                }
                return None;
            }
        }
    }
    let out = child.wait_with_output().ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).into_owned())
}
