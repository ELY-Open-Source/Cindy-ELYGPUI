//! Electron's macOS `OpenExternal`: the address to the application that takes it.

use block2::RcBlock;
use futures::channel::oneshot;
use objc2_app_kit::{NSRunningApplication, NSWorkspace, NSWorkspaceOpenConfiguration};
use objc2_foundation::{NSData, NSError, NSURL};
use std::cell::Cell;

/// The receiver answers once the system has opened the address, or failed to.
pub fn open(spec: &str) -> Result<oneshot::Receiver<bool>, &'static str> {
    // As `net::NSURLWithGURL`: the bytes as written, not NSURL's stricter string parse.
    let data = NSData::with_bytes(spec.as_bytes());
    let address = NSURL::absoluteURLWithDataRepresentation_relativeToURL(&data, None);
    let workspace = NSWorkspace::sharedWorkspace();
    if workspace.URLForApplicationToOpenURL(&address).is_none() {
        return Err("No application found to open URL");
    }
    let configuration = NSWorkspaceOpenConfiguration::configuration();
    configuration.setActivates(true);
    let (done, opened) = oneshot::channel();
    let done = Cell::new(Some(done));
    let handler = RcBlock::new(move |_: *mut NSRunningApplication, error: *mut NSError| {
        let unheard = done
            .take()
            .is_some_and(|done| done.send(error.is_null()).is_err());
        if unheard {
            log::warn!(target: "open-external", "the opener's caller went away");
        }
    });
    workspace.openURL_configuration_completionHandler(&address, &configuration, Some(&handler));
    Ok(opened)
}
