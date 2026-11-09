//! `enterView` and `leaveView`: each mounted view counts; the last to leave pauses the mirror.

use gpui::App;

use super::{Store, reconcile::Opts};

impl Store {
    /// `enterView`; a view coming back reads again and hands its rows over.
    pub fn enter_view(&self, id: &str, cx: &App) {
        if let Some(entry) = self.0.borrow_mut().views.get_mut(id) {
            entry.mirror_paused = false;
        }
        let view = self.view(id);
        let resuming = view.as_ref().is_some_and(|v| !v.is_active());
        if let Some(view) = &view {
            view.set_active(true);
        }
        {
            let mut inner = self.0.borrow_mut();
            *inner.active.entry(id.to_owned()).or_default() += 1;
            inner.last_viewed.remove(id);
        }
        self.ensure_timer();
        if resuming {
            log::debug!(target: "chatStore", "history of {id} resumes");
            let opts = Opts {
                activation_read: true,
                ..Opts::default()
            };
            drop(self.reconcile(id, opts, &cx.to_async()));
        }
    }

    /// `leaveView`.
    pub fn leave_view(&self, id: &str) {
        {
            let mut inner = self.0.borrow_mut();
            let Some(count) = inner.active.get_mut(id) else {
                return;
            };
            if *count > 1 {
                *count -= 1;
                return;
            }
            inner.active.remove(id);
            if let Some(entry) = inner.views.get_mut(id) {
                entry.mirror_paused = true;
            }
        }
        log::debug!(target: "chatStore", "history of {id} left");
        if let Some(view) = self.view(id) {
            view.set_active(false);
        }
        self.note_left(id);
        self.trim_messages();
        self.soft_evict();
    }
}
