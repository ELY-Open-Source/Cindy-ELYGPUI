//! `attachmentSafety.ts` and `isSafetyDowngradedAttachment`: the files a chat only saves.

/// `DANGEROUS_ATTACHMENT_EXTS`.
const DANGEROUS: &[&str] = &[
    ".exe",
    ".msi",
    ".msp",
    ".msu",
    ".msix",
    ".msixbundle",
    ".appx",
    ".appxbundle",
    ".com",
    ".scr",
    ".cpl",
    ".pif",
    ".hta",
    ".js",
    ".jse",
    ".vbs",
    ".vbe",
    ".wsf",
    ".wsh",
    ".reg",
    ".application",
    ".appref-ms",
    ".gadget",
    ".dmg",
    ".pkg",
    ".app",
    ".command",
    ".bat",
    ".cmd",
    ".ps1",
    ".psm1",
    ".py",
    ".pyw",
    ".pl",
    ".rb",
    ".sh",
    ".bash",
    ".zsh",
    ".fish",
    ".run",
    ".appimage",
    ".deb",
    ".rpm",
    ".apk",
    ".jar",
    ".jnlp",
    ".lnk",
    ".scf",
    ".desktop",
    ".url",
];

/// `attachmentExtension`.
pub fn extension(value: &str) -> String {
    let base = value.rsplit(['\\', '/']).next().unwrap_or_default();
    match base.rfind('.') {
        Some(dot) if dot > 0 => base[dot..].to_lowercase(),
        _ => String::new(),
    }
}

/// `isDangerousAttachmentName`.
pub fn dangerous(name: &str) -> bool {
    DANGEROUS.contains(&extension(name).as_str())
}

/// `isSafetyDowngradedAttachment`.
pub fn downgraded(name: &str, path: &str) -> bool {
    dangerous(name) || dangerous(path) || (extension(path) == ".bin" && extension(name) != ".bin")
}

#[cfg(test)]
mod tests;
