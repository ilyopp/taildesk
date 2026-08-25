use serde::Serialize;
use std::path::PathBuf;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

static LOGIN_POLLING: AtomicBool = AtomicBool::new(false);
static LAST_HEAL_MS: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

pub fn bundle_dir() -> Option<PathBuf> {
    #[cfg(debug_assertions)]
    {
        let p = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tailscale-bundle");
        if p.is_dir() {
            return Some(p);
        }
    }
    #[cfg(not(debug_assertions))]
    {
        if let Some(dir) = std::env::current_exe()
            .ok()
            .and_then(|e| e.parent().map(|p| p.to_path_buf()))
        {
            let p = dir.join("tailscale-bundle");
            if p.is_dir() {
                return Some(p);
            }
        }
    }
    None
}

pub fn bundled_cli() -> Option<PathBuf> {
    let dir = bundle_dir()?;
    let cli = dir.join("tailscale.exe");
    if cli.is_file() {
        Some(cli)
    } else {
        None
    }
}

fn bundle_version() -> String {
    let Some(dir) = bundle_dir() else {
        return String::new();
    };
    std::fs::read_to_string(dir.join("VERSION"))
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

#[derive(Serialize)]
pub struct TsProbe {
    pub available: bool,
    pub source: String,
    pub version: String,
    pub backend_state: String,
}

/// Daemon injoignable : relancer la tâche planifiée Taildesk créée à
/// l'installation. Un utilisateur peut relancer ses propres tâches sans
/// élévation. Limité à une tentative toutes les 30 s.
fn try_heal_daemon() {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    if now - LAST_HEAL_MS.load(Ordering::Relaxed) < 30_000 {
        return;
    }
    LAST_HEAL_MS.store(now, Ordering::Relaxed);
    let mut c = Command::new("schtasks.exe");
    c.args(["/run", "/tn", "Taildesk"]);
    let _ = run_output_timeout(&mut c, 15);
}

#[tauri::command(async)]
pub fn ts_probe() -> TsProbe {
    if bundled_cli().is_none() {
        return TsProbe {
            available: false,
            source: "none".into(),
            version: String::new(),
            backend_state: String::new(),
        };
    }
    let state = status_state();
    if state.is_none() {
        std::thread::spawn(try_heal_daemon);
    }
    TsProbe {
        available: state.is_some(),
        source: "bundled".into(),
        version: bundle_version(),
        backend_state: state.unwrap_or_default(),
    }
}

fn status_state() -> Option<String> {
    let mut c = crate::ts_command().ok()?;
    c.args(["status", "--json"]);
    let out = run_output_timeout(&mut c, 4).ok()?;
    if !out.status.success() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_slice(&out.stdout).ok()?;
    v.get("BackendState")
        .and_then(|x| x.as_str())
        .map(|s| s.to_string())
}

fn spawn_piped(c: &mut Command) -> std::io::Result<std::process::Child> {
    #[cfg(windows)]
    {
        c.creation_flags(crate::CREATE_NO_WINDOW);
    }
    c.stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
}

/// Extrait l'URL d'authentification d'une ligne de sortie de `tailscale login`.
fn extract_auth_url(line: &str) -> Option<String> {
    let start = line.find("https://").or_else(|| line.find("http://"))?;
    let rest = &line[start..];
    if !rest.contains("/a/") && !rest.contains("login.") {
        return None;
    }
    let end = rest
        .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
        .unwrap_or(rest.len());
    Some(rest[..end].to_string())
}

/// Surveille un flux de sortie du processus de login : émet la première URL
/// d'authentification trouvée, garde la dernière ligne non vide (message
/// d'erreur éventuel du CLI) et signale la fin du flux. Le navigateur ne
/// s'ouvre jamais tout seul : l'UI affiche le lien et ne l'ouvre que sur clic.
fn watch_auth_url(
    app: &tauri::AppHandle,
    stream: impl std::io::Read + Send + 'static,
    emitted: std::sync::Arc<AtomicBool>,
    last_line: Arc<parking_lot::Mutex<String>>,
    closed: Arc<AtomicBool>,
) {
    let app = app.clone();
    std::thread::spawn(move || {
        use std::io::BufRead as _;
        let reader = std::io::BufReader::new(stream);
        for line in reader.lines().map_while(Result::ok) {
            if line.chars().any(|c| !c.is_whitespace()) {
                *last_line.lock() = line.clone();
            }
            if let Some(url) = extract_auth_url(&line) {
                if !emitted.swap(true, Ordering::SeqCst) {
                    emit_auth_url(&app, &url);
                }
            }
        }
        closed.store(true, Ordering::SeqCst);
    });
}

pub(crate) fn run_output_timeout(
    c: &mut Command,
    secs: u64,
) -> std::io::Result<std::process::Output> {
    #[cfg(windows)]
    {
        c.creation_flags(crate::CREATE_NO_WINDOW);
    }
    let mut child = c
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let deadline = std::time::Instant::now() + Duration::from_secs(secs);
    loop {
        if child.try_wait()?.is_some() {
            return child.wait_with_output();
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "délai dépassé",
            ));
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn emit_stage(app: &tauri::AppHandle, stage: &str) {
    use tauri::Emitter as _;
    let _ = app.emit("welcome-status", serde_json::json!({ "stage": stage }));
}

fn emit_auth_url(app: &tauri::AppHandle, url: &str) {
    use tauri::Emitter as _;
    let _ = app.emit(
        "welcome-status",
        serde_json::json!({ "stage": "authurl", "url": url }),
    );
}

fn emit_error(app: &tauri::AppHandle, code: &str) {
    use tauri::Emitter as _;
    let _ = app.emit(
        "welcome-status",
        serde_json::json!({ "stage": "error", "message": code }),
    );
}

#[tauri::command(async)]
pub fn ts_login(app: tauri::AppHandle) -> Result<(), String> {
    if LOGIN_POLLING.swap(true, Ordering::SeqCst) {
        return Ok(());
    }
    std::thread::spawn(move || {
        run_login(&app);
        LOGIN_POLLING.store(false, Ordering::SeqCst);
    });
    Ok(())
}

/// Connexion établie : prévenir l'UI et activer le mode unattended pour que
/// le daemon se reconnecte seul aux prochains démarrages du service.
fn emit_connected(app: &tauri::AppHandle) {
    emit_stage(app, "connected");
    std::thread::spawn(|| {
        if let Ok(mut c) = crate::ts_command() {
            c.args(["set", "--unattended"]);
            let _ = run_output_timeout(&mut c, 8);
        }
    });
}

fn run_login(app: &tauri::AppHandle) {
    if status_state().as_deref() == Some("Running") {
        emit_connected(app);
        return;
    }
    emit_stage(app, "login");
    // En mode interactif Windows, sans mode unattended, tailscaled bascule
    // sur le profil vide dès qu'aucun client n'est plus connecté : chaque
    // `status` du polling ci-dessous referait donc tomber la session de
    // login en cours ("client disconnected" -> NoState). `up --unattended`
    // pose ForceDaemon avant d'attendre l'authentification et affiche lui-
    // même l'URL : un seul processus piloté reste en vie pendant tout le flow.
    let last_line = Arc::new(parking_lot::Mutex::new(String::new()));
    let closed = Arc::new(AtomicBool::new(false));
    match crate::ts_command() {
        Ok(mut c) => {
            c.args(["up", "--unattended"]);
            match spawn_piped(&mut c) {
                Ok(mut child) => {
                    let stdout = child.stdout.take();
                    let stderr = child.stderr.take();
                    let emitted = Arc::new(AtomicBool::new(false));
                    if let Some(out) = stdout {
                        watch_auth_url(
                            app,
                            out,
                            emitted.clone(),
                            last_line.clone(),
                            closed.clone(),
                        );
                    }
                    if let Some(err) = stderr {
                        watch_auth_url(app, err, emitted, last_line.clone(), closed.clone());
                    }
                    std::thread::spawn(move || {
                        let _ = child.wait();
                    });
                }
                Err(_) => closed.store(true, Ordering::SeqCst),
            }
        }
        Err(_) => closed.store(true, Ordering::SeqCst),
    }

    let mut misses = 0u32;
    let mut gone_ticks = 0u32;
    for _ in 0..600 {
        match status_state() {
            Some(state) => {
                misses = 0;
                if state == "Running" {
                    emit_connected(app);
                    return;
                }
            }
            None => {
                misses += 1;
                if misses >= 15 {
                    // Le daemon ne répond plus : inutile d'attendre le timeout.
                    emit_error(app, "daemonLost");
                    return;
                }
            }
        }
        if closed.load(Ordering::SeqCst) {
            gone_ticks += 1;
            // `up` peut sortir dès que l'intention est posée alors que le
            // backend finit de passer Running en arrière-plan : petit délai.
            if gone_ticks >= 8 {
                let reason = last_line.lock().clone();
                let code = if reason.is_empty() { "timeout" } else { &reason };
                emit_error(app, code);
                return;
            }
        }
        std::thread::sleep(Duration::from_millis(1000));
    }
    emit_error(app, "timeout");
}
