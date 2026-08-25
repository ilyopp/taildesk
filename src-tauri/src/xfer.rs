use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::io::{Read, Write};
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::{Mutex as StdMutex, OnceLock};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use tauri::Emitter as _;

const XFER_PORT: u16 = 47610;
const OFFER_TIMEOUT_SECS: u64 = 300;
const HISTORY_MAX: usize = 100;
const PART_EXT: &str = ".tdpart";

#[derive(Serialize, Deserialize, Clone)]
pub struct XferFile {
    pub name: String,
    pub size: u64,
    #[serde(default)]
    pub sent: u64,
    pub state: String,
}

#[derive(Serialize, Deserialize, Clone)]
pub struct XferBatch {
    pub id: String,
    pub dir: String,
    pub peer: String,
    pub ip: String,
    pub mode: String,
    pub state: String,
    #[serde(default)]
    pub error: String,
    #[serde(default)]
    pub target: String,
    pub created: u64,
    pub files: Vec<XferFile>,
}

struct State {
    batches: Vec<XferBatch>,
    decisions: HashMap<String, std::sync::mpsc::Sender<bool>>,
}

static STATE: OnceLock<StdMutex<State>> = OnceLock::new();
static APP: OnceLock<tauri::AppHandle> = OnceLock::new();

/// AppHandle global : le dispatcher de connexions réveille l'UI pour la
/// pop-up de consentement du contrôle à distance.
pub(crate) fn app_handle() -> Option<tauri::AppHandle> {
    APP.get().cloned()
}

static BOUND_IP: StdMutex<Option<String>> = StdMutex::new(None);
static SERVER_GEN: AtomicU64 = AtomicU64::new(0);
static AUTO_ACCEPT: StdMutex<bool> = StdMutex::new(false);
static SAVE_DIR: StdMutex<Option<String>> = StdMutex::new(None);
static HISTORY_PATH: StdMutex<Option<PathBuf>> = StdMutex::new(None);
static SEQ: AtomicU32 = AtomicU32::new(0);
static LAST_EMIT: StdMutex<Option<Instant>> = StdMutex::new(None);

#[derive(Deserialize)]
struct OfferReq {
    files: Vec<OfferFile>,
    #[serde(default)]
    from: String,
}

#[derive(Deserialize)]
struct OfferFile {
    name: String,
    #[serde(default)]
    size: u64,
}

fn state() -> &'static StdMutex<State> {
    STATE.get_or_init(|| StdMutex::new(State {
        batches: load_history(),
        decisions: HashMap::new(),
    }))
}

pub fn init(app: tauri::AppHandle) {
    let _ = APP.set(app.clone());
    use tauri::Manager as _;
    if let Ok(dir) = app.path().app_data_dir() {
        let _ = std::fs::create_dir_all(&dir);
        *HISTORY_PATH.lock().unwrap() = Some(dir.join("xfer_history.json"));
    }
}

fn load_history() -> Vec<XferBatch> {
    let path = HISTORY_PATH.lock().unwrap().clone();
    let Some(path) = path else {
        return Vec::new();
    };
    std::fs::read(path)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok())
        .unwrap_or_default()
}

fn persist_locked(st: &State) {
    let path = HISTORY_PATH.lock().unwrap().clone();
    let Some(path) = path else { return };
    let mut all: Vec<XferBatch> = st
        .batches
        .iter()
        .filter(|b| matches!(b.state.as_str(), "done" | "failed" | "refused"))
        .cloned()
        .collect();
    all.sort_by(|a, b| b.created.cmp(&a.created));
    all.truncate(HISTORY_MAX);
    if let Ok(j) = serde_json::to_string_pretty(&all) {
        let _ = std::fs::write(path, j);
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0)
}

fn new_id() -> String {
    let n = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.subsec_nanos())
        .unwrap_or(0)
        ^ SEQ.fetch_add(1, Ordering::SeqCst).wrapping_mul(2654435761);
    format!("{n:08x}")
}

fn host_label() -> String {
    std::env::var("COMPUTERNAME")
        .ok()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "PC".into())
}

fn safe_name(raw: &str) -> String {
    let base = raw.rsplit(['\\', '/']).next().unwrap_or("");
    let mut out = String::new();
    for c in base.chars().take(120) {
        if c.is_ascii_alphanumeric() || matches!(c, ' ' | '.' | '_' | '-' | '(' | ')') {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    let trimmed = out.trim().trim_matches('.').to_string();
    if trimmed.is_empty() {
        "fichier".into()
    } else {
        trimmed
    }
}

pub(crate) fn safe_label(raw: &str) -> String {
    let mut out = String::new();
    for c in raw.trim().chars().take(48) {
        if c.is_ascii_alphanumeric() || matches!(c, ' ' | '.' | '_' | '-') {
            out.push(c);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "inconnu".into()
    } else {
        out
    }
}

fn uniquify(path: PathBuf) -> PathBuf {
    if !path.exists() {
        return path;
    }
    let dir = path.parent().map(|p| p.to_path_buf()).unwrap_or_default();
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("fichier")
        .to_string();
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|e| format!(".{e}"))
        .unwrap_or_default();
    for n in 1..1000u32 {
        let cand = dir.join(format!("{stem} ({n}){ext}"));
        if !cand.exists() {
            return cand;
        }
    }
    path
}

fn resolve_save_dir() -> String {
    let custom = SAVE_DIR.lock().unwrap().clone();
    let dir = match custom {
        Some(d) if !d.trim().is_empty() => d,
        _ => {
            let base = std::env::var("USERPROFILE").unwrap_or_else(|_| ".".into());
            format!("{base}\\Downloads\\Taildesk")
        }
    };
    let _ = std::fs::create_dir_all(&dir);
    dir
}

fn cleanup_parts(dir: &str) {
    if let Ok(rd) = std::fs::read_dir(dir) {
        for e in rd.flatten() {
            let p = e.path();
            if p.extension().and_then(|x| x.to_str()) == Some("tdpart") {
                let _ = std::fs::remove_file(p);
            }
        }
    }
}

pub(crate) fn verify_tailnet_ip(ip: &str) -> bool {
    let mut cmd = match crate::ts_command() {
        Ok(c) => c,
        Err(_) => return false,
    };
    match cmd.args(["status", "--json"]).output() {
        Ok(o) => {
            let s = String::from_utf8_lossy(&o.stdout);
            s.contains(&format!("\"{ip}\""))
        }
        Err(_) => false,
    }
}

fn snapshot() -> Vec<XferBatch> {
    let st = state().lock().unwrap();
    let mut v = st.batches.clone();
    v.sort_by(|a, b| b.created.cmp(&a.created));
    v
}

fn notify(force: bool) {
    if !force {
        let mut last = LAST_EMIT.lock().unwrap();
        if let Some(t0) = *last {
            if t0.elapsed() < Duration::from_millis(150) {
                return;
            }
        }
        *last = Some(Instant::now());
    }
    if let Some(app) = APP.get() {
        let items = snapshot();
        let _ = app.emit("xfer-update", json!({ "items": items }));
    }
}

fn mut_batch(id: &str, f: impl FnOnce(&mut XferBatch)) -> bool {
    let mut st = state().lock().unwrap();
    match st.batches.iter_mut().find(|b| b.id == id) {
        Some(b) => {
            f(b);
            true
        }
        None => false,
    }
}

fn mark_fail(id: &str, idx: Option<usize>, msg: &str) {
    {
        let mut st = state().lock().unwrap();
        if let Some(b) = st.batches.iter_mut().find(|b| b.id == id) {
            if let Some(i) = idx {
                if let Some(f) = b.files.get_mut(i) {
                    f.state = "failed".into();
                }
            }
            b.state = "failed".into();
            b.error = msg.to_string();
            persist_locked(&st);
        }
    }
    notify(true);
}

pub fn ensure_server(self_ip: &str) {
    {
        let bound = BOUND_IP.lock().unwrap();
        if bound.as_deref() == Some(self_ip) {
            return;
        }
    }
    let addr: SocketAddr = match format!("{self_ip}:{XFER_PORT}").parse() {
        Ok(a) => a,
        Err(_) => return,
    };
    let listener = match TcpListener::bind(addr) {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Transferts : port {XFER_PORT} indisponible ({e})");
            return;
        }
    };
    *BOUND_IP.lock().unwrap() = Some(self_ip.to_string());
    let gen = SERVER_GEN.fetch_add(1, Ordering::SeqCst) + 1;
    std::thread::spawn(move || {
        let _ = listener.set_nonblocking(true);
        loop {
            if SERVER_GEN.load(Ordering::SeqCst) != gen {
                return;
            }
            match listener.accept() {
                Ok((s, _)) => {
                    let _ = std::thread::spawn(move || handle_conn(s));
                }
                Err(ref e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(120));
                }
                Err(_) => std::thread::sleep(Duration::from_millis(300)),
            }
        }
    });
}

pub(crate) fn read_head(stream: &mut TcpStream) -> Result<(String, Vec<u8>), String> {
    let mut buf: Vec<u8> = Vec::with_capacity(4096);
    let mut tmp = [0u8; 1024];
    loop {
        if let Some(i) = buf.windows(4).position(|w| w == b"\r\n\r\n") {
            let rest = buf.split_off(i + 4);
            return Ok((String::from_utf8_lossy(&buf).to_string(), rest));
        }
        if buf.len() > 16384 {
            return Err("entête trop grande".into());
        }
        let n = stream.read(&mut tmp).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("connexion fermée".into());
        }
        buf.extend_from_slice(&tmp[..n]);
    }
}

fn parse_head(head: &str) -> Option<(String, String, HashMap<String, String>)> {
    let mut lines = head.split("\r\n");
    let first = lines.next()?;
    let mut parts = first.split_whitespace();
    let method = parts.next()?.to_string();
    let target = parts.next()?.to_string();
    let mut headers = HashMap::new();
    for l in lines {
        if l.is_empty() {
            break;
        }
        if let Some((k, v)) = l.split_once(':') {
            headers.insert(k.trim().to_lowercase(), v.trim().to_string());
        }
    }
    Some((method, target, headers))
}

pub(crate) fn header_cl(head: &str) -> Option<u64> {
    for l in head.lines() {
        if let Some((k, v)) = l.split_once(':') {
            if k.trim().eq_ignore_ascii_case("content-length") {
                return v.trim().parse().ok();
            }
        }
    }
    None
}

pub(crate) fn content_len(hdrs: &HashMap<String, String>) -> Option<u64> {
    hdrs.get("content-length")?.parse().ok()
}

pub(crate) fn expect_continue(hdrs: &HashMap<String, String>, stream: &mut TcpStream) -> bool {
    if hdrs.get("expect").map(|v| v.contains("100-continue")) == Some(true) {
        stream.write_all(b"HTTP/1.1 100 Continue\r\n\r\n").is_ok()
    } else {
        true
    }
}

pub(crate) fn read_n(
    stream: &mut TcpStream,
    leftover: Vec<u8>,
    len: u64,
) -> Result<Vec<u8>, String> {
    let mut out = Vec::with_capacity(len.min(1 << 20) as usize);
    let mut remaining = len;
    let take = (leftover.len() as u64).min(remaining) as usize;
    out.extend_from_slice(&leftover[..take]);
    remaining -= take as u64;
    let mut tmp = [0u8; 16384];
    while remaining > 0 {
        let want = (remaining as usize).min(tmp.len());
        let n = stream.read(&mut tmp[..want]).map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("fin de flux".into());
        }
        out.extend_from_slice(&tmp[..n]);
        remaining -= n as u64;
    }
    Ok(out)
}

pub(crate) fn respond(stream: &mut TcpStream, status: u16, reason: &str, ctype: &str, body: &str) {
    let head = format!(
        "HTTP/1.1 {status} {reason}\r\nContent-Type: {ctype}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    let _ = stream.write_all(head.as_bytes());
    let _ = stream.write_all(body.as_bytes());
    let _ = stream.flush();
}

pub(crate) fn q_value(q: &str, key: &str) -> Option<String> {
    for pair in q.split('&') {
        let (k, v) = pair.split_once('=')?;
        if k == key {
            return Some(v.to_string());
        }
    }
    None
}

fn handle_conn(mut stream: TcpStream) {
    let _ = stream.set_nonblocking(false);
    let _ = stream.set_read_timeout(Some(Duration::from_secs(20)));
    let (head, leftover) = match read_head(&mut stream) {
        Ok(v) => v,
        Err(_) => return,
    };
    let Some((method, target, hdrs)) = parse_head(&head) else {
        return;
    };
    let (path, query) = match target.split_once('?') {
        Some((p, q)) => (p.to_string(), q.to_string()),
        None => (target.clone(), String::new()),
    };
    match (method.as_str(), path.as_str()) {
        ("GET", "/ping") => {
            let body = json!({"app": "taildesk", "name": host_label()}).to_string();
            respond(&mut stream, 200, "OK", "application/json", &body);
        }
        ("POST", "/offer") => handle_offer(&mut stream, &hdrs, leftover),
        ("PUT", "/data") => handle_put(&mut stream, &hdrs, leftover, &query),
        ("POST", "/rc-request") => {
            if let Some(app) = app_handle() {
                crate::rc::handle_request(&app, &mut stream, &hdrs, leftover);
            }
        }
        ("GET", "/rc-stream") => {
            if let Some(app) = app_handle() {
                crate::rc::handle_stream(&app, &mut stream, &query);
            }
        }
        ("POST", "/rc-input") => {
            if let Some(app) = app_handle() {
                crate::rc::handle_input(&app, &mut stream, &query);
            }
        }
        _ => respond(
            &mut stream,
            404,
            "Not Found",
            "application/json",
            "{\"ok\":false}",
        ),
    }
}

fn handle_offer(stream: &mut TcpStream, hdrs: &HashMap<String, String>, leftover: Vec<u8>) {
    let Some(cl) = content_len(hdrs) else {
        respond(stream, 400, "Bad Request", "application/json", "{\"ok\":false}");
        return;
    };
    if cl > 65536 {
        respond(stream, 413, "Payload Too Large", "application/json", "{\"ok\":false}");
        return;
    }
    if !expect_continue(hdrs, stream) {
        return;
    }
    let body = match read_n(stream, leftover, cl) {
        Ok(b) => b,
        Err(_) => return,
    };
    let req: OfferReq = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(_) => {
            respond(stream, 400, "Bad Request", "application/json", "{\"ok\":false}");
            return;
        }
    };
    let files: Vec<XferFile> = req
        .files
        .iter()
        .take(512)
        .map(|f| XferFile {
            name: safe_name(&f.name),
            size: f.size,
            sent: 0,
            state: "queued".into(),
        })
        .collect();
    if files.is_empty() {
        respond(stream, 400, "Bad Request", "application/json", "{\"ok\":false}");
        return;
    }
    let ip = match stream.peer_addr() {
        Ok(a) => a.ip().to_string(),
        Err(_) => return,
    };
    let peer = safe_label(&req.from);
    if !verify_tailnet_ip(&ip) {
        respond(
            stream,
            403,
            "Forbidden",
            "application/json",
            "{\"ok\":false,\"error\":\"hors tailnet\"}",
        );
        return;
    }

    let id = new_id();
    let (tx, rx) = std::sync::mpsc::channel::<bool>();
    {
        let mut st = state().lock().unwrap();
        st.decisions.insert(id.clone(), tx);
        st.batches.push(XferBatch {
            id: id.clone(),
            dir: "in".into(),
            peer,
            ip,
            mode: "direct".into(),
            state: "pending".into(),
            error: String::new(),
            target: String::new(),
            created: now_ms(),
            files,
        });
    }
    notify(true);

    if *AUTO_ACCEPT.lock().unwrap() {
        let accepted = {
            let mut st = state().lock().unwrap();
            match st.batches.iter_mut().find(|b| b.id == id && b.state == "pending") {
                Some(b) => {
                    b.state = "active".into();
                    b.target = resolve_save_dir();
                    true
                }
                None => false,
            }
        };
        if accepted {
            if let Some(tx) = state().lock().unwrap().decisions.remove(&id) {
                let _ = tx.send(true);
            }
            notify(true);
        }
    }

    match rx.recv_timeout(Duration::from_secs(OFFER_TIMEOUT_SECS)) {
        Ok(true) => {
            let body = json!({"ok": true, "batch": id}).to_string();
            respond(stream, 200, "OK", "application/json", &body);
        }
        Ok(false) => {
            respond(
                stream,
                403,
                "Forbidden",
                "application/json",
                "{\"ok\":false,\"error\":\"refusé\"}",
            );
        }
        Err(_) => {
            {
                let mut st = state().lock().unwrap();
                if let Some(b) = st
                    .batches
                    .iter_mut()
                    .find(|b| b.id == id && b.state == "pending")
                {
                    b.state = "failed".into();
                    b.error = "expiré".into();
                }
                st.decisions.remove(&id);
                persist_locked(&st);
            }
            notify(true);
            respond(
                stream,
                408,
                "Request Timeout",
                "application/json",
                "{\"ok\":false,\"error\":\"expiré\"}",
            );
        }
    }
}

fn handle_put(stream: &mut TcpStream, hdrs: &HashMap<String, String>, leftover: Vec<u8>, q: &str) {
    let id = q_value(q, "batch").unwrap_or_default();
    let idx: usize = q_value(q, "i").and_then(|v| v.parse().ok()).unwrap_or(usize::MAX);
    let Some(cl) = content_len(hdrs) else {
        respond(stream, 400, "Bad Request", "application/json", "{\"ok\":false}");
        return;
    };
    if !expect_continue(hdrs, stream) {
        return;
    }
    let save_dir = resolve_save_dir();
    let declared_size = {
        let mut st = state().lock().unwrap();
        match st
            .batches
            .iter_mut()
            .find(|b| b.id == id && b.dir == "in" && b.state == "active")
        {
            Some(b) => match b.files.get_mut(idx) {
                Some(f) if f.state == "queued" || f.state == "saving" => {
                    f.state = "saving".into();
                    if b.target.is_empty() {
                        b.target = save_dir.clone();
                    }
                    f.size
                }
                _ => {
                    respond(stream, 409, "Conflict", "application/json", "{\"ok\":false}");
                    return;
                }
            },
            None => {
                respond(stream, 404, "Not Found", "application/json", "{\"ok\":false}");
                return;
            }
        }
    };
    notify(false);

    if cl != declared_size {
        mark_fail(&id, Some(idx), "taille inattendue");
        respond(
            stream,
            400,
            "Bad Request",
            "application/json",
            "{\"ok\":false,\"error\":\"taille\"}",
        );
        return;
    }

    let final_path = {
        let st = state().lock().unwrap();
        let name = st
            .batches
            .iter()
            .find(|b| b.id == id)
            .and_then(|b| b.files.get(idx))
            .map(|f| f.name.clone())
            .unwrap_or_else(|| "fichier".into());
        uniquify(std::path::Path::new(&save_dir).join(name))
    };
    let mut part_os = final_path.into_os_string();
    part_os.push(PART_EXT);
    let part_path = PathBuf::from(part_os);

    let mut file = match std::fs::File::create(&part_path) {
        Ok(f) => f,
        Err(e) => {
            mark_fail(&id, Some(idx), &format!("disque : {e}"));
            respond(stream, 500, "Server Error", "application/json", "{\"ok\":false}");
            return;
        }
    };

    let _ = stream.set_read_timeout(Some(Duration::from_secs(60)));
    let mut remaining = cl;
    let mut ok = true;
    let mut errmsg = String::new();

    let take = (leftover.len() as u64).min(remaining) as usize;
    if file.write_all(&leftover[..take]).is_err() {
        ok = false;
        errmsg = "écriture disque impossible".into();
    }
    remaining -= take as u64;

    let mut tmp = [0u8; 65536];
    let mut last = Instant::now();
    while ok && remaining > 0 {
        let want = (remaining as usize).min(tmp.len());
        match stream.read(&mut tmp[..want]) {
            Ok(0) => {
                ok = false;
                errmsg = "flux interrompu".into();
            }
            Ok(n) => {
                if file.write_all(&tmp[..n]).is_err() {
                    ok = false;
                    errmsg = "écriture disque impossible".into();
                    break;
                }
                remaining -= n as u64;
                if last.elapsed() >= Duration::from_millis(150) {
                    let sent = cl - remaining;
                    mut_batch(&id, |b| {
                        if let Some(f) = b.files.get_mut(idx) {
                            f.sent = sent;
                        }
                    });
                    notify(false);
                    last = Instant::now();
                }
            }
            Err(e) => {
                ok = false;
                errmsg = e.to_string();
            }
        }
    }
    drop(file);

    if ok {
        let dest = uniquify(part_path.with_extension(""));
        match std::fs::rename(&part_path, &dest) {
            Ok(()) => {
                {
                    let mut st = state().lock().unwrap();
                    if let Some(b) = st.batches.iter_mut().find(|b| b.id == id) {
                        if let Some(f) = b.files.get_mut(idx) {
                            f.sent = f.size;
                            f.state = "done".into();
                        }
                        if b.files.iter().all(|f| f.state == "done") {
                            b.state = "done".into();
                        }
                        persist_locked(&st);
                    }
                }
                notify(true);
                respond(stream, 200, "OK", "application/json", "{\"ok\":true}");
            }
            Err(e) => {
                let _ = std::fs::remove_file(&part_path);
                mark_fail(&id, Some(idx), &format!("renommer : {e}"));
                respond(stream, 500, "Server Error", "application/json", "{\"ok\":false}");
            }
        }
    } else {
        let _ = std::fs::remove_file(&part_path);
        mark_fail(&id, Some(idx), &errmsg);
        respond(stream, 500, "Server Error", "application/json", "{\"ok\":false}");
    }
}

pub(crate) fn connect_to(ip: &str, timeout: Duration) -> Result<TcpStream, String> {
    let addr: SocketAddr = format!("{ip}:{XFER_PORT}")
        .parse()
        .map_err(|_| "adresse invalide".to_string())?;
    TcpStream::connect_timeout(&addr, timeout).map_err(|e| format!("connexion impossible : {e}"))
}

fn probe_peer(ip: &str) -> Result<(), String> {
    let mut s = connect_to(ip, Duration::from_millis(900))?;
    let _ = s.set_read_timeout(Some(Duration::from_millis(1600)));
    s.write_all(b"GET /ping HTTP/1.1\r\nHost: taildesk\r\nConnection: close\r\n\r\n")
        .map_err(|e| e.to_string())?;
    let mut buf = [0u8; 2048];
    let mut got: Vec<u8> = Vec::new();
    loop {
        match s.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                got.extend_from_slice(&buf[..n]);
                if got.len() > 4096 || got.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            Err(_) => break,
        }
    }
    let txt = String::from_utf8_lossy(&got);
    if txt.contains("\"taildesk\"") {
        Ok(())
    } else {
        Err("pas de serveur taildesk".into())
    }
}

fn offer_peer(ip: &str, body: &str) -> Result<Result<String, String>, String> {
    let mut s = connect_to(ip, Duration::from_secs(3))?;
    let _ = s.set_read_timeout(Some(Duration::from_secs(OFFER_TIMEOUT_SECS + 25)));
    let head = format!(
        "POST /offer HTTP/1.1\r\nHost: taildesk\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    s.write_all(head.as_bytes()).map_err(|e| e.to_string())?;
    s.write_all(body.as_bytes()).map_err(|e| e.to_string())?;

    let (resp_head, leftover) = read_head(&mut s)?;
    let code = resp_head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse::<u16>().ok())
        .unwrap_or(0);
    let bytes = read_n(&mut s, leftover, header_cl(&resp_head).unwrap_or(0))?;
    let v: serde_json::Value =
        serde_json::from_slice(&bytes).unwrap_or(json!({}));
    if code == 200 {
        Ok(Ok(v["batch"].as_str().unwrap_or("").to_string()))
    } else {
        Ok(Err(
            v["error"].as_str().unwrap_or("refusé").to_string()
        ))
    }
}

fn put_file(
    ip: &str,
    batch: &str,
    idx: usize,
    path: &std::path::Path,
    on_progress: impl Fn(u64),
) -> Result<(), String> {
    let size = std::fs::metadata(path).map_err(|e| format!("lecture : {e}"))?.len();
    let mut s = connect_to(ip, Duration::from_secs(3))?;
    let _ = s.set_read_timeout(Some(Duration::from_secs(120)));
    let head = format!(
        "PUT /data?batch={batch}&i={idx} HTTP/1.1\r\nHost: taildesk\r\nContent-Length: {size}\r\nConnection: close\r\n\r\n"
    );
    s.write_all(head.as_bytes()).map_err(|e| e.to_string())?;

    let mut f = std::fs::File::open(path).map_err(|e| format!("lecture : {e}"))?;
    let mut buf = [0u8; 65536];
    let mut sent = 0u64;
    let mut last = Instant::now() - Duration::from_millis(200);
    loop {
        let n = f.read(&mut buf).map_err(|e| format!("lecture : {e}"))?;
        if n == 0 {
            break;
        }
        s.write_all(&buf[..n])
            .map_err(|e| format!("envoi interrompu : {e}"))?;
        sent += n as u64;
        if last.elapsed() >= Duration::from_millis(150) {
            on_progress(sent);
            last = Instant::now();
        }
    }
    on_progress(sent);

    let (resp_head, leftover) = read_head(&mut s)?;
    let code = resp_head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse::<u16>().ok())
        .unwrap_or(0);
    if code == 200 {
        Ok(())
    } else {
        let bytes = read_n(&mut s, leftover, header_cl(&resp_head).unwrap_or(0))?;
        let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or(json!({}));
        Err(v["error"]
            .as_str()
            .unwrap_or("échec côté destinataire")
            .to_string())
    }
}

fn finalize_send(id: &str) {
    {
        let mut st = state().lock().unwrap();
        if let Some(b) = st.batches.iter_mut().find(|b| b.id == id) {
            if b.state != "refused" && b.state != "failed" {
                if b.files.iter().all(|f| f.state == "done") {
                    b.state = "done".into();
                } else {
                    b.state = "failed".into();
                    if b.error.is_empty() {
                        b.error = "échec partiel".into();
                    }
                }
            }
            persist_locked(&st);
        }
    }
    notify(true);
}

fn run_send(id: String, host: String, ip: String, paths: Vec<PathBuf>) {
    let direct = probe_peer(&ip).is_ok();

    if !direct {
        mut_batch(&id, |b| b.mode = "taildrop".into());
        notify(true);
        for (i, path) in paths.iter().enumerate() {
            match crate::taildrop_copy(&host, &path.to_string_lossy()) {
                Ok(()) => {
                    let all_done = {
                        let mut done = false;
                        mut_batch(&id, |b| {
                            if let Some(f) = b.files.get_mut(i) {
                                f.sent = f.size;
                                f.state = "done".into();
                            }
                            done = b.files.iter().all(|f| f.state == "done");
                        });
                        done
                    };
                    if all_done {
                        mut_batch(&id, |b| b.state = "done".into());
                    }
                    notify(true);
                }
                Err(e) => {
                    mark_fail(&id, Some(i), &e);
                }
            }
        }
        finalize_send(&id);
        return;
    }

    let offer_body = {
        let st = state().lock().unwrap();
        let files: Vec<serde_json::Value> = st
            .batches
            .iter()
            .find(|b| b.id == id)
            .map(|b| {
                b.files
                    .iter()
                    .map(|f| json!({"name": f.name, "size": f.size}))
                    .collect()
            })
            .unwrap_or_default();
        json!({"files": files, "from": host_label()}).to_string()
    };

    match offer_peer(&ip, &offer_body) {
        Err(e) => {
            mark_fail(&id, None, &e);
            return;
        }
        Ok(Err(reason)) => {
            {
                let mut st = state().lock().unwrap();
                if let Some(b) = st.batches.iter_mut().find(|b| b.id == id) {
                    b.state = "refused".into();
                    b.error = reason;
                    persist_locked(&st);
                }
            }
            notify(true);
            return;
        }
        Ok(Ok(_)) => {}
    }

    for i in 0..paths.len() {
        let path = paths[i].clone();
        let id2 = id.clone();
        let progress = move |sent: u64| {
            mut_batch(&id2, |b| {
                if let Some(f) = b.files.get_mut(i) {
                    f.sent = sent;
                }
            });
            notify(false);
        };
        match put_file(&ip, &id, i, &path, progress) {
            Ok(()) => {
                let all_done = {
                    let mut done = false;
                    mut_batch(&id, |b| {
                        if let Some(f) = b.files.get_mut(i) {
                            f.sent = f.size;
                            f.state = "done".into();
                        }
                        done = b.files.iter().all(|f| f.state == "done");
                    });
                    done
                };
                if all_done {
                    mut_batch(&id, |b| b.state = "done".into());
                }
                notify(true);
            }
            Err(e) => {
                mark_fail(&id, Some(i), &e);
            }
        }
    }
    finalize_send(&id);
}

#[tauri::command]
pub fn xfer_state() -> Vec<XferBatch> {
    snapshot()
}

#[tauri::command]
pub fn xfer_decide(id: String, accept: bool) -> Result<(), String> {
    let tx = {
        let mut st = state().lock().unwrap();
        match st
            .batches
            .iter_mut()
            .find(|b| b.id == id && b.dir == "in" && b.state == "pending")
        {
            Some(b) => {
                if accept {
                    b.state = "active".into();
                    b.target = resolve_save_dir();
                } else {
                    b.state = "refused".into();
                    b.error = "refusé".into();
                }
                persist_locked(&st);
            }
            None => return Err("Offre introuvable ou déjà traitée.".into()),
        }
        st.decisions.remove(&id)
    };
    if let Some(tx) = tx {
        let _ = tx.send(accept);
    }
    notify(true);
    Ok(())
}

#[tauri::command]
pub fn xfer_send(host: String, ip: String, paths: Vec<String>) -> Result<(), String> {
    if !crate::is_safe_host(&host) {
        return Err("Hôte invalide.".into());
    }
    if ip.parse::<IpAddr>().is_err() {
        return Err("Adresse invalide.".into());
    }
    let mut files = Vec::new();
    let mut real = Vec::new();
    for p in &paths {
        let pb = PathBuf::from(p);
        if let Ok(md) = pb.metadata() {
            if md.is_file() {
                let name = safe_name(
                    pb.file_name().map(|s| s.to_string_lossy()).unwrap_or_default().as_ref(),
                );
                files.push(XferFile {
                    name,
                    size: md.len(),
                    sent: 0,
                    state: "queued".into(),
                });
                real.push(pb);
            }
        }
    }
    if files.is_empty() {
        return Err("Aucun fichier valide.".into());
    }
    if files.len() > 512 {
        return Err("Trop de fichiers d'un coup (512 max).".into());
    }
    let batch = XferBatch {
        id: new_id(),
        dir: "out".into(),
        peer: host.clone(),
        ip: ip.clone(),
        mode: "direct".into(),
        state: "active".into(),
        error: String::new(),
        target: String::new(),
        created: now_ms(),
        files,
    };
    let id = batch.id.clone();
    state().lock().unwrap().batches.push(batch);
    notify(true);
    std::thread::spawn(move || run_send(id, host, ip, real));
    Ok(())
}

#[tauri::command]
pub fn xfer_prefs(auto_accept: bool, dir: String) -> Result<(), String> {
    *AUTO_ACCEPT.lock().unwrap() = auto_accept;
    let trimmed = dir.trim().to_string();
    if trimmed.is_empty() {
        *SAVE_DIR.lock().unwrap() = None;
    } else {
        let _ = std::fs::create_dir_all(&trimmed);
        *SAVE_DIR.lock().unwrap() = Some(trimmed);
    }
    let d = resolve_save_dir();
    cleanup_parts(&d);
    Ok(())
}

#[tauri::command]
pub fn xfer_open_dir(dir: String) -> Result<(), String> {
    let d = dir.trim();
    if d.is_empty() || !std::path::Path::new(d).is_dir() {
        return Err("Dossier introuvable.".into());
    }
    std::process::Command::new("explorer")
        .arg(d)
        .spawn()
        .map_err(|e| e.to_string())?;
    Ok(())
}

#[tauri::command]
pub fn xfer_clear_history() -> Result<(), String> {
    {
        let mut st = state().lock().unwrap();
        let dead: Vec<String> = st
            .batches
            .iter()
            .filter(|b| matches!(b.state.as_str(), "done" | "failed" | "refused"))
            .map(|b| b.id.clone())
            .collect();
        st.decisions.retain(|id, _| !dead.contains(id));
        st.batches
            .retain(|b| !matches!(b.state.as_str(), "done" | "failed" | "refused"));
        persist_locked(&st);
    }
    notify(true);
    Ok(())
}
