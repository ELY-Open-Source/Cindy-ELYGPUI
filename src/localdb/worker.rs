//! Cindy's DB worker thread: one connection, its reader lease, and the daily `optimize`.

use std::{
    sync::mpsc,
    time::{Duration, Instant},
};

use rusqlite::Connection;

use super::lease::Lease;

/// `OPTIMIZE_INTERVAL_MS`.
const OPTIMIZE_EVERY: Duration = Duration::from_secs(24 * 60 * 60);

type Job = Box<dyn FnOnce(&Connection) + Send>;

/// A handle to the database thread; dropping the last one closes the connection and the lease.
#[derive(Clone)]
pub struct Db {
    _jobs: mpsc::Sender<Job>,
}

/// `runOptimize`: failures only log.
pub fn optimize(db: &Connection, mask: Option<u32>) {
    let started = Instant::now();
    let sql = mask.map_or("PRAGMA optimize".to_owned(), |m| {
        format!("PRAGMA optimize={m}")
    });
    match db.execute_batch(&sql) {
        Ok(()) => {
            log::info!(target: "localDb", "localDb.optimize.ok mask {mask:?} elapsedMs {}", started.elapsed().as_millis())
        }
        Err(error) => log::error!(target: "localDb", "PRAGMA optimize failed: {error}"),
    }
}

impl Db {
    /// Takes the opened connection and its lease onto a thread of their own.
    pub fn spawn(connection: Connection, lease: Lease) -> std::io::Result<Self> {
        let (jobs, inbox) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name("cindy-local-db".into())
            .spawn(move || {
                let _lease = lease;
                optimize(&connection, Some(0x10002));
                let mut next = Instant::now() + OPTIMIZE_EVERY;
                loop {
                    match inbox.recv_timeout(next.saturating_duration_since(Instant::now())) {
                        Ok(job) => job(&connection),
                        Err(mpsc::RecvTimeoutError::Timeout) => {
                            optimize(&connection, None);
                            next = Instant::now() + OPTIMIZE_EVERY;
                        }
                        Err(mpsc::RecvTimeoutError::Disconnected) => break,
                    }
                }
                log::info!(target: "localDb", "local database closed");
            })?;
        Ok(Self { _jobs: jobs })
    }
}
