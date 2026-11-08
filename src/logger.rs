use std::{
    fs::{self, File, OpenOptions},
    io::Write as _,
    path::{Path, PathBuf},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use anyhow::{Context as _, Result};
use jiff::{Zoned, civil::Date};
use log::{Level, LevelFilter, Log, Metadata, Record};

const SENTINEL_SCOPE: &str = "logger";
const SENTINEL_MSG: &str = "#cindy-log-format:2";
const RETENTION_DAYS: i64 = 30;
const DEV: bool = cfg!(debug_assertions);

/// Writes Cindy's `main-YYYY-MM-DD.log`.
struct MainLog {
    dir: PathBuf,
    level: LevelFilter,
    day: Mutex<Option<(Date, File)>>,
    mirror: AtomicBool,
}

fn tag(level: Level) -> &'static str {
    match level {
        Level::Error => "ERROR",
        Level::Warn => "WARN ",
        Level::Info => "INFO ",
        Level::Debug => "DEBUG",
        Level::Trace => "TRACE",
    }
}

fn timestamp(now: &Zoned) -> String {
    now.strftime("%Y-%m-%dT%H:%M:%S%.3f%:z").to_string()
}

/// Continuation lines start with a space so none can pass for a record head.
fn record_line(now: &Zoned, tag: &str, scope: &str, msg: &str) -> String {
    format!(
        "[{}] [{tag}] [{scope}] {}\n",
        timestamp(now),
        msg.replace('\n', "\n ")
    )
}

fn parse_level(raw: &str) -> Option<LevelFilter> {
    match raw.trim().to_lowercase().as_str() {
        "trace" => Some(LevelFilter::Trace),
        "debug" => Some(LevelFilter::Debug),
        "info" => Some(LevelFilter::Info),
        "warn" => Some(LevelFilter::Warn),
        "error" => Some(LevelFilter::Error),
        "fatal" => Some(LevelFilter::Off),
        _ => None,
    }
}

impl MainLog {
    fn file_for(&self, date: Date) -> PathBuf {
        self.dir.join(format!("main-{date}.log"))
    }

    fn open(&self, now: &Zoned) -> Result<File> {
        let path = self.file_for(now.date());
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&path)
            .with_context(|| format!("open {}", path.display()))?;
        file.write_all(
            record_line(now, tag(Level::Info), SENTINEL_SCOPE, SENTINEL_MSG).as_bytes(),
        )?;
        self.remove_expired(now.date());
        Ok(file)
    }

    fn remove_expired(&self, today: Date) {
        let cutoff = today.saturating_sub(jiff::Span::new().days(RETENTION_DAYS));
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(error) => return eprintln!("[logger] read {}: {error}", self.dir.display()),
        };
        for entry in entries.flatten() {
            let name = entry.file_name();
            let Some(date) = name
                .to_str()
                .and_then(|n| n.strip_prefix("main-")?.strip_suffix(".log"))
                .and_then(|d| d.parse::<Date>().ok())
            else {
                continue;
            };
            if date > cutoff {
                continue;
            }
            if let Err(error) = fs::remove_file(entry.path()) {
                eprintln!("[logger] remove {}: {error}", entry.path().display());
            }
        }
    }

    fn ensure_day(&self, day: &mut Option<(Date, File)>, now: &Zoned) -> Result<()> {
        if day.as_ref().is_none_or(|(date, _)| *date != now.date()) {
            *day = Some((now.date(), self.open(now)?));
        }
        Ok(())
    }

    fn write(&self, now: &Zoned, line: &str) {
        let mut day = self.day.lock().expect("log lock poisoned");
        if let Err(error) = self.ensure_day(&mut day, now) {
            return eprintln!("[logger] {error:#}");
        }
        let (_, file) = day.as_mut().expect("opened above");
        if let Err(error) = file.write_all(line.as_bytes()) {
            eprintln!("[logger] write main log: {error}");
        }
    }

    fn emit(&self, level_tag: &str, scope: &str, msg: &str, to_stderr: bool) {
        let now = Zoned::now();
        let line = record_line(&now, level_tag, scope, msg);
        self.write(&now, &line);
        self.mirror(&now, &line, to_stderr);
    }

    /// Debug builds echo to the terminal until it goes away.
    fn mirror(&self, now: &Zoned, text: &str, to_stderr: bool) {
        if !DEV || !self.mirror.load(Ordering::Relaxed) {
            return;
        }
        let mirrored = if to_stderr {
            std::io::stderr().lock().write_all(text.as_bytes())
        } else {
            std::io::stdout().lock().write_all(text.as_bytes())
        };
        if let Err(error) = mirrored {
            self.mirror.store(false, Ordering::Relaxed);
            let note = format!("dev terminal mirror off: {error}");
            self.write(now, &record_line(now, tag(Level::Warn), "logger", &note));
        }
    }
}

impl Log for MainLog {
    fn enabled(&self, metadata: &Metadata) -> bool {
        metadata.level() <= self.level
    }

    fn log(&self, record: &Record) {
        let ours = record.module_path().is_some_and(|m| m.starts_with("cindy"));
        let limit = if ours {
            self.level
        } else {
            LevelFilter::Warn.min(self.level)
        };
        if record.level() > limit {
            return;
        }
        let msg = record.args().to_string();
        self.emit(
            tag(record.level()),
            record.target(),
            &msg,
            record.level() == Level::Error,
        );
    }

    fn flush(&self) {}
}

fn config_level(dir: &Path) -> Result<Option<String>> {
    let path = dir.join("log-config.json");
    let raw = match fs::read_to_string(&path) {
        Ok(raw) => raw,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error).with_context(|| format!("read {}", path.display())),
    };
    let value: serde_json::Value =
        serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
    let Some(fields) = value.as_object() else {
        anyhow::bail!("{} is not a JSON object", path.display());
    };
    match fields.get("level") {
        None => Ok(None),
        Some(serde_json::Value::String(level)) => Ok(Some(level.clone())),
        Some(other) => anyhow::bail!("{} level is not a string: {other}", path.display()),
    }
}

/// Logs go to the checkout in debug builds, as Cindy's dev does.
pub fn init(user_data: &Path) -> Result<()> {
    let dir = if DEV {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("logs")
    } else {
        user_data.join("logs")
    };
    fs::create_dir_all(&dir).with_context(|| format!("create {}", dir.display()))?;
    let config_dir = if DEV {
        dir.clone()
    } else {
        user_data.to_owned()
    };
    let mut ignored = Vec::new();
    let from_config = match config_level(&config_dir) {
        Ok(Some(raw)) => parse_level(&raw).or_else(|| {
            ignored.push(format!("log-config.json level {raw:?}"));
            None
        }),
        Ok(None) => None,
        Err(error) => {
            ignored.push(format!("{error:#}"));
            None
        }
    };
    let default = if DEV {
        LevelFilter::Trace
    } else {
        LevelFilter::Info
    };
    let from_env = std::env::var("LOG_LEVEL").ok().and_then(|raw| {
        parse_level(&raw).or_else(|| {
            ignored.push(format!("LOG_LEVEL {raw:?}"));
            None
        })
    });
    let level = from_env.or(from_config).unwrap_or(default);
    let logger: &'static MainLog = Box::leak(Box::new(MainLog {
        dir: dir.clone(),
        level,
        day: Mutex::new(None),
        mirror: AtomicBool::new(true),
    }));
    let mut day = logger.day.lock().expect("log lock poisoned");
    logger.ensure_day(&mut day, &Zoned::now())?;
    drop(day);
    log::set_logger(logger).context("logger already set")?;
    log::set_max_level(level);
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        logger.emit("FATAL", "process", &format!("panic: {info}"), true);
        default_hook(info);
    }));
    let note = format!("[logger] dev log dir: {}\n", dir.display());
    logger.mirror(&Zoned::now(), &note, false);
    log::info!(
        target: "logger",
        "=== App started === v{} level={} isDev={DEV}",
        env!("CARGO_PKG_VERSION"),
        level.as_str().to_lowercase()
    );
    for item in ignored {
        logger.emit(
            tag(Level::Warn),
            "logger",
            &format!("ignored invalid {item}"),
            false,
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests;
