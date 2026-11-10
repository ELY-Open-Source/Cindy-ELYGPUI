//! `createRemoteHistoryView`'s local `page` and `details`: one reader per database, on its worker.

use std::{
    rc::Rc,
    sync::{Arc, Mutex},
    time::Instant,
};

use futures::{FutureExt, future::LocalBoxFuture};
use gpui::AsyncApp;

use super::{
    map_server_messages,
    merge::{Fields, Message},
};
use crate::{
    history_view::{
        Item, controller,
        reader::{DetailPage, Page, Reader, Ref, db::Db},
        rows,
    },
    i18n::Translator,
    localdb::{LocalDb, State, search::mcp_app::TrustLabels},
    rich_answer::js::{self, Js},
};

/// The reader `registerMessageIpc` makes, kept per database open.
#[derive(Default)]
struct Readers(Arc<Mutex<Option<(u64, Reader)>>>);

impl gpui::Global for Readers {}

/// One task's reads, as the renderer invokes `local-db:messages:view` and `work-details`.
pub struct Local {
    session: String,
    cx: AsyncApp,
}

impl Local {
    pub fn new(session: String, cx: AsyncApp) -> Self {
        Local { session, cx }
    }

    /// Runs `job` with this database's reader on the worker.
    fn read<R: Send + 'static>(
        &self,
        what: &'static str,
        job: impl FnOnce(&mut Reader, &mut Db, &str) -> rows::Result<R> + Send + 'static,
    ) -> LocalBoxFuture<'static, Result<R, String>> {
        let (cx, session) = (self.cx.clone(), self.session.clone());
        async move {
            // An invoke leaves its caller's stack, which may hold the app, before it reads.
            controller::settled(std::future::ready(())).await;
            let (db, labels, readers) = cx.update(|cx| {
                let State::Ready(db) = &cx.global::<LocalDb>().state else {
                    return Err("DbClient not ready".to_owned());
                };
                let labels = TrustLabels::new(cx.global::<Translator>());
                Ok((db.clone(), labels, cx.default_global::<Readers>().0.clone()))
            })?;
            let (open, started) = (db.open(), Instant::now());
            let task = session.clone();
            let answer = db.call(move |conn| {
                let mut slot = readers.lock().expect("an unpoisoned reader");
                if slot.as_ref().is_none_or(|(at, _)| *at != open) {
                    log::info!(target: "historyView", "reader for database open {open}");
                    *slot = Some((open, Reader::default()));
                }
                let (_, reader) = slot.as_mut().expect("a reader");
                let mut deps = Db {
                    db: conn,
                    open,
                    labels: &labels,
                };
                job(reader, &mut deps, &task).map_err(|e| e.to_string())
            });
            let result = answer
                .await
                .map_err(|_| "the database closed during the read".to_owned())?;
            let ms = started.elapsed().as_millis();
            match &result {
                Ok(_) => log::debug!(target: "historyView", "{what} {session} read in {ms}ms"),
                Err(error) => log::warn!(target: "historyView", "{what} {session} failed: {error}"),
            }
            result
        }
        .boxed_local()
    }
}

impl controller::Transport<Message> for Local {
    fn page(
        &self,
        before: Option<String>,
    ) -> LocalBoxFuture<'static, Result<Page<Message>, String>> {
        let read = self.read("page", move |reader, deps, session| {
            reader.page(deps, session, before.as_deref(), true)
        });
        async move {
            let page = read.await?;
            Ok(Page {
                items: items(page.items)?,
                has_more: page.has_more,
                next_cursor: page.next_cursor,
            })
        }
        .boxed_local()
    }

    fn details(
        &self,
        r: Ref,
        after: Option<String>,
    ) -> LocalBoxFuture<'static, Result<DetailPage<Message>, String>> {
        let read = self.read("details", move |reader, deps, session| {
            reader.details(deps, session, &r, after.as_deref())
        });
        async move {
            let page = read.await?;
            Ok(DetailPage {
                messages: map_rows(&page.messages)?,
                has_more: page.has_more,
                next_cursor: page.next_cursor,
            })
        }
        .boxed_local()
    }
}

/// `mapHistoryViewMessages` with the view's `mapRows`.
pub(crate) fn items(items: Vec<Item>) -> Result<Vec<Item<Message>>, String> {
    items
        .into_iter()
        .map(|item| {
            Ok(match item {
                Item::Messages {
                    key,
                    messages,
                    deferred,
                } => Item::Messages {
                    key,
                    messages: map_rows(&messages)?,
                    deferred,
                },
                Item::Work {
                    key,
                    summary,
                    children,
                } => Item::Work {
                    key,
                    summary,
                    children: children.map(self::items).transpose()?,
                },
            })
        })
        .collect()
}

/// `mapRows`: `{ ...row, id: row.id ?? row.clientId, createdAt: row.createdAt ?? '' }`.
pub(crate) fn map_rows(rows: &[Js]) -> Result<Vec<Message>, String> {
    let mapped = map_server_messages(rows)?;
    Ok(mapped
        .into_iter()
        .map(|row| {
            // The mapper writes `id` and `createdAt` as strings or not at all.
            let id = row.get("id").or_else(|| row.get("clientId")).cloned();
            let created = row.get("createdAt").cloned();
            let created = created.unwrap_or(Js::Str(String::new()));
            let mut fields: Fields = row
                .into_fields()
                .into_iter()
                .map(|(k, v)| (k, v.map(Rc::new)))
                .collect();
            fields.insert("id".into(), id.map(Rc::new));
            fields.insert("createdAt".into(), Some(Rc::new(created)));
            Rc::new(fields)
        })
        .collect())
}

fn text<'a>(m: &'a Message, key: &str) -> &'a str {
    super::merge::get(m, key).and_then(Js::str).unwrap_or("")
}

impl controller::Message for Message {
    fn id(&self) -> &str {
        text(self, "id")
    }

    fn client_id(&self) -> &str {
        text(self, "clientId")
    }

    fn created_at(&self) -> &str {
        text(self, "createdAt")
    }

    fn characters(&self) -> usize {
        let mut pending: Vec<&Js> = self.values().flatten().map(|v| &**v).collect();
        let mut size = 0;
        while let Some(value) = pending.pop() {
            match value {
                Js::Str(s) => size += js::utf16_len(s),
                Js::Arr(items) => pending.extend(items),
                Js::Obj(fields) => pending.extend(fields.values()),
                _ => {}
            }
        }
        size
    }
}

#[cfg(test)]
mod tests;
