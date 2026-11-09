//! `loadOlderMessages` through the view.

use futures::{FutureExt, future::LocalBoxFuture};

use super::Store;

impl Store {
    /// `loadOlderMessages`: true only when the view's window moved back.
    pub fn load_older_messages(&self, id: &str) -> LocalBoxFuture<'static, Result<bool, String>> {
        let Some(view) = self.view(id).filter(|v| v.snapshot().ready) else {
            log::debug!(target: "chatStore", "older history of {id} waits for its view; raw pages come with 9.2f");
            return async { Ok(false) }.boxed_local();
        };
        let before = view.snapshot().next_cursor;
        let epoch = self.epoch(id);
        let run = view.refresh(true, false);
        let (store, id) = (self.clone(), id.to_owned());
        async move {
            run.await;
            if !store.is_view(&id, &view) {
                return Ok(false);
            }
            let snapshot = view.snapshot();
            if let Some(error) = snapshot.error {
                log::warn!(target: "chatStore", "older history of {id} failed: {error}");
                return Err(error);
            }
            Ok(store.is_view(&id, &view)
                && snapshot.ready
                && view.is_active()
                && store.epoch(&id) == epoch
                && snapshot.next_cursor != before)
        }
        .boxed_local()
    }
}
