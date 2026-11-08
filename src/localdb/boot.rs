//! `schemaMigrationBootIdentity.ts`: this kernel boot's id, `<platform>:<uuid>`.

use std::sync::OnceLock;

#[cfg(target_os = "linux")]
fn probe() -> Option<(&'static str, String)> {
    let id = std::fs::read_to_string("/proc/sys/kernel/random/boot_id").ok()?;
    Some(("linux", id))
}

#[cfg(target_os = "macos")]
fn probe() -> Option<(&'static str, String)> {
    let out = std::process::Command::new("/usr/sbin/sysctl")
        .args(["-n", "kern.bootsessionuuid"])
        .output()
        .ok()?;
    Some(("darwin", String::from_utf8(out.stdout).ok()?))
}

/// `SystemBootEnvironmentInformation` (90) starts with the kernel's boot GUID.
#[cfg(target_os = "windows")]
fn probe() -> Option<(&'static str, String)> {
    use windows::Wdk::System::SystemInformation::{
        NtQuerySystemInformation, SYSTEM_INFORMATION_CLASS,
    };
    let mut buffer = [0u8; 32];
    let mut returned = 0u32;
    // SAFETY: a 32-byte buffer, as Cindy's probe passes.
    let status = unsafe {
        NtQuerySystemInformation(
            SYSTEM_INFORMATION_CLASS(90),
            buffer.as_mut_ptr().cast(),
            buffer.len() as u32,
            &mut returned,
        )
    };
    if status.0 != 0 {
        return None;
    }
    let d1 = u32::from_le_bytes(buffer[0..4].try_into().ok()?);
    let d2 = u16::from_le_bytes(buffer[4..6].try_into().ok()?);
    let d3 = u16::from_le_bytes(buffer[6..8].try_into().ok()?);
    let d4 = &buffer[8..16];
    let tail: String = d4[2..].iter().map(|b| format!("{b:02x}")).collect();
    let guid = format!(
        "{d1:08x}-{d2:04x}-{d3:04x}-{:02x}{:02x}-{tail}",
        d4[0], d4[1]
    );
    Some(("win32", guid))
}

fn is_uuid(value: &str) -> bool {
    let groups: Vec<&str> = value.split('-').collect();
    let lengths = [8, 4, 4, 4, 12];
    groups.len() == 5
        && groups.iter().zip(lengths).all(|(g, n)| {
            g.len() == n
                && g.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        })
        && value != "00000000-0000-0000-0000-000000000000"
}

/// `readSchemaMigrationBootIdentity`: cached once found; `None` when the platform will not say.
pub fn identity() -> Option<&'static str> {
    static FOUND: OnceLock<String> = OnceLock::new();
    if let Some(found) = FOUND.get() {
        return Some(found);
    }
    let (platform, raw) = probe()?;
    let uuid = raw.trim().to_lowercase();
    is_uuid(&uuid).then(|| FOUND.get_or_init(|| format!("{platform}:{uuid}")).as_str())
}
