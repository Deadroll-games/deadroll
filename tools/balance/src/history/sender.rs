//! Sends a Run to the Apps Script web app, on a thread of its own so the UI never waits.
//!
//! Every request goes into a queue file first (`balance-log-queue.jsonl`, one request per line),
//! then the queue is sent oldest first. An entry leaves the queue only once the sheet has it, so
//! nothing is lost when the network is down and nothing is sent twice:
//! - the web app answers a POST with a redirect after it has written the rows, so a redirect means
//!   the rows are in; the reply is then read with a GET to the redirect target and the POST is
//!   never repeated (a GET that fails or answers something else only raises a warning);
//! - a plain 2xx must have the body `ok`; a network error, an error status or any other body keeps
//!   the entry in the queue and stops the flush, to be tried again with the next Run.
//!
//! The URL is a secret. It is read here, when it is needed, from `DEADROLL_SHEET_URL` or from
//! `private/balance-sheet-url.txt`, and is never stored, logged or put into a message: errors are
//! reduced to fixed texts.
use std::{
    fs,
    path::PathBuf,
    sync::{mpsc, Arc, Mutex},
    time::Duration,
};

use super::store::write_atomic;

/// Where the URL comes from.
#[derive(Clone)]
pub enum UrlSource {
    /// The environment variable, then the file in the `private/` folder.
    Default(PathBuf),
    /// A fixed URL (tests, with a local server).
    #[cfg(test)]
    Fixed(String),
}

impl std::fmt::Debug for UrlSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UrlSource(..)")
    }
}

impl UrlSource {
    fn get(&self) -> Option<String> {
        match self {
            UrlSource::Default(dir) => {
                let from_env = std::env::var("DEADROLL_SHEET_URL").ok();
                from_env
                    .or_else(|| fs::read_to_string(dir.join("balance-sheet-url.txt")).ok())
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            }
            #[cfg(test)]
            UrlSource::Fixed(url) => Some(url.clone()),
        }
    }

    /// Whether there is a URL at all (the history is off without one).
    pub fn available(&self) -> bool {
        self.get().is_some()
    }
}

/// What the Results tab shows.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Status {
    pub busy: bool,
    /// Requests still waiting in the queue.
    pub queued: usize,
    /// Requests the sheet has taken since the bench started.
    pub sent: u64,
    pub warning: Option<String>,
}

enum Msg {
    Submit(String),
    Flush,
    /// Answers when everything before it is done (tests).
    #[cfg(test)]
    Barrier(mpsc::Sender<()>),
}

pub struct Sender {
    tx: mpsc::Sender<Msg>,
    status: Arc<Mutex<Status>>,
}

impl Sender {
    /// Starts the worker. `repaint` is called whenever the status changes.
    pub fn start(dir: PathBuf, source: UrlSource, repaint: impl Fn() + Send + 'static) -> Sender {
        let (tx, rx) = mpsc::channel();
        let status = Arc::new(Mutex::new(Status::default()));
        let mut worker = Worker {
            path: dir.join("balance-log-queue.jsonl"),
            queue: Vec::new(),
            source,
            status: status.clone(),
            repaint: Box::new(repaint),
        };
        worker.load();
        std::thread::spawn(move || {
            while let Ok(msg) = rx.recv() {
                match msg {
                    Msg::Submit(body) => worker.submit(body),
                    Msg::Flush => worker.flush(),
                    #[cfg(test)]
                    Msg::Barrier(done) => {
                        let _ = done.send(());
                    }
                }
            }
        });
        Sender { tx, status }
    }

    /// Queues `body` (one JSON document) and sends the queue.
    pub fn submit(&self, body: String) {
        self.set(|s| s.busy = true);
        let _ = self.tx.send(Msg::Submit(body));
    }

    /// Sends what an earlier session left in the queue.
    pub fn flush(&self) {
        let _ = self.tx.send(Msg::Flush);
    }

    pub fn status(&self) -> Status {
        self.status.lock().map(|s| s.clone()).unwrap_or_default()
    }

    /// Blocks until the worker has dealt with everything sent so far.
    #[cfg(test)]
    pub fn wait_idle(&self) {
        let (done, wait) = mpsc::channel();
        let _ = self.tx.send(Msg::Barrier(done));
        let _ = wait.recv();
    }

    fn set(&self, f: impl FnOnce(&mut Status)) {
        if let Ok(mut s) = self.status.lock() {
            f(&mut s);
        }
    }
}

struct Worker {
    path: PathBuf,
    queue: Vec<String>,
    source: UrlSource,
    status: Arc<Mutex<Status>>,
    repaint: Box<dyn Fn() + Send>,
}

impl Worker {
    fn load(&mut self) {
        self.queue = fs::read_to_string(&self.path)
            .map(|text| {
                text.lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(str::to_string)
                    .collect()
            })
            .unwrap_or_default();
        let queued = self.queue.len();
        self.update(|s| s.queued = queued);
    }

    fn update(&self, f: impl FnOnce(&mut Status)) {
        if let Ok(mut s) = self.status.lock() {
            f(&mut s);
        }
        (self.repaint)();
    }

    /// Writes the queue file. A failure is a warning, the queue stays in memory.
    fn persist(&self) -> Result<(), ()> {
        let mut text = self.queue.join("\n");
        if !text.is_empty() {
            text.push('\n');
        }
        write_atomic(&self.path, &text).map_err(|_| ())
    }

    fn submit(&mut self, body: String) {
        // One request per line: a document with a line break would split.
        self.queue.push(body.replace(['\n', '\r'], " "));
        let saved = self.persist();
        let queued = self.queue.len();
        self.update(|s| {
            s.queued = queued;
            s.busy = true;
            if saved.is_err() {
                s.warning = Some("could not save the send queue to private/".into());
            }
        });
        self.flush();
    }

    fn flush(&mut self) {
        if self.queue.is_empty() {
            self.update(|s| s.busy = false);
            return;
        }
        let Some(url) = self.source.get() else {
            let queued = self.queue.len();
            self.update(|s| {
                s.busy = false;
                s.queued = queued;
                s.warning = Some("no sheet URL, requests wait in the queue".into());
            });
            return;
        };
        self.update(|s| s.busy = true);
        let mut warning = None;
        while !self.queue.is_empty() {
            match deliver(&url, &self.queue[0]) {
                Ok(confirmed) => {
                    self.queue.remove(0);
                    let saved = self.persist();
                    if !confirmed {
                        warning = Some(
                            "the rows were sent, but the sheet did not confirm with ok".to_string(),
                        );
                    }
                    if saved.is_err() {
                        warning = Some("could not save the send queue to private/".into());
                    }
                    let queued = self.queue.len();
                    self.update(|s| {
                        s.sent += 1;
                        s.queued = queued;
                    });
                }
                Err(why) => {
                    warning = Some(format!("{why}; {} waiting in the queue", self.queue.len()));
                    break;
                }
            }
        }
        let queued = self.queue.len();
        self.update(|s| {
            s.busy = false;
            s.queued = queued;
            s.warning = warning;
        });
    }
}

fn agent(url: &str, follow: bool) -> ureq::Agent {
    let local = url.starts_with("http://127.0.0.1") || url.starts_with("http://localhost");
    let mut builder = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .timeout_global(Some(Duration::from_secs(30)));
    if !follow {
        builder = builder.max_redirects(0);
    }
    if local {
        builder = builder.proxy(None);
    }
    builder.build().into()
}

/// A text for an error that says nothing about where it went.
fn short(e: &ureq::Error) -> &'static str {
    match e {
        ureq::Error::Timeout(_) => "the request timed out",
        ureq::Error::HostNotFound | ureq::Error::ConnectionFailed | ureq::Error::Io(_) => {
            "no connection to the sheet"
        }
        _ => "the request failed",
    }
}

/// Sends one request. `Ok(true)`: the sheet has it and said `ok`. `Ok(false)`: the rows are in
/// (the web app redirected) but the reply was not `ok`. `Err`: the sheet does not have it.
fn deliver(url: &str, body: &str) -> Result<bool, String> {
    let response = agent(url, false)
        .post(url)
        .content_type("application/json")
        .send(body)
        .map_err(|e| short(&e).to_string())?;
    let status = response.status().as_u16();
    match status {
        301 | 302 | 303 | 307 | 308 => {
            let target = response
                .headers()
                .get("location")
                .and_then(|v| v.to_str().ok())
                .map(|loc| resolve(url, loc));
            let Some(target) = target else {
                return Ok(false);
            };
            // The rows are written. Only the reply is read; the POST is never sent again.
            let reply = agent(&target, true)
                .get(&target)
                .call()
                .ok()
                .filter(|r| r.status().is_success())
                .and_then(|r| r.into_body().read_to_string().ok());
            Ok(reply.is_some_and(|text| text.trim() == "ok"))
        }
        200..=299 => {
            let text = response.into_body().read_to_string().unwrap_or_default();
            if text.trim() == "ok" {
                Ok(true)
            } else {
                Err("the sheet answered something other than ok".into())
            }
        }
        _ => Err(format!("the sheet answered with status {status}")),
    }
}

/// A redirect target as a full URL.
fn resolve(base: &str, location: &str) -> String {
    if location.contains("://") {
        return location.to_string();
    }
    let origin_end = base
        .find("://")
        .and_then(|i| base[i + 3..].find('/').map(|j| i + 3 + j))
        .unwrap_or(base.len());
    let origin = &base[..origin_end];
    if location.starts_with('/') {
        format!("{origin}{location}")
    } else {
        format!("{origin}/{location}")
    }
}

#[cfg(test)]
pub(crate) fn queue_len(dir: &std::path::Path) -> usize {
    fs::read_to_string(dir.join("balance-log-queue.jsonl"))
        .map(|t| t.lines().filter(|l| !l.trim().is_empty()).count())
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::{
        collections::VecDeque,
        io::{Read, Write},
        net::{TcpListener, TcpStream},
    };

    use super::*;

    /// What the test server answers to a POST.
    #[derive(Clone, Copy)]
    enum Reply {
        /// 302 to a page that says `ok`.
        Redirect,
        /// 302 to a page that fails.
        RedirectToBroken,
        Status(u16),
        Body(&'static str),
    }

    struct Server {
        url: String,
        posts: Arc<Mutex<Vec<String>>>,
        script: Arc<Mutex<VecDeque<Reply>>>,
    }

    fn read_request(stream: &mut TcpStream) -> Option<(String, String, String)> {
        let mut data = Vec::new();
        let mut chunk = [0u8; 4096];
        let end = loop {
            let n = stream.read(&mut chunk).ok()?;
            if n == 0 {
                return None;
            }
            data.extend_from_slice(&chunk[..n]);
            if let Some(i) = data.windows(4).position(|w| w == b"\r\n\r\n") {
                break i + 4;
            }
        };
        let head = String::from_utf8_lossy(&data[..end]).to_string();
        let length = head
            .lines()
            .find_map(|l| {
                l.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .map(|v| v.trim().parse::<usize>().ok())
            })
            .flatten()
            .unwrap_or(0);
        while data.len() < end + length {
            let n = stream.read(&mut chunk).ok()?;
            if n == 0 {
                break;
            }
            data.extend_from_slice(&chunk[..n]);
        }
        let mut first = head.lines().next()?.split_whitespace();
        let method = first.next()?.to_string();
        let path = first.next()?.to_string();
        Some((
            method,
            path,
            String::from_utf8_lossy(&data[end..end + length]).to_string(),
        ))
    }

    fn respond(stream: &mut TcpStream, status: u16, extra: &str, body: &str) {
        let text = format!(
            "HTTP/1.1 {status} X\r\n{extra}Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
            body.len()
        );
        let _ = stream.write_all(text.as_bytes());
    }

    impl Server {
        fn start(script: Vec<Reply>) -> Server {
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            let url = format!("http://{}/exec", listener.local_addr().unwrap());
            let posts = Arc::new(Mutex::new(Vec::new()));
            let script = Arc::new(Mutex::new(VecDeque::from(script)));
            let (p, s, base) = (posts.clone(), script.clone(), url.clone());
            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    let Ok(mut stream) = stream else { break };
                    let Some((method, path, body)) = read_request(&mut stream) else {
                        continue;
                    };
                    match (method.as_str(), path.as_str()) {
                        ("POST", _) => {
                            p.lock().unwrap().push(body);
                            let reply = s.lock().unwrap().pop_front().unwrap_or(Reply::Redirect);
                            match reply {
                                Reply::Redirect => respond(
                                    &mut stream,
                                    302,
                                    &format!("Location: {}\r\n", base.replace("/exec", "/done")),
                                    "",
                                ),
                                Reply::RedirectToBroken => respond(
                                    &mut stream,
                                    302,
                                    &format!("Location: {}\r\n", base.replace("/exec", "/broken")),
                                    "",
                                ),
                                Reply::Status(code) => respond(&mut stream, code, "", "nope"),
                                Reply::Body(text) => respond(&mut stream, 200, "", text),
                            }
                        }
                        ("GET", "/done") => respond(&mut stream, 200, "", "ok"),
                        _ => respond(&mut stream, 500, "", "broken"),
                    }
                }
            });
            Server { url, posts, script }
        }

        fn posts(&self) -> Vec<String> {
            self.posts.lock().unwrap().clone()
        }
    }

    fn temp(name: &str) -> PathBuf {
        let dir =
            std::env::temp_dir().join(format!("deadroll-sender-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sender(dir: &std::path::Path, url: &str) -> Sender {
        Sender::start(dir.to_path_buf(), UrlSource::Fixed(url.to_string()), || {})
    }

    #[test]
    fn a_run_is_posted_once_and_leaves_the_queue_after_the_redirect() {
        let server = Server::start(vec![]);
        let dir = temp("once");
        let s = sender(&dir, &server.url);
        s.submit(r#"{"sheets":[]}"#.into());
        s.wait_idle();
        assert_eq!(server.posts(), vec![r#"{"sheets":[]}"#.to_string()]);
        let status = s.status();
        assert_eq!((status.sent, status.queued, status.busy), (1, 0, false));
        assert_eq!(status.warning, None);
        assert_eq!(queue_len(&dir), 0);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_failed_send_is_kept_and_goes_out_first_with_the_next_run_in_order() {
        let server = Server::start(vec![Reply::Status(500)]);
        let dir = temp("order");
        let s = sender(&dir, &server.url);
        s.submit(r#"{"run":"A"}"#.into());
        s.wait_idle();
        assert_eq!(queue_len(&dir), 1);
        let status = s.status();
        assert_eq!((status.sent, status.queued), (0, 1));
        assert!(status.warning.is_some());
        s.submit(r#"{"run":"B"}"#.into());
        s.wait_idle();
        // A was refused once, then delivered once; B once; order kept.
        assert_eq!(
            server.posts(),
            vec![r#"{"run":"A"}"#, r#"{"run":"A"}"#, r#"{"run":"B"}"#]
        );
        let status = s.status();
        assert_eq!((status.sent, status.queued, status.warning), (2, 0, None));
        assert_eq!(queue_len(&dir), 0);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_reply_other_than_ok_keeps_the_request() {
        let server = Server::start(vec![Reply::Body("denied")]);
        let dir = temp("body");
        let s = sender(&dir, &server.url);
        s.submit(r#"{"run":"A"}"#.into());
        s.wait_idle();
        assert_eq!(s.status().queued, 1);
        assert_eq!(queue_len(&dir), 1);
        s.flush();
        s.wait_idle();
        assert_eq!(s.status().queued, 0);
        assert_eq!(server.posts().len(), 2);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn rows_that_were_written_are_never_posted_again_when_the_confirmation_fails() {
        let server = Server::start(vec![Reply::RedirectToBroken]);
        let dir = temp("nodup");
        let s = sender(&dir, &server.url);
        s.submit(r#"{"run":"A"}"#.into());
        s.wait_idle();
        let status = s.status();
        assert_eq!((status.sent, status.queued), (1, 0));
        assert!(status.warning.is_some());
        s.submit(r#"{"run":"B"}"#.into());
        s.wait_idle();
        assert_eq!(server.posts(), vec![r#"{"run":"A"}"#, r#"{"run":"B"}"#]);
        assert!(server.script.lock().unwrap().is_empty());
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_dead_network_queues_and_a_new_session_sends_the_queue_in_order() {
        let dir = temp("restart");
        // Nothing listens on this port.
        let dead = {
            let l = TcpListener::bind("127.0.0.1:0").unwrap();
            format!("http://{}/exec", l.local_addr().unwrap())
        };
        let s = sender(&dir, &dead);
        s.submit(r#"{"run":"A"}"#.into());
        s.submit(r#"{"run":"B"}"#.into());
        s.wait_idle();
        assert_eq!(s.status().queued, 2);
        assert_eq!(queue_len(&dir), 2);
        let shown = format!("{:?}", s.status());
        assert!(!shown.contains("127.0.0.1"), "{shown}");
        drop(s);
        let server = Server::start(vec![]);
        let again = sender(&dir, &server.url);
        assert_eq!(again.status().queued, 2);
        again.flush();
        again.wait_idle();
        assert_eq!(server.posts(), vec![r#"{"run":"A"}"#, r#"{"run":"B"}"#]);
        assert_eq!(queue_len(&dir), 0);
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn no_message_contains_the_url() {
        let server = Server::start(vec![Reply::Status(502), Reply::Body("no")]);
        let dir = temp("secret");
        let s = sender(&dir, &server.url);
        for _ in 0..2 {
            s.submit(r#"{"run":"A"}"#.into());
            s.wait_idle();
            let shown = format!("{:?}", s.status());
            assert!(
                !shown.contains(&server.url) && !shown.contains("127.0.0.1"),
                "{shown}"
            );
        }
        fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn a_relative_redirect_is_made_whole() {
        assert_eq!(
            resolve("https://h.example/a/b", "/done"),
            "https://h.example/done"
        );
        assert_eq!(
            resolve("https://h.example/a/b", "https://o/x"),
            "https://o/x"
        );
        assert_eq!(
            resolve("https://h.example", "done"),
            "https://h.example/done"
        );
    }

    #[test]
    fn the_url_comes_from_the_environment_or_the_private_file() {
        let dir = temp("url");
        let source = UrlSource::Default(dir.clone());
        if std::env::var_os("DEADROLL_SHEET_URL").is_none() {
            assert!(!source.available());
            fs::write(
                dir.join("balance-sheet-url.txt"),
                "  https://example.test/x \n",
            )
            .unwrap();
            assert!(source.available());
            assert_eq!(source.get().as_deref(), Some("https://example.test/x"));
        }
        assert_eq!(format!("{source:?}"), "UrlSource(..)");
        fs::remove_dir_all(&dir).unwrap();
    }
}
