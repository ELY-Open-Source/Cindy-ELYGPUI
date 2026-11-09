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
    jobs: Jobs,
    /// Which open this is, so a reopened database is told apart.
    open: u64,
}

#[derive(Clone)]
enum Jobs {
    Thread(mpsc::Sender<Job>),
    /// The caller's thread runs each job: gpui's test scheduler refuses wakes from others.
    #[cfg(test)]
    Inline(std::sync::Arc<std::sync::Mutex<Connection>>),
}

fn next_open() -> u64 {
    static OPENS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    OPENS.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
}

impl PartialEq for Db {
    fn eq(&self, other: &Self) -> bool {
        self.open == other.open
    }
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
        Ok(Self {
            jobs: Jobs::Thread(jobs),
            open: next_open(),
        })
    }

    /// A database whose jobs run where they are called.
    #[cfg(test)]
    pub fn inline(connection: Connection) -> Self {
        Self {
            jobs: Jobs::Inline(std::sync::Arc::new(std::sync::Mutex::new(connection))),
            open: next_open(),
        }
    }

    /// Which open this is; the history reader keys its epoch on it.
    pub fn open(&self) -> u64 {
        self.open
    }

    /// Runs `job` on the database thread; the answer is dropped if the thread has stopped.
    pub fn call<R: Send + 'static>(
        &self,
        job: impl FnOnce(&Connection) -> R + Send + 'static,
    ) -> futures::channel::oneshot::Receiver<R> {
        let (reply, answer) = futures::channel::oneshot::channel();
        let job: Job = Box::new(move |db: &Connection| {
            let _ = reply.send(job(db));
        });
        match &self.jobs {
            Jobs::Thread(jobs) => {
                if jobs.send(job).is_err() {
                    log::error!(target: "localDb", "the database thread has stopped");
                }
            }
            #[cfg(test)]
            Jobs::Inline(db) => job(&db.lock().expect("an unpoisoned test database")),
        }
        answer
    }
}
