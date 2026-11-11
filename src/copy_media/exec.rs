//! Node's `execFile` as `fileClipboard.ts` calls it: ten seconds, 64K of output, the error's message.

use std::{
    io::{ErrorKind, Read},
    process::{Child, Command, Stdio},
    sync::mpsc::{Sender, channel},
    thread::JoinHandle,
    time::{Duration, Instant},
};

const TARGET: &str = "media:copy-to-clipboard";
/// `maxBuffer`, in bytes of the decoded text.
const MAX: usize = 64 * 1024;

/// The bytes Node counts: each bad sequence as U+FFFD, a cut one at the end not yet.
fn decoded(mut bytes: &[u8]) -> usize {
    let mut count = 0;
    loop {
        match std::str::from_utf8(bytes) {
            Ok(text) => return count + text.len(),
            Err(error) => {
                let Some(bad) = error.error_len() else {
                    return count + error.valid_up_to();
                };
                count += error.valid_up_to() + char::REPLACEMENT_CHARACTER.len_utf8();
                bytes = &bytes[error.valid_up_to() + bad..];
            }
        }
    }
}

/// A pipe read to its end as text; past `maxBuffer` it says so on `over` and stops.
fn drain(
    mut pipe: impl Read + Send + 'static,
    name: &'static str,
    over: Sender<&'static str>,
) -> JoinHandle<String> {
    std::thread::spawn(move || {
        let (mut held, mut chunk) = (Vec::new(), [0; 8192]);
        loop {
            match pipe.read(&mut chunk) {
                // The end flushes a cut sequence as U+FFFD, which counts too.
                Ok(0) if String::from_utf8_lossy(&held).len() > MAX => {
                    _ = over.send(name);
                    break;
                }
                Ok(0) => break,
                Ok(n) => held.extend_from_slice(&chunk[..n]),
                Err(error) => {
                    log::warn!(target: TARGET, "{name} not read: {error}");
                    break;
                }
            }
            if decoded(&held) > MAX {
                // The receiver is gone once the run has its answer.
                _ = over.send(name);
                break;
            }
        }
        String::from_utf8_lossy(&held).into_owned()
    })
}

/// `SIGTERM`, as Node's kill sends; Windows ends the process.
fn stop(child: &mut Child) {
    #[cfg(unix)]
    // SAFETY: the child is not reaped yet, so the pid is still its own.
    let stopped = match unsafe { libc::kill(child.id() as libc::pid_t, libc::SIGTERM) } {
        0 => Ok(()),
        _ => Err(std::io::Error::last_os_error()),
    };
    #[cfg(not(unix))]
    let stopped = child.kill();
    if let Err(error) = stopped {
        log::warn!(target: TARGET, "the helper did not stop: {error}");
    }
}

/// Runs `file` and waits; `Err` holds what `error.message` would.
pub fn run(file: &str, args: &[&str], timeout: Duration) -> Result<(), String> {
    let mut command = Command::new(file);
    command.args(args).stdin(Stdio::null());
    command.stdout(Stdio::piped()).stderr(Stdio::piped());
    #[cfg(windows)]
    {
        // `windowsHide`: `CREATE_NO_WINDOW`.
        use std::os::windows::process::CommandExt as _;
        command.creation_flags(0x0800_0000);
    }
    let mut child = command.spawn().map_err(|error| match error.kind() {
        ErrorKind::NotFound => format!("spawn {file} ENOENT"),
        ErrorKind::PermissionDenied => format!("spawn {file} EACCES"),
        _ => format!("spawn {file}: {error}"),
    })?;
    let (over, overflowed) = channel();
    let out = child.stdout.take().expect("stdout is piped");
    let err = child.stderr.take().expect("stderr is piped");
    let out = drain(out, "stdout", over.clone());
    let err = drain(err, "stderr", over);
    let (deadline, mut full, mut stopped) = (Instant::now() + timeout, None, false);
    let status = loop {
        if let Some(status) = child.try_wait().map_err(|error| error.to_string())? {
            break status;
        }
        if full.is_none() {
            full = overflowed.try_recv().ok();
        }
        if !stopped && (full.is_some() || Instant::now() >= deadline) {
            stop(&mut child);
            stopped = true;
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let join = |pipe: JoinHandle<String>| pipe.join().expect("the reader does not panic");
    let (_, stderr) = (join(out), join(err));
    if let Some(name) = full.or(overflowed.try_recv().ok()) {
        return Err(format!("{name} maxBuffer length exceeded"));
    }
    if status.success() {
        return Ok(());
    }
    let command = [&[file], args].concat().join(" ");
    Err(format!("Command failed: {command}\n{stderr}"))
}
