use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead as _, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::Emitter as _;

const RC_TIMEOUT_SECS: u64 = 90;
const STREAM_FPS: u64 = 15;
const MUTE_MINS: u64 = 15;

// ---------------------------------------------------------------------------
// État partagé
// ---------------------------------------------------------------------------

struct FrameStore {
    jpeg: Mutex<Vec<u8>>,
    version: AtomicU64,
}

impl FrameStore {
    fn new() -> Self {
        Self {
            jpeg: Mutex::new(Vec::new()),
            version: AtomicU64::new(0),
        }
    }

    fn publish(&self, data: Vec<u8>) {
        *self.jpeg.lock() = data;
        self.version.fetch_add(1, Ordering::SeqCst);
    }

    fn snapshot(&self, last_seen: u64) -> Option<(u64, Vec<u8>)> {
        let v = self.version.load(Ordering::SeqCst);
        if v == last_seen {
            return None;
        }
        Some((v, self.jpeg.lock().clone()))
    }
}

enum Decision {
    Accept { kb: bool, mouse: bool },
    Refuse,
}

struct PendingConsent {
    id: String,
    from_ip: String,
    tx: std::sync::mpsc::Sender<Decision>,
}

static PENDING: Mutex<Option<PendingConsent>> = Mutex::new(None);
static MUTED: std::sync::OnceLock<Mutex<HashMap<String, Instant>>> = std::sync::OnceLock::new();

fn muted() -> &'static Mutex<HashMap<String, Instant>> {
    MUTED.get_or_init(|| Mutex::new(HashMap::new()))
}

/// Retire la demande en attente seulement si c'est encore celle-ci : évite
/// qu'un timeout tardif ne consomme une demande plus récente.
fn clear_pending_if_mine(id: &str) {
    let mut g = PENDING.lock();
    if g.as_ref().is_some_and(|p| p.id == id) {
        g.take();
    }
}

struct ActiveSession {
    token: String,
    ip: String,
    kb: bool,
    mouse: bool,
    store: Arc<FrameStore>,
    alive: Arc<AtomicBool>,
}

static SESSION: Mutex<Option<Arc<ActiveSession>>> = Mutex::new(None);

/// Session terminée (cible ou visionneur) : libère capture et jeton.
fn end_session(app: &tauri::AppHandle) {
    let mut guard = SESSION.lock();
    if let Some(s) = guard.take() {
        s.alive.store(false, Ordering::SeqCst);
        let _ = app.emit(
            "rc-status",
            serde_json::json!({ "stage": "ended", "error": false }),
        );
    }
}

fn end_session_quiet() {
    if let Some(s) = SESSION.lock().take() {
        s.alive.store(false, Ordering::SeqCst);
    }
}

fn new_token() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    static SEQ: AtomicU64 = AtomicU64::new(0x2545F4914F6CDD1D);
    let mut x = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0x9E3779B97F4A7C15)
        ^ SEQ.fetch_add(0x9E3779B97F4A7C15, Ordering::Relaxed);
    let mut out = String::with_capacity(32);
    for _ in 0..2 {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        out.push_str(&format!("{x:016x}"));
    }
    out
}

// ---------------------------------------------------------------------------
// Côté cible : demande d'autorisation, pop-up, capture, entrées
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RcReqBody {
    #[serde(default)]
    from: String,
}

pub(crate) fn handle_request(
    app: &tauri::AppHandle,
    stream: &mut TcpStream,
    hdrs: &HashMap<String, String>,
    leftover: Vec<u8>,
) {
    let Some(cl) = super::xfer::content_len(hdrs) else {
        super::xfer::respond(stream, 400, "Bad Request", "application/json", "{\"ok\":false}");
        return;
    };
    if cl > 4096 || !super::xfer::expect_continue(hdrs, stream) {
        return;
    }
    let Ok(body) = super::xfer::read_n(stream, leftover, cl) else {
        return;
    };
    let req: RcReqBody = serde_json::from_slice(&body).unwrap_or(RcReqBody { from: String::new() });

    let Ok(peer) = stream.peer_addr() else { return };
    let ip = peer.ip().to_string();
    if !super::xfer::verify_tailnet_ip(&ip) {
        super::xfer::respond(
            stream,
            403,
            "Forbidden", "application/json",
            "{\"status\":\"refused\",\"error\":\"hors tailnet\"}",
        );
        return;
    }

    // Appareil ignoré ? Refus immédiat, sans déranger l'utilisateur.
    {
        let mut muted = muted().lock();
        if muted.get(&ip).is_some_and(|until| *until > Instant::now()) {
            super::xfer::respond(stream, 403, "Forbidden", "application/json", "{\"status\":\"muted\"}");
            return;
        }
        muted.remove(&ip);
    }

    // Une seule session de contrôle et une seule demande à la fois.
    if SESSION.lock().is_some() {
        super::xfer::respond(
            stream,
            409,
            "Conflict", "application/json",
            "{\"status\":\"refused\",\"error\":\"session en cours\"}",
        );
        return;
    }
    if PENDING.lock().is_some() {
        super::xfer::respond(
            stream,
            409,
            "Conflict", "application/json",
            "{\"status\":\"refused\",\"error\":\"demande déjà en cours\"}",
        );
        return;
    }

    let id = new_token();
    let (tx, rx) = std::sync::mpsc::channel::<Decision>();
    *PENDING.lock() = Some(PendingConsent {
        id: id.clone(),
        from_ip: ip.clone(),
        tx,
    });
    eprintln!("[rc] demande de contrôle de {} ({ip}) -> pop-up", req.from);
    let _ = app.emit(
        "rc-incoming",
        serde_json::json!({ "id": id, "host": req.from }),
    );
    if let Some(w) = tauri::Manager::get_webview_window(app, "main") {
        let _ = w.set_focus();
    }

    match rx.recv_timeout(Duration::from_secs(RC_TIMEOUT_SECS)) {
        Ok(Decision::Accept { kb, mouse }) => {
            clear_pending_if_mine(&id);
            let token = new_token();
            let session = Arc::new(ActiveSession {
                token,
                ip,
                kb,
                mouse,
                store: Arc::new(FrameStore::new()),
                alive: Arc::new(AtomicBool::new(true)),
            });
            *SESSION.lock() = Some(session.clone());
            {
                let store = session.store.clone();
                let alive = session.alive.clone();
                std::thread::spawn(move || run_capture(store, alive));
            }
            let dims = screen_size();
            super::xfer::respond(
                stream,
                200,
                "OK",
                "application/json",
                &serde_json::json!({
                    "status": "accepted",
                    "token": session.token,
                    "kb": kb,
                    "mouse": mouse,
                    "width": dims.0,
                    "height": dims.1,
                })
                .to_string(),
            );
        }
        Ok(Decision::Refuse) => {
            clear_pending_if_mine(&id);
            super::xfer::respond(stream, 403, "Forbidden", "application/json", "{\"status\":\"refused\"}");
        }
        Err(_) => {
            // Ne voler que la demande qui est encore la nôtre : une demande
            // plus récente a pu remplacer la nôtre dans l'intervalle.
            clear_pending_if_mine(&id);
            let _ = app.emit("rc-incoming-closed", serde_json::json!({ "id": id }));
            super::xfer::respond(
                stream,
                408,
                "Request Timeout", "application/json",
                "{\"status\":\"refused\",\"error\":\"aucune réponse\"}",
            );
        }
    }
}

/// Réponse de la pop-up côté cible. `mute` = refuser en plus d'ignorer
/// l'appareil demandeur pendant 15 min.
#[tauri::command]
pub fn rc_respond(id: String, allow: bool, kb: bool, mouse: bool, mute: bool) -> Result<(), String> {
    let mut g = PENDING.lock();
    let mine = g.as_ref().is_some_and(|p| p.id == id);
    if !mine {
        return Err("aucune demande en cours".into());
    }
    let p = g.take().expect("vérifié ci-dessus");
    drop(g);
    if mute && !allow {
        muted()
            .lock()
            .insert(p.from_ip.clone(), Instant::now() + Duration::from_secs(MUTE_MINS * 60));
    }
    let d = if allow {
        Decision::Accept { kb, mouse }
    } else {
        Decision::Refuse
    };
    let _ = p.tx.send(d);
    Ok(())
}

pub(crate) fn handle_stream(app: &tauri::AppHandle, stream: &mut TcpStream, query: &str) {
    let Some(session) = auth_session(query, stream) else { return };
    let head = "\
        HTTP/1.1 200 OK\r\n\
        Content-Type: multipart/x-mixed-replace; boundary=bcframe\r\n\
        Cache-Control: no-cache, no-store\r\n\
        Connection: close\r\n\r\n";
    if stream.write_all(head.as_bytes()).is_err() {
        return;
    }
    let mut last_version: u64 = 0;
    loop {
        if !session.alive.load(Ordering::SeqCst) {
            break;
        }
        match session.store.snapshot(last_version) {
            Some((v, jpeg)) => {
                last_version = v;
                if jpeg.is_empty() {
                    std::thread::sleep(Duration::from_millis(80));
                    continue;
                }
                let part = format!(
                    "--bcframe\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                    jpeg.len()
                );
                if stream.write_all(part.as_bytes()).is_err()
                    || stream.write_all(&jpeg).is_err()
                    || stream.write_all(b"\r\n").is_err()
                    || stream.flush().is_err()
                {
                    break;
                }
            }
            None => std::thread::sleep(Duration::from_millis(35)),
        }
    }
    end_session(app);
}

#[derive(Deserialize, Serialize)]
struct RcEvent {
    kind: String,
    #[serde(default)]
    x: f64,
    #[serde(default)]
    y: f64,
    #[serde(default)]
    button: String,
    #[serde(default)]
    down: bool,
    #[serde(default)]
    dy: i16,
    #[serde(default)]
    scancode: u8,
    #[serde(default)]
    extended: bool,
    #[serde(default)]
    release: bool,
}

pub(crate) fn handle_input(app: &tauri::AppHandle, stream: &mut TcpStream, query: &str) {
    let Some(session) = auth_session(query, stream) else { return };
    let _ = stream.set_read_timeout(Some(Duration::from_secs(600)));
    if stream
        .write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/x-ndjson\r\nConnection: close\r\n\r\n")
        .is_err()
    {
        end_session(app);
        return;
    }
    let reader = BufReader::new(stream.try_clone().expect("clone du socket"));
    for line in reader.lines().map_while(Result::ok) {
        if line.trim().is_empty() {
            continue;
        }
        if !session.alive.load(Ordering::SeqCst) {
            break;
        }
        let Ok(ev) = serde_json::from_str::<RcEvent>(&line) else {
            continue;
        };
        apply_input(&session, &ev);
    }
    end_session(app);
}

fn auth_session(query: &str, stream: &mut TcpStream) -> Option<Arc<ActiveSession>> {
    let token = super::xfer::q_value(query, "t").unwrap_or_default();
    let guard = SESSION.lock();
    let Some(session) = guard.as_ref() else {
        super::xfer::respond(stream, 404, "Not Found", "application/json", "{\"error\":\"pas de session\"}");
        return None;
    };
    let Ok(peer) = stream.peer_addr() else { return None };
    if session.token != token || peer.ip().to_string() != session.ip {
        super::xfer::respond(stream, 403, "Forbidden", "application/json", "{\"error\":\"jeton invalide\"}");
        return None;
    }
    Some(session.clone())
}

#[cfg(windows)]
fn apply_input(session: &ActiveSession, ev: &RcEvent) {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, INPUT_MOUSE, KEYBDINPUT, MOUSEINPUT, KEYEVENTF_EXTENDEDKEY,
        KEYEVENTF_KEYUP, KEYEVENTF_SCANCODE, MOUSEEVENTF_ABSOLUTE, MOUSEEVENTF_LEFTDOWN,
        MOUSEEVENTF_LEFTUP, MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP, MOUSEEVENTF_MOVE,
        MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP, MOUSEEVENTF_WHEEL,
    };

    let send_mouse = |mi: MOUSEINPUT| {
        let inp = INPUT {
            r#type: INPUT_MOUSE,
            Anonymous: INPUT_0 { mi },
        };
        unsafe {
            let _ = SendInput(&[inp], std::mem::size_of::<INPUT>() as i32);
        }
    };
    let send_key = |ki: KEYBDINPUT| {
        let inp = INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 { ki },
        };
        unsafe {
            let _ = SendInput(&[inp], std::mem::size_of::<INPUT>() as i32);
        }
    };

    match ev.kind.as_str() {
        "move" => {
            if !session.mouse {
                return;
            }
            let (w, h) = screen_size();
            let dx = (ev.x.clamp(0.0, w as f64 - 0.001) / w as f64 * 65535.0) as i32;
            let dy = (ev.y.clamp(0.0, h as f64 - 0.001) / h as f64 * 65535.0) as i32;
            send_mouse(MOUSEINPUT {
                dx,
                dy,
                mouseData: 0,
                dwFlags: MOUSEEVENTF_MOVE | MOUSEEVENTF_ABSOLUTE,
                time: 0,
                dwExtraInfo: 0,
            });
        }
        "button" => {
            if !session.mouse {
                return;
            }
            let (down_flag, up_flag) = match ev.button.as_str() {
                "middle" => (MOUSEEVENTF_MIDDLEDOWN, MOUSEEVENTF_MIDDLEUP),
                "right" => (MOUSEEVENTF_RIGHTDOWN, MOUSEEVENTF_RIGHTUP),
                _ => (MOUSEEVENTF_LEFTDOWN, MOUSEEVENTF_LEFTUP),
            };
            send_mouse(MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: 0,
                dwFlags: if ev.down { down_flag } else { up_flag },
                time: 0,
                dwExtraInfo: 0,
            });
        }
        "wheel" => {
            if !session.mouse {
                return;
            }
            send_mouse(MOUSEINPUT {
                dx: 0,
                dy: 0,
                mouseData: (ev.dy as i32 * 120) as u32,
                dwFlags: MOUSEEVENTF_WHEEL,
                time: 0,
                dwExtraInfo: 0,
            });
        }
        "key" => {
            if !session.kb {
                return;
            }
            let mut flags = KEYEVENTF_SCANCODE;
            if ev.extended {
                flags |= KEYEVENTF_EXTENDEDKEY;
            }
            if ev.release {
                flags |= KEYEVENTF_KEYUP;
            }
            send_key(KEYBDINPUT {
                wVk: Default::default(),
                wScan: ev.scancode as u16,
                dwFlags: flags,
                time: 0,
                dwExtraInfo: 0,
            });
        }
        _ => {}
    }
}

#[cfg(not(windows))]
fn apply_input(_session: &ActiveSession, _ev: &RcEvent) {}

#[cfg(windows)]
fn screen_size() -> (u16, u16) {
    use windows::Win32::UI::WindowsAndMessaging::{GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
    unsafe {
        (
            GetSystemMetrics(SM_CXSCREEN).max(1) as u16,
            GetSystemMetrics(SM_CYSCREEN).max(1) as u16,
        )
    }
}

#[cfg(not(windows))]
fn screen_size() -> (u16, u16) {
    (1280, 720)
}

#[cfg(windows)]
fn run_capture(store: Arc<FrameStore>, alive: Arc<AtomicBool>) {
    use windows::Win32::Graphics::Gdi::{
        BitBlt, CreateCompatibleBitmap, CreateCompatibleDC, DeleteDC, DeleteObject, GetDC, GetDIBits,
        ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, SRCCOPY,
    };

    let hdc = unsafe { GetDC(None) };
    if hdc.is_invalid() {
        return;
    }
    let (w, h) = screen_size();
    let mem = unsafe { CreateCompatibleDC(Some(hdc)) };
    let hbmp = unsafe { CreateCompatibleBitmap(hdc, w as i32, h as i32) };
    let old = unsafe { SelectObject(mem, hbmp.into()) };

    let mut bi = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w as i32,
            biHeight: -(h as i32),
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut pixels = vec![0u8; w as usize * h as usize * 4];
    let stride = w as usize * 4;

    let frame_interval = Duration::from_millis(1000 / STREAM_FPS.max(1));
    while alive.load(Ordering::SeqCst) {
        let started = Instant::now();
        unsafe {
            let _ = BitBlt(
                mem,
                0,
                0,
                w as i32,
                h as i32,
                Some(hdc),
                0,
                0,
                SRCCOPY,
            );
            let ok = GetDIBits(
                mem,
                hbmp,
                0,
                h as u32,
                Some(pixels.as_mut_ptr().cast()),
                &mut bi,
                DIB_RGB_COLORS,
            );
            if ok == 0 {
                break;
            }
        }
        let mut jpeg = Vec::with_capacity(stride * h as usize / 12 + 1024);
        if jpeg_encoder::Encoder::new(&mut jpeg, 60)
            .encode(&pixels, w, h, jpeg_encoder::ColorType::Bgra)
            .is_ok()
        {
            store.publish(jpeg);
        }
        let spent = started.elapsed();
        if spent < frame_interval {
            std::thread::sleep(frame_interval - spent);
        }
    }

    unsafe {
        SelectObject(mem, old);
        let _ = DeleteObject(hbmp.into());
        let _ = DeleteDC(mem);
        ReleaseDC(None, hdc);
    }
}

// Côté visionneur : demande, relais MJPEG local, canal d'entrées
// ---------------------------------------------------------------------------

#[derive(Serialize)]
pub struct RdcStartResp {
    pub port: u16,
    pub width: u16,
    pub height: u16,
    pub kb: bool,
    pub mouse: bool,
}

struct ViewSession {
    input: Option<TcpStream>,
    relay_alive: Arc<AtomicBool>,
}

static VIEW: Mutex<Option<ViewSession>> = Mutex::new(None);

fn post_json(
    ip: &str,
    path: &str,
    body: &str,
    timeout: Duration,
) -> Result<(u16, serde_json::Value), String> {
    let mut s = super::xfer::connect_to(ip, Duration::from_secs(4))?;
    let _ = s.set_read_timeout(Some(timeout));
    let head = format!(
        "POST {path} HTTP/1.1\r\nHost: taildesk\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        body.len()
    );
    s.write_all(head.as_bytes()).map_err(|e| e.to_string())?;
    s.write_all(body.as_bytes()).map_err(|e| e.to_string())?;
    let (resp_head, leftover) = super::xfer::read_head(&mut s)?;
    let code = resp_head
        .lines()
        .next()
        .and_then(|l| l.split_whitespace().nth(1))
        .and_then(|c| c.parse::<u16>().ok())
        .unwrap_or(0);
    let bytes =
        super::xfer::read_n(&mut s, leftover, super::xfer::header_cl(&resp_head).unwrap_or(0))?;
    let v = serde_json::from_slice(&bytes).unwrap_or(serde_json::json!({}));
    Ok((code, v))
}

/// Demande le contrôle de `target` : la pop-up s'affiche chez l'autre appareil,
/// cette fonction ne rend la main qu'une fois la décision rendue.
#[tauri::command(async)]
pub fn rdc_start(
    app: tauri::AppHandle,
    target: String,
    host: String,
) -> Result<RdcStartResp, String> {
    rc_stop();
    let body = serde_json::json!({ "from": host }).to_string();
    let (code, v) = post_json(
        &target,
        "/rc-request",
        &body,
        Duration::from_secs(RC_TIMEOUT_SECS + 30),
    )?;
    if code != 200 {
        let status = v["status"].as_str().unwrap_or("refused");
        let detail = v["error"].as_str().unwrap_or("");
        let msg = match (status, detail.is_empty()) {
            ("muted", _) => "muted".to_string(),
            (_, false) => format!("{status}: {detail}"),
            _ => status.to_string(),
        };
        return Err(msg);
    }
    let token = v["token"].as_str().unwrap_or_default().to_string();
    let resp = RdcStartResp {
        port: 0,
        width: v["width"].as_u64().unwrap_or(1280) as u16,
        height: v["height"].as_u64().unwrap_or(720) as u16,
        kb: v["kb"].as_bool().unwrap_or(false),
        mouse: v["mouse"].as_bool().unwrap_or(false),
    };

    // Canal d'entrées persistant (NDJSON, une ligne par événement).
    let mut input_sock = super::xfer::connect_to(&target, Duration::from_secs(4))?;
    let _ = input_sock.set_read_timeout(Some(Duration::from_secs(600)));
    input_sock
        .write_all(
            format!(
                "POST /rc-input?t={token} HTTP/1.1\r\nHost: taildesk\r\nContent-Type: application/x-ndjson\r\nConnection: close\r\n\r\n"
            )
            .as_bytes(),
        )
        .map_err(|e| e.to_string())?;

    // Relais MJPEG : le webview n'a le droit qu'à 127.0.0.1 (CSP), donc on
    // re-sert localement le flux distant.
    let relay_alive = Arc::new(AtomicBool::new(true));
    let (port_tx, port_rx) = std::sync::mpsc::channel::<u16>();
    {
        let token = token.clone();
        let target = target.clone();
        let alive = relay_alive.clone();
        let app2 = app.clone();
        std::thread::spawn(move || {
            let _ = run_relay(&target, &token, port_tx, alive, &app2);
        });
    }
    let port = port_rx
        .recv_timeout(Duration::from_secs(8))
        .map_err(|_| "relais impossible".to_string())?;

    *VIEW.lock() = Some(ViewSession {
        input: Some(input_sock),
        relay_alive,
    });

    Ok(RdcStartResp { port, ..resp })
}

fn run_relay(
    target: &str,
    token: &str,
    port_tx: std::sync::mpsc::Sender<u16>,
    alive: Arc<AtomicBool>,
    app: &tauri::AppHandle,
) -> Result<(), String> {
    let mut remote = super::xfer::connect_to(target, Duration::from_secs(6))?;
    let _ = remote.set_read_timeout(Some(Duration::from_secs(15)));
    remote
        .write_all(
            format!(
                "GET /rc-stream?t={token} HTTP/1.1\r\nHost: taildesk\r\nConnection: close\r\n\r\n"
            )
            .as_bytes(),
        )
        .map_err(|e| e.to_string())?;

    let listener = TcpListener::bind("127.0.0.1:0").map_err(|e| e.to_string())?;
    port_tx
        .send(listener.local_addr().map_err(|e| e.to_string())?.port())
        .map_err(|_| "viewer parti".to_string())?;
    let (mut client, _) = listener.accept().map_err(|e| e.to_string())?;
    let _ = client.set_read_timeout(Some(Duration::from_secs(15)));

    let mut buf = [0u8; 65536];
    loop {
        if !alive.load(Ordering::SeqCst) {
            break;
        }
        match remote.read(&mut buf) {
            Ok(0) => {
                let _ = app.emit(
                    "rc-status",
                    serde_json::json!({ "stage": "ended", "error": false }),
                );
                break;
            }
            Ok(n) => {
                if client.write_all(&buf[..n]).is_err() {
                    break;
                }
            }
            Err(ref e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                continue;
            }
            Err(_) => break,
        }
    }
    alive.store(false, Ordering::SeqCst);
    end_session_quiet();
    Ok(())
}

#[tauri::command]
pub fn rdc_input(evt: serde_json::Value) -> Result<(), String> {
    let mut guard = VIEW.lock();
    let Some(view) = guard.as_mut() else {
        return Ok(());
    };
    let Some(sock) = view.input.as_mut() else {
        return Ok(());
    };
    let mut line = evt.to_string();
    line.push('\n');
    sock.write_all(line.as_bytes())
        .map_err(|_| "connexion perdue".to_string())?;
    Ok(())
}

#[tauri::command]
pub fn rc_stop() {
    let mut guard = VIEW.lock();
    if let Some(mut view) = guard.take() {
        view.relay_alive.store(false, Ordering::SeqCst);
        view.input = None;
    }
}
