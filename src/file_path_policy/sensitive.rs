//! `buildSensitiveMediaBlocklist`: the system list with the roots of credentials and browsers.

use super::system_blocklist;
use crate::node_path::{posix, win32};

/// `process.platform` as the list reads it: Windows, macOS, or any other POSIX.
#[derive(Clone, Copy, PartialEq)]
pub enum Platform {
    Win32,
    Darwin,
    Other,
}

/// What the list is built from; `real` is `realpathSync`, `None` where it throws.
pub struct Host<'a> {
    pub platform: Platform,
    pub home: &'a str,
    pub cwd: &'a str,
    pub env: &'a dyn Fn(&str) -> Option<String>,
    pub real: &'a dyn Fn(&str) -> Option<String>,
}

/// `CREDENTIAL_HOME_DIRS`.
const HOME_DIRS: [&str; 6] = [".ssh", ".aws", ".gnupg", ".kube", ".docker", ".azure"];

/// `CREDENTIAL_HOME_FILES`.
const HOME_FILES: [&[&str]; 9] = [
    &[".npmrc"],
    &[".netrc"],
    &[".git-credentials"],
    &[".pypirc"],
    &[".cargo", "credentials.toml"],
    &[".cargo", "credentials"],
    &[".m2", "settings.xml"],
    &[".m2", "settings-security.xml"],
    &[".config", "containers", "auth.json"],
];

/// The folders and the files a credential tool's own variable names.
const REDIRECTS: [&str; 13] = [
    "CLOUDSDK_CONFIG",
    "GH_CONFIG_DIR",
    "GNUPGHOME",
    "DOCKER_CONFIG",
    "AZURE_CONFIG_DIR",
    "AWS_SHARED_CREDENTIALS_FILE",
    "AWS_CONFIG_FILE",
    "AWS_WEB_IDENTITY_TOKEN_FILE",
    "GOOGLE_APPLICATION_CREDENTIALS",
    "AZURE_FEDERATED_TOKEN_FILE",
    "NPM_CONFIG_USERCONFIG",
    "npm_config_userconfig",
    "REGISTRY_AUTH_FILE",
];

/// A browser's profile roots under macOS's `~/Library/Application Support`.
const MAC_PROFILES: [&[&str]; 16] = [
    &["Google", "Chrome"],
    &["Google", "Chrome Beta"],
    &["Google", "Chrome Dev"],
    &["Google", "Chrome Canary"],
    &["Google", "Chrome for Testing"],
    &["Chromium"],
    &["Firefox"],
    &["Microsoft Edge"],
    &["Microsoft Edge Beta"],
    &["Microsoft Edge Dev"],
    &["Microsoft Edge Canary"],
    &["BraveSoftware", "Brave-Browser"],
    &["BraveSoftware", "Brave-Browser-Beta"],
    &["com.operasoftware.Opera"],
    &["com.operasoftware.OperaGX"],
    &["Vivaldi"],
];

/// A Chromium browser's folders under a Linux config base.
const LINUX_CHANNELS: [&str; 14] = [
    "google-chrome",
    "google-chrome-beta",
    "google-chrome-unstable",
    "google-chrome-canary",
    "google-chrome-for-testing",
    "chromium",
    "microsoft-edge",
    "microsoft-edge-beta",
    "microsoft-edge-dev",
    "BraveSoftware/Brave-Browser",
    "BraveSoftware/Brave-Browser-Beta",
    "vivaldi",
    "opera",
    "opera-beta",
];

const FLATPAKS: [&str; 7] = [
    "org.mozilla.firefox",
    "org.chromium.Chromium",
    "com.google.Chrome",
    "com.microsoft.Edge",
    "com.brave.Browser",
    "com.opera.Opera",
    "com.vivaldi.Vivaldi",
];

/// The roots as written, before they are resolved.
struct Raw<'a> {
    host: &'a Host<'a>,
    list: Vec<String>,
}

impl Host<'_> {
    fn windows(&self) -> bool {
        self.platform == Platform::Win32
    }

    fn join(&self, parts: &[&str]) -> String {
        match self.windows() {
            true => win32::join(parts),
            false => posix::join(parts),
        }
    }

    fn resolve(&self, path: &str) -> String {
        match self.windows() {
            true => win32::resolve(path, self.cwd, self.env),
            false => posix::resolve(path, self.cwd),
        }
    }

    fn dirname<'p>(&self, path: &'p str) -> &'p str {
        match self.windows() {
            true => win32::dirname(path),
            false => posix::dirname(path),
        }
    }

    fn basename<'p>(&self, path: &'p str) -> &'p str {
        match self.windows() {
            true => win32::basename(path),
            false => posix::basename(path),
        }
    }

    /// A variable that is set and not empty, as JavaScript's truthiness reads it.
    fn set(&self, key: &str) -> Option<String> {
        (self.env)(key).filter(|value| !value.is_empty())
    }

    /// `realpathViaNearestAncestor`: a root that is not there, through its nearest real folder.
    fn through_ancestor(&self, resolved: &str) -> Option<String> {
        let mut ancestor = self.dirname(resolved).to_owned();
        let mut tail = vec![self.basename(resolved).to_owned()];
        loop {
            if let Some(real) = (self.real)(&ancestor) {
                let mut parts = vec![real.as_str()];
                parts.extend(tail.iter().map(String::as_str));
                let variant = self.join(&parts);
                return (variant != resolved).then_some(variant);
            }
            let parent = self.dirname(&ancestor).to_owned();
            if parent == ancestor {
                return None;
            }
            tail.insert(0, self.basename(&ancestor).to_owned());
            ancestor = parent;
        }
    }
}

impl Raw<'_> {
    fn under(&mut self, base: &str, names: &[&str]) {
        let parts: Vec<&str> = [base].into_iter().chain(names.iter().copied()).collect();
        self.list.push(self.host.join(&parts));
    }

    /// `pushHome`.
    fn home(&mut self, names: &[&str]) {
        if !self.host.home.is_empty() {
            self.under(self.host.home, names);
        }
    }

    /// `pushLocalAppData` and `pushRoamingAppData`: the variable's folder, else the home's.
    fn app_data(&mut self, key: &str, fallback: &str, names: &[&str]) {
        match self.host.set(key) {
            Some(base) => self.under(&base, names),
            None => {
                let parts: Vec<&str> = ["AppData", fallback]
                    .into_iter()
                    .chain(names.iter().copied())
                    .collect();
                self.home(&parts);
            }
        }
    }

    fn windows(&mut self) {
        let local = |raw: &mut Self, names: &[&str]| raw.app_data("LOCALAPPDATA", "Local", names);
        let roaming = |raw: &mut Self, names: &[&str]| raw.app_data("APPDATA", "Roaming", names);
        local(self, &["Chromium", "User Data"]);
        for channel in [
            "Chrome",
            "Chrome Beta",
            "Chrome Dev",
            "Chrome SxS",
            "Chrome for Testing",
        ] {
            local(self, &["Google", channel, "User Data"]);
        }
        for channel in ["Edge", "Edge Beta", "Edge Dev", "Edge SxS"] {
            local(self, &["Microsoft", channel, "User Data"]);
        }
        local(self, &["BraveSoftware", "Brave-Browser", "User Data"]);
        local(self, &["BraveSoftware", "Brave-Browser-Beta", "User Data"]);
        local(self, &["Vivaldi", "User Data"]);
        roaming(self, &["Opera Software", "Opera Stable"]);
        roaming(self, &["Opera Software", "Opera GX Stable"]);
        roaming(self, &["Mozilla", "Firefox"]);
        roaming(self, &["gcloud"]);
        roaming(self, &["GitHub CLI"]);
        for store in ["Credentials", "Protect", "Vault"] {
            roaming(self, &["Microsoft", store]);
            local(self, &["Microsoft", store]);
        }
    }

    fn posix(&mut self) {
        self.list
            .extend(["/var/root", "/var/db"].map(str::to_owned));
        let host = self.host;
        let base = host.set("XDG_CONFIG_HOME");
        let base =
            base.or_else(|| (!host.home.is_empty()).then(|| host.join(&[host.home, ".config"])));
        if let Some(base) = &base {
            self.under(base, &["gcloud"]);
        }
        self.home(&[".config", "gcloud"]);
        if let Some(base) = &base {
            self.under(base, &["gh"]);
        }
        if host.platform == Platform::Darwin {
            self.home(&["Library", "Keychains"]);
            self.home(&["Library", "Cookies"]);
            self.home(&["Library", "Safari"]);
            self.home(&["Library", "Containers", "com.apple.Safari"]);
            for profile in MAC_PROFILES {
                let parts: Vec<&str> = ["Library", "Application Support"]
                    .into_iter()
                    .chain(profile.iter().copied())
                    .collect();
                self.home(&parts);
            }
            self.list.push("/Library/Keychains".into());
            return;
        }
        self.home(&[".mozilla"]);
        self.home(&[".local", "share", "keyrings"]);
        self.home(&[".gnome2", "keyrings"]);
        // `chromeBases`: Chromium's own prefix, then the config base, each once.
        let chrome = host.set("CHROME_CONFIG_HOME");
        let second = base.filter(|base| chrome.as_ref() != Some(base));
        for base in chrome.into_iter().chain(second) {
            for channel in LINUX_CHANNELS {
                self.under(&base, &[channel]);
            }
        }
        self.list.extend(host.set("CHROME_USER_DATA_DIR"));
        for app in FLATPAKS {
            self.home(&[".var", "app", app]);
        }
        for snap in ["firefox", "chromium", "brave", "opera", "vivaldi"] {
            self.home(&["snap", snap]);
        }
    }
}

/// `buildSensitiveMediaBlocklist`: each root resolved, and as its real path reads.
pub fn blocklist(host: &Host) -> Vec<String> {
    let list = system_blocklist(host.windows(), host.env);
    let mut raw = Raw { host, list };
    for dir in HOME_DIRS {
        raw.home(&[dir]);
    }
    for file in HOME_FILES {
        raw.home(file);
    }
    raw.list
        .extend(REDIRECTS.iter().filter_map(|key| host.set(key)));
    if let Some(configs) = host.set("KUBECONFIG") {
        let delimiter = if host.windows() { ';' } else { ':' };
        let named = configs.split(delimiter).filter(|config| !config.is_empty());
        raw.list.extend(named.map(str::to_owned));
    }
    if let Some(cargo) = host.set("CARGO_HOME") {
        raw.under(&cargo, &["credentials.toml"]);
        raw.under(&cargo, &["credentials"]);
    }
    for key in ["XDG_RUNTIME_DIR", "XDG_CONFIG_HOME"] {
        if let Some(base) = host.set(key) {
            raw.under(&base, &["containers", "auth.json"]);
        }
    }
    match host.windows() {
        true => raw.windows(),
        false => raw.posix(),
    }
    let mut out: Vec<String> = Vec::new();
    let add = |out: &mut Vec<String>, path: String| {
        if !out.contains(&path) {
            out.push(path);
        }
    };
    for root in &raw.list {
        let resolved = host.resolve(root);
        let also = match (host.real)(&resolved) {
            Some(real) => Some(real).filter(|real| !real.is_empty()),
            None => host.through_ancestor(&resolved),
        };
        add(&mut out, resolved);
        if let Some(also) = also {
            add(&mut out, also);
        }
    }
    out
}
