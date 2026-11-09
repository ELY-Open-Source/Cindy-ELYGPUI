//! The reader's dependencies on the local database, as `registerMessageIpc` gives them.

use rusqlite::Connection;

use super::{Deps, Opts, Result, Row};
use crate::{
    history_view::{artifacts::with_artifacts, rows},
    localdb::search::mcp_app::TrustLabels,
};

/// One open database; `open` is its client epoch.
pub struct Db<'a> {
    pub db: &'a Connection,
    pub open: u64,
    pub labels: &'a TrustLabels,
}

impl Deps for Db<'_> {
    fn list(&mut self, session: &str, opts: &Opts) -> Result<Vec<Row>> {
        rows::list(self.db, session, opts, false, self.labels)
    }

    fn outlines(&self) -> bool {
        true
    }

    fn outline(&mut self, session: &str, opts: &Opts) -> Result<Vec<Row>> {
        let rows = rows::list(self.db, session, opts, true, self.labels)?;
        Ok(rows.into_iter().map(with_artifacts).collect())
    }

    fn hydrate(&mut self, session: &str, ids: &[String]) -> Result<Vec<Row>> {
        rows::hydrate(self.db, session, ids, self.labels)
    }

    fn revision(&mut self) -> Option<Result<String>> {
        Some(rows::revision(self.db, self.open))
    }

    fn validate(&mut self, session: &str, ids: &[String]) -> Option<Result<()>> {
        Some(rows::validate(self.db, session, ids))
    }

    fn anchor(&mut self, session: &str, id: &str, outline: bool) -> Result<Row> {
        let row = rows::anchor(self.db, session, id, outline, self.labels)?;
        Ok(if outline { with_artifacts(row) } else { row })
    }

    /// Turns run with 10.0.
    fn running(&self, _session: &str) -> bool {
        false
    }

    /// In-flight rows come with 10.0.
    fn live(&self, _session: &str) -> Vec<Row> {
        Vec::new()
    }
}
