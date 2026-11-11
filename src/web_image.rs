//! A web address's image as Blink's `<img>` fetches one (`logs/review/9.6i3c2-blink-http.log`).

use std::sync::Arc;

use anyhow::{Context as _, Result, anyhow, bail};
use futures::AsyncReadExt as _;
use gpui::http_client::{
    AsyncBody, HttpClient, HttpRequestExt as _, RedirectPolicy, Request, StatusCode, Url,
    http::{
        HeaderMap, HeaderValue,
        header::{ACCEPT, CONTENT_DISPOSITION, CONTENT_TYPE, LOCATION},
    },
};

#[cfg(test)]
mod tests;

/// Blink's `Accept` for an image.
const WANTED: &str = "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8";

/// Chromium's `HTTP_LWS`.
const LWS: [char; 2] = [' ', '\t'];

/// Chromium follows twenty redirects.
const REDIRECTS: u32 = 20;

/// Electron's user agent with Cindy's product token, on this platform.
pub fn user_agent() -> String {
    let platform = match (cfg!(windows), cfg!(target_os = "macos")) {
        (true, _) => "Windows NT 10.0; Win64; x64",
        (false, true) => "Macintosh; Intel Mac OS X 10_15_7",
        (false, false) => "X11; Linux x86_64",
    };
    let version = env!("CARGO_PKG_VERSION");
    format!(
        "Mozilla/5.0 ({platform}) AppleWebKit/537.36 (KHTML, like Gecko) Cindy/{version} \
         Chrome/148.0.7778.280 Electron/42.11.12 Safari/537.36"
    )
}

/// Chromium 148's `kRestrictedPorts` (`net/base/port_util.cc`).
const BARRED: &[u16] = &[
    0, 1, 7, 9, 11, 13, 15, 17, 19, 20, 21, 22, 23, 25, 37, 42, 43, 53, 69, 77, 79, 87, 95, 101,
    102, 103, 104, 109, 110, 111, 113, 115, 117, 119, 123, 135, 137, 139, 143, 161, 179, 389, 427,
    465, 512, 513, 514, 515, 526, 530, 531, 532, 540, 548, 554, 556, 563, 587, 601, 636, 989, 990,
    993, 995, 1719, 1720, 1723, 2049, 3659, 4045, 5060, 5061, 6000, 6566, 6665, 6666, 6667, 6668,
    6669, 6697, 10080,
];

/// An address Blink's image asks: `http` or `https`, no credentials, no restricted port.
fn asked(url: &Url) -> Result<()> {
    if !matches!(url.scheme(), "http" | "https") {
        bail!("no web address: {}", url.scheme());
    }
    if !url.username().is_empty() || url.password().is_some() {
        bail!("an address with credentials");
    }
    let port = url
        .port_or_known_default()
        .context("a web address has a port")?;
    if BARRED.contains(&port) {
        bail!("restricted port {port}");
    }
    Ok(())
}

/// A header's fields as Chromium keeps them: the raw bytes, one character each.
fn fields(headers: &HeaderMap, name: &str) -> Vec<String> {
    let raw = |field: &HeaderValue| -> String {
        let text: String = field.as_bytes().iter().map(|b| char::from(*b)).collect();
        text.trim_matches(LWS).to_owned()
    };
    headers.get_all(name).iter().map(raw).collect()
}

/// `HttpUtil::ValuesIterator`: a field's values, split at the commas outside double quotes.
fn values(field: &str) -> Vec<&str> {
    let (mut out, mut from) = (Vec::new(), 0);
    let (mut quoted, mut escaped) = (false, false);
    for (at, c) in field.char_indices() {
        match (quoted, escaped, c) {
            (true, true, _) => escaped = false,
            (true, false, '\\') => escaped = true,
            (_, false, '"') => quoted = !quoted,
            (false, _, ',') => {
                out.push(field[from..at].trim_matches(LWS));
                from = at + 1;
            }
            _ => {}
        }
    }
    out.push(field[from..].trim_matches(LWS));
    out
}

/// `ParseContentType`'s type of one value: none without a slash in it, none for `*/*`.
fn kind(value: &str) -> Option<&str> {
    let end = value.find([' ', '\t', ';', '(']).unwrap_or(value.len());
    let slash = value.find('/')?;
    (slash <= end && value != "*/*").then(|| &value[..end])
}

/// `HeadersContainMultipleCopiesOfField`: two values of a header differ.
fn differ<T: PartialEq>(values: &[T]) -> bool {
    values.windows(2).any(|pair| pair[0] != pair[1])
}

/// Where a redirect answer sends the fetch; none when it is no redirect (`IsRedirect`).
fn next(status: StatusCode, places: &[String], from: &Url) -> Result<Option<Url>> {
    if !matches!(status.as_u16(), 301 | 302 | 303 | 307 | 308) {
        return Ok(None);
    }
    let Some(place) = places.iter().find(|place| !place.is_empty()) else {
        return Ok(None);
    };
    // `EscapeNonASCII`: a byte past ASCII goes on as it came.
    let escape = |c: char| match c.is_ascii() {
        true => c.to_string(),
        false => format!("%{:02X}", u32::from(c)),
    };
    let place: String = place.chars().map(escape).collect();
    let to = from.join(&place);
    let mut to = to.map_err(|error| anyhow!("status {status}, no place read: {error}"))?;
    to.set_fragment(None);
    asked(&to)?;
    Ok(Some(to))
}

/// The bytes `url` answers with, whatever its status, and whether they are typed as SVG.
pub async fn fetch(client: Arc<dyn HttpClient>, url: &str) -> Result<(Vec<u8>, bool)> {
    let mut at = Url::parse(url).map_err(|error| anyhow!("no address read: {error}"))?;
    at.set_fragment(None);
    asked(&at)?;
    // Each hop is asked here: its answer is checked, and no `Referer` names the hop before.
    for hop in 0..=REDIRECTS {
        let request = Request::builder()
            .uri(at.as_str())
            .header(ACCEPT, WANTED)
            .follow_redirects(RedirectPolicy::NoFollow)
            .body(AsyncBody::default())?;
        let sent = client.send(request).await;
        // A client's error names the address; its root cause does not.
        let mut response = sent.map_err(|error| anyhow!("{}", error.root_cause()))?;
        let (status, headers) = (response.status(), response.headers());
        // Chromium refuses an answer that names two places or two dispositions.
        let places = fields(headers, LOCATION.as_str());
        let saved = fields(headers, CONTENT_DISPOSITION.as_str());
        let saved: Vec<&str> = saved.iter().flat_map(|field| values(field)).collect();
        if differ(&places) || differ(&saved) {
            bail!("status {status}, a header named twice differs");
        }
        // The message's page is no site of the web: either policy keeps the answer from it.
        let policy = fields(headers, "cross-origin-resource-policy").join(", ");
        if matches!(policy.as_str(), "same-origin" | "same-site") {
            bail!("status {status}, kept from other origins: {policy}");
        }
        if let Some(to) = next(status, &places, &at)? {
            log::info!(target: "sessionView", "a web image's hop {hop} answered {status}");
            at = to;
            continue;
        }
        if matches!(status.as_u16(), 204 | 205 | 304) {
            bail!("status {status} has no body");
        }
        // The last type read wins, as in `HttpResponseHeaders::GetMimeTypeAndCharset`.
        let kinds = fields(headers, CONTENT_TYPE.as_str());
        let kinds = kinds.iter().flat_map(|field| values(field));
        let named = kinds.filter_map(kind).next_back();
        let vector = named.is_some_and(|kind| kind.eq_ignore_ascii_case("image/svg+xml"));
        let mut bytes = Vec::new();
        let read = response.body_mut().read_to_end(&mut bytes).await;
        read.map_err(|error| anyhow!("status {status}, the body broke off: {}", error.kind()))?;
        if bytes.is_empty() {
            bail!("status {status}, no body");
        }
        log::info!(target: "sessionView", "a web image answered {status}: {} bytes", bytes.len());
        return Ok((bytes, vector));
    }
    bail!("more than {REDIRECTS} redirects")
}
