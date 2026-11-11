//! Electron's Linux `ShowItemHelper`: the portal, else `FileManager1`, else the parent folder.

use std::{collections::HashMap, fs::File, path::Path, sync::OnceLock};

use anyhow::{Context as _, bail};
use zbus::{
    blocking::{Connection, Proxy, fdo::DBusProxy},
    names::{BusName, WellKnownName},
    zvariant::{Fd, OwnedObjectPath, OwnedValue, Value},
};

const PORTAL: &str = "org.freedesktop.portal.Desktop";
const FILE_MANAGER: &str = "org.freedesktop.FileManager1";
/// `DBUS_START_REPLY_SUCCESS` and `DBUS_START_REPLY_ALREADY_RUNNING`.
const STARTED: [u32; 2] = [1, 2];
const TARGET: &str = "show-item-in-folder";

#[derive(Clone, Copy, Debug)]
enum Api {
    Portal,
    FileManager,
    None,
}

/// Chosen once, as `api_type_` is.
static API: OnceLock<Api> = OnceLock::new();

/// `CheckForServiceAndStart`: a name with an owner, or one the bus could start.
fn running(bus: &Connection, name: &'static str) -> anyhow::Result<bool> {
    let dbus = DBusProxy::new(bus)?;
    if dbus.name_has_owner(BusName::try_from(name)?)? {
        return Ok(true);
    }
    let names = dbus.list_activatable_names()?;
    if !names.iter().any(|known| known.as_str() == name) {
        return Ok(false);
    }
    let reply = dbus.start_service_by_name(WellKnownName::try_from(name)?, 0)?;
    Ok(STARTED.contains(&reply))
}

/// A check the bus did not answer counts as a service that is not there.
fn has(bus: &Connection, name: &'static str) -> bool {
    running(bus, name).unwrap_or_else(|error| {
        log::warn!(target: TARGET, "no answer about {name}: {error:#}");
        false
    })
}

/// `OpenURI.OpenDirectory` with the file's descriptor, heard out to its `Response`.
fn portal(bus: &Connection, path: &str) -> anyhow::Result<()> {
    let file = File::open(path).context("the file did not open")?;
    let token = uuid::Uuid::new_v4().simple().to_string().to_uppercase();
    let sender = bus.unique_name().context("the bus gave no name")?;
    let sender = sender.trim_start_matches(':').replace('.', "_");
    let request = format!("/org/freedesktop/portal/desktop/request/{sender}/{token}");
    let listen = |at: &str| -> anyhow::Result<_> {
        let proxy = Proxy::new(bus, PORTAL, at.to_owned(), "org.freedesktop.portal.Request")?;
        Ok(proxy.receive_signal("Response")?)
    };
    // The signal is heard before the call, so no answer is missed.
    let mut heard = listen(&request)?;
    // No activation token: gpui hands none out.
    let options = HashMap::from([
        ("activation_token", Value::from("")),
        ("handle_token", Value::from(token.as_str())),
    ]);
    let reply = bus.call_method(
        Some(PORTAL),
        "/org/freedesktop/portal/desktop",
        Some("org.freedesktop.portal.OpenURI"),
        "OpenDirectory",
        &("", Fd::from(&file), options),
    )?;
    let made: OwnedObjectPath = reply.body().deserialize()?;
    if made.as_str() != request {
        // A portal before 0.9 names the request itself.
        heard = listen(made.as_str())?;
    }
    let answer = heard.next().context("the request ended with no response")?;
    let (code, _): (u32, HashMap<String, OwnedValue>) = answer.body().deserialize()?;
    if code != 0 {
        bail!("the portal answered {code}");
    }
    Ok(())
}

/// `FileManager1.ShowItems` with the path as Electron writes its URI, unescaped.
fn file_manager(bus: &Connection, path: &str) -> anyhow::Result<()> {
    bus.call_method(
        Some(FILE_MANAGER),
        "/org/freedesktop/FileManager1",
        Some(FILE_MANAGER),
        "ShowItems",
        &(vec![format!("file://{path}")], ""),
    )?;
    Ok(())
}

/// `OpenParentFolderFallback`: Electron's `OpenFolder` opens `.` from the folder's own parent.
fn parent(path: &str) {
    let dir = Path::new(path).parent().unwrap_or(Path::new("/"));
    if dir.is_dir() {
        let from = dir.parent().unwrap_or(Path::new("/"));
        crate::open_path::xdg_open(".".as_ref(), from);
    }
}

pub fn show(path: &str) {
    let bus = match Connection::session() {
        Ok(bus) => bus,
        Err(error) => {
            log::error!(target: TARGET, "no session bus: {error:#}");
            return parent(path);
        }
    };
    let api = *API.get_or_init(|| {
        let api = if has(&bus, PORTAL) {
            Api::Portal
        } else if has(&bus, FILE_MANAGER) {
            Api::FileManager
        } else {
            Api::None
        };
        log::info!(target: TARGET, "the file manager is reached through {api:?}");
        api
    });
    let shown = match api {
        Api::Portal => portal(&bus, path),
        Api::FileManager => file_manager(&bus, path),
        Api::None => return parent(path),
    };
    if let Err(error) = shown {
        log::warn!(target: TARGET, "{api:?} did not show the file: {error:#}");
        parent(path);
    }
}
