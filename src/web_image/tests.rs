//! The fetch against a loopback server, by `logs/review/9.6i3c2-blink-http.log`.

use std::{
    io::{BufRead as _, BufReader, Write as _},
    net::TcpListener,
    sync::{Arc, Mutex},
};

use reqwest_client::ReqwestClient;

use super::{WANTED, fetch, user_agent};

/// Answers Blink was asked (`9.6i3c2-r3-blink-headers.log`): status, head, body, loaded as SVG.
const HEADS: &[(u16, &str, &str, Option<bool>)] = &[
    (
        200,
        "Content-Type: image/svg+xml; name=\"a,b.svg\"\r\n",
        "svg",
        Some(true),
    ),
    (
        200,
        "Content-Type: image/svg+xml; name=\"a,b.svg\", text/plain\r\n",
        "svg",
        Some(false),
    ),
    (
        200,
        "Content-Type: text/plain, image/svg+xml; name=\"a,b\"\r\n",
        "svg",
        Some(true),
    ),
    (200, "Content-Type: image/svg+xml, ,\r\n", "svg", Some(true)),
    (
        200,
        "Content-Type: image/svg+xml, invalid\r\n",
        "svg",
        Some(true),
    ),
    (
        200,
        "Content-Type: image/svg+xml; name=caf\u{e9}.svg\r\n",
        "svg",
        Some(true),
    ),
    (
        200,
        "Content-Type: image/svg+xml\r\nContent-Type: \u{e9}\r\n",
        "svg",
        Some(true),
    ),
    (
        200,
        "Content-Type: image/svg+xml (note)\r\n",
        "svg",
        Some(true),
    ),
    (
        200,
        "Content-Type: image/svg+xml\r\nContent-Type: */*\r\n",
        "svg",
        Some(true),
    ),
    (
        200,
        "Content-Type: image/svg+xml\r\nContent-Type: text; a=b/c\r\n",
        "svg",
        Some(true),
    ),
    (
        200,
        "Content-Type: image/svg+xml; name=\"a\\\",text/plain\"\r\n",
        "svg",
        Some(true),
    ),
    (
        200,
        "Content-Type: text/plain; name=\"a, image/svg+xml\r\n",
        "svg",
        Some(false),
    ),
    (
        200,
        "Content-Type: image/svg+xml\r\nContent-Type: /\r\n",
        "svg",
        Some(false),
    ),
    (
        200,
        "Content-Type: image/svg+xml;,text/plain\r\n",
        "svg",
        Some(false),
    ),
    (
        200,
        "Cross-Origin-Resource-Policy: same-origin\r\nCross-Origin-Resource-Policy: \u{e9}\r\n",
        "png",
        Some(false),
    ),
    (
        200,
        "Cross-Origin-Resource-Policy: same-origin,\r\n",
        "png",
        Some(false),
    ),
    (
        200,
        "Cross-Origin-Resource-Policy: Same-Origin\r\n",
        "png",
        Some(false),
    ),
    (302, "Location:\r\n", "png", Some(false)),
    (302, "Location: \t\r\n", "png", Some(false)),
    (302, "Location:\r\nLocation:\r\n", "png", Some(false)),
    (302, "Location: /caf\u{e9}\r\n", "", Some(false)),
    (302, "Location: /ok\r\nLocation:\r\n", "png", None),
    (302, "Location: /ok\r\nLocation: /OK\r\n", "png", None),
    (302, "Location: /ok,/ok\r\n", "", Some(false)),
    (302, "Location: #part\r\n", "png", None),
    (200, "Location: /ok\r\nLocation: /svg\r\n", "png", None),
    (
        200,
        "Location: /ok\r\nLocation: /ok\r\n",
        "png",
        Some(false),
    ),
    (404, "Location: /a\r\nLocation: /b\r\n", "png", None),
    (
        200,
        "Content-Disposition: attachment; filename=a,b.png\r\n",
        "png",
        None,
    ),
    (
        200,
        "Content-Disposition: attachment; filename=\"a,b.png\"\r\n",
        "png",
        Some(false),
    ),
    (
        200,
        "Content-Disposition: inline\r\nContent-Disposition: attachment\r\n",
        "png",
        None,
    ),
    (
        200,
        "Content-Disposition: inline,inline\r\n",
        "png",
        Some(false),
    ),
    (
        200,
        "Content-Disposition: inline\r\nContent-Disposition:\r\n",
        "png",
        None,
    ),
];

/// A server of one thread: `/hops/<n>` redirects n times more, the other paths answer as named.
fn serve(seen: Arc<Mutex<Vec<String>>>) -> String {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    std::thread::spawn(move || {
        for stream in listener.incoming() {
            let mut stream = stream.unwrap();
            let mut lines = BufReader::new(stream.try_clone().unwrap()).lines();
            let first = lines.next().unwrap().unwrap();
            let head: Vec<String> = lines
                .map(Result::unwrap)
                .take_while(|line| !line.is_empty())
                .collect();
            seen.lock().unwrap().extend(head);
            let path = first.split(' ').nth(1).unwrap();
            let path = path.split('?').next().unwrap();
            let answer = |status: &str, headers: &str, body: &str| {
                let length = body.len();
                format!(
                    "HTTP/1.1 {status}\r\n{headers}Content-Length: {length}\r\nConnection: close\r\n\r\n{body}"
                )
            };
            let numbered = |prefix: &str| path.strip_prefix(prefix).map(|n| n.parse::<usize>());
            if let Some(Ok(nth)) = numbered("/head/") {
                let (status, head, body, _) = HEADS[nth];
                stream
                    .write_all(answer(&format!("{status} Probe"), head, body).as_bytes())
                    .unwrap();
                continue;
            }
            let reply = match path.strip_prefix("/hops/").map(|left| left.parse::<u32>()) {
                Some(Ok(0)) => answer("302 Found", "Location: /ok\r\n", ""),
                Some(Ok(left)) => answer(
                    "302 Found",
                    &format!("Location: /hops/{}\r\n", left - 1),
                    "",
                ),
                _ => match path {
                    "/ok" | "/caf%C3%A9" | "/ok,/ok" => {
                        answer("200 OK", "Content-Type: image/png\r\n", "png")
                    }
                    "/gone" => answer("404 Not Found", "Content-Type: text/html\r\n", "png"),
                    "/empty" => answer("200 OK", "Content-Type: image/png\r\n", ""),
                    "/svg" => answer(
                        "200 OK",
                        "Content-Type: Image/SVG+xml; charset=utf-8\r\n",
                        "svg",
                    ),
                    "/xml" => answer("200 OK", "Content-Type: text/xml\r\n", "svg"),
                    "/kept" => answer(
                        "200 OK",
                        "Cross-Origin-Resource-Policy: same-origin\r\n",
                        "png",
                    ),
                    "/site" => answer(
                        "200 OK",
                        "Cross-Origin-Resource-Policy: same-site\r\n",
                        "png",
                    ),
                    "/open" => answer(
                        "200 OK",
                        "Cross-Origin-Resource-Policy: cross-origin\r\n",
                        "png",
                    ),
                    "/data" => answer("302 Found", "Location: data:image/png,png\r\n", "png"),
                    "/file" => answer("302 Found", "Location: file:///etc/hostname\r\n", "png"),
                    "/broken" => answer("302 Found", "Location: http://[\r\n", "png"),
                    "/two" => answer("302 Found", "Location: /ok\r\nLocation: /svg\r\n", "png"),
                    "/twice" => answer("302 Found", "Location: /ok\r\nLocation: /ok\r\n", ""),
                    "/kept-hop" => {
                        let headers =
                            "Location: /ok\r\nCross-Origin-Resource-Policy: same-site\r\n";
                        answer("302 Found", headers, "")
                    }
                    "/login" => {
                        let to = format!(
                            "Location: http://u:p@{}/ok\r\n",
                            stream.local_addr().unwrap()
                        );
                        answer("302 Found", &to, "png")
                    }
                    "/port" => answer("302 Found", "Location: http://127.0.0.1:6000/ok\r\n", "png"),
                    "/reset" => answer("205 Reset Content", "Content-Type: image/png\r\n", "png"),
                    "/both" => {
                        let policy = "Cross-Origin-Resource-Policy: same-origin\r\n";
                        answer("200 OK", &format!("{policy}{policy}"), "png")
                    }
                    "/last" => {
                        let kinds =
                            "Content-Type: text/plain\r\nContent-Type: image/svg+xml\t;x=1\r\n";
                        answer("200 OK", kinds, "svg")
                    }
                    "/first" => answer(
                        "200 OK",
                        "Content-Type: image/svg+xml, text/plain\r\n",
                        "svg",
                    ),
                    "/a/b" => answer("200 OK", "", "png"),
                    "/away" => {
                        let to = format!(
                            "Location: http://localhost:{}/ok\r\n",
                            stream.local_addr().unwrap().port()
                        );
                        answer("302 Found", &to, "")
                    }
                    "/bare" => answer("302 Found", "", "png"),
                    other => panic!("no route {other}"),
                },
            };
            stream.write_all(reply.as_bytes()).unwrap();
        }
    });
    base
}

#[test]
fn a_web_image_is_fetched_as_blinks_is() {
    let seen = Arc::new(Mutex::new(Vec::new()));
    let base = serve(seen.clone());
    let client = Arc::new(ReqwestClient::user_agent(&user_agent()).unwrap());
    let get = |path: &str| {
        let fetched = futures::executor::block_on(fetch(client.clone(), &format!("{base}{path}")));
        fetched.map(|(bytes, vector)| (String::from_utf8(bytes).unwrap(), vector))
    };
    let (png, svg) = (("png".to_owned(), false), ("svg".to_owned(), true));
    assert_eq!(get("/ok").unwrap(), png);
    // A status is no failure while a body came.
    assert_eq!(get("/gone").unwrap(), png);
    assert_eq!(get("/bare").unwrap(), png);
    assert!(get("/empty").is_err());
    assert_eq!(get("/svg").unwrap(), svg);
    assert_eq!(get("/xml").unwrap(), ("svg".to_owned(), false));
    assert!(get("/kept").is_err());
    assert!(get("/site").is_err());
    assert_eq!(get("/open").unwrap(), png);
    // A redirect that may not be followed fails, whatever body it carries.
    for path in [
        "/data",
        "/file",
        "/broken",
        "/two",
        "/kept-hop",
        "/login",
        "/port",
    ] {
        assert!(get(path).is_err(), "{path}");
    }
    assert_eq!(get("/twice").unwrap(), png);
    assert!(get("/reset").is_err());
    // Two policies are none; the last type named is the answer's.
    assert_eq!(get("/both").unwrap(), png);
    assert_eq!(get("/last").unwrap(), svg);
    assert_eq!(get("/first").unwrap(), ("svg".to_owned(), false));
    assert_eq!(get("/a\\b#part").unwrap(), png);
    assert_eq!(get("/away?token=secret").unwrap(), png);
    // Twenty redirects are followed and the twenty-first is not.
    assert_eq!(get("/hops/19").unwrap(), png);
    assert!(get("/hops/20").is_err());
    for (nth, (_, head, body, loads)) in HEADS.iter().enumerate() {
        let got = get(&format!("/head/{nth}")).ok();
        let body = if body.is_empty() { "png" } else { body };
        let wanted = loads.map(|vector| (body.to_owned(), vector));
        assert_eq!(got, wanted, "{head}");
    }
    // An address is made canonical first; one with credentials or a restricted port is not asked.
    let port = base.rsplit(':').next().unwrap();
    let escaped = format!("http://%31%32%37.0.0.1:{port}/ok");
    assert!(futures::executor::block_on(fetch(client.clone(), &escaped)).is_ok());
    for refused in [
        format!("http://u:p@127.0.0.1:{port}/ok"),
        "http://127.0.0.1:6000/ok".into(),
    ] {
        assert!(futures::executor::block_on(fetch(client.clone(), &refused)).is_err());
    }
    // A failure names no address.
    let closed = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    let gone = format!(
        "http://{}/private.png?token=secret",
        closed.local_addr().unwrap()
    );
    drop(closed);
    let error = futures::executor::block_on(fetch(client.clone(), &gone)).unwrap_err();
    assert!(!format!("{error:#}").contains("private"), "{error:#}");
    let seen = seen.lock().unwrap();
    assert!(
        !seen
            .iter()
            .any(|head| head.to_lowercase().starts_with("referer"))
    );
    assert!(
        !seen
            .iter()
            .any(|head| head.to_lowercase().starts_with("authorization"))
    );
    let sent = |line: String| seen.iter().any(|head| head.eq_ignore_ascii_case(&line));
    assert!(sent(format!("accept: {WANTED}")));
    assert!(sent(format!("user-agent: {}", user_agent())));
}
