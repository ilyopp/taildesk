#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use std::process::Command;
use std::sync::{OnceLock, Mutex as StdMutex};

#[cfg(windows)]
use std::os::windows::process::CommandExt;

pub(crate) const CREATE_NO_WINDOW: u32 = 0x0800_0000;
const CREATE_NEW_CONSOLE: u32 = 0x0000_0010;

mod embedded;
mod rdp;
mod xfer;

#[tauri::command]
fn get_default_lang() -> Result<String, String> {
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    if let Some(dir) = exe.parent() {
        if let Ok(s) = std::fs::read_to_string(dir.join("language.txt")) {
            if s.contains("rench") || s.trim() == "1036" {
                return Ok("fr".into());
            }
            return Ok("en".into());
        }
    }
    Ok("en".into())
}

static UPDATE_PENDING: OnceLock<StdMutex<Option<tauri_plugin_updater::Update>>> = OnceLock::new();

#[tauri::command]
async fn updater_check(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_updater::UpdaterExt as _;
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    match update {
        Some(u) => {
            let version = u.version.clone();
            let slot = UPDATE_PENDING.get_or_init(|| StdMutex::new(None));
            *slot.lock().unwrap() = Some(u);
            Ok(Some(version))
        }
        None => Ok(None),
    }
}

#[tauri::command]
async fn updater_install(app: tauri::AppHandle) -> Result<(), String> {
    let update = {
        let slot = UPDATE_PENDING.get_or_init(|| StdMutex::new(None));
        slot.lock().unwrap().take()
    }
    .ok_or("No update available")?;
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    app.restart();
}

fn tailscale_bin() -> &'static Option<PathBuf> {
    static TS: OnceLock<Option<PathBuf>> = OnceLock::new();
    TS.get_or_init(embedded::bundled_cli)
}

fn ts_command() -> Result<Command, String> {
    match tailscale_bin() {
        Some(p) => {
            // Socket par défaut : en mode service Windows, tailscaled n'applique
            // pas --socket à l'écoute après son re-exec enfant (/subproc).
            let mut c = Command::new(&p);
            #[cfg(windows)]
            c.creation_flags(CREATE_NO_WINDOW);
            Ok(c)
        }
        None => Err("Binaires Taildesk introuvables, réinstalle l'application.".into()),
    }
}

fn is_safe_host(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 253
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '@' | '_'))
}

#[derive(Deserialize)]
struct TsStatus {
    #[serde(rename = "BackendState")]
    backend_state: Option<String>,
    #[serde(rename = "Version")]
    version: Option<String>,
    #[serde(rename = "MagicDNSSuffix")]
    magicdns_suffix: Option<String>,
    #[serde(rename = "Self")]
    self_peer: Option<TsPeer>,
    #[serde(rename = "Peer")]
    peers: Option<BTreeMap<String, TsPeer>>,
    #[serde(rename = "ExitNodeStatus", default)]
    exit_node_status: Option<Value>,
}

#[derive(Deserialize)]
struct TsPeer {
    #[serde(rename = "HostName")]
    host_name: Option<String>,
    #[serde(rename = "DNSName")]
    dns_name: Option<String>,
    #[serde(rename = "OS")]
    os: Option<String>,
    #[serde(rename = "TailscaleIPs")]
    ips: Option<Vec<String>>,
    #[serde(rename = "Online")]
    online: Option<bool>,
    #[serde(rename = "Tags")]
    tags: Option<Vec<String>>,
    #[serde(rename = "LastSeen")]
    last_seen: Option<String>,
}

#[derive(Serialize)]
struct PeerVm {
    hostname: String,
    dns_name: String,
    os: String,
    ipv4: String,
    ipv6: String,
    online: bool,
    last_seen: String,
    tags: Vec<String>,
    is_self: bool,
}

#[derive(Serialize)]
struct StatusVm {
    backend_state: String,
    version: String,
    magicdns_suffix: String,
    exit_node: String,
    self_device: Option<PeerVm>,
    peers: Vec<PeerVm>,
}

fn split_ips(ips: &[String]) -> (String, String) {
    let mut v4 = String::new();
    let mut v6 = String::new();
    for ip in ips {
        if ip.contains(':') {
            if v6.is_empty() {
                v6 = ip.clone();
            }
        } else if v4.is_empty() {
            v4 = ip.clone();
        }
    }
    (v4, v6)
}

impl From<&TsPeer> for PeerVm {
    fn from(p: &TsPeer) -> Self {
        let (ipv4, ipv6) = split_ips(p.ips.as_deref().unwrap_or_default());
        PeerVm {
            hostname: p.host_name.clone().unwrap_or_default(),
            dns_name: p
                .dns_name
                .clone()
                .map(|d| d.trim_end_matches('.').to_string())
                .unwrap_or_default(),
            os: p.os.clone().unwrap_or_default(),
            ipv4,
            ipv6,
            online: p.online.unwrap_or(false),
            last_seen: p.last_seen.clone().unwrap_or_default(),
            tags: p.tags.clone().unwrap_or_default(),
            is_self: false,
        }
    }
}

fn local_status() -> Result<StatusVm, String> {
    let mut cmd = ts_command()?;
    cmd.args(["status", "--json"]);
    let out = embedded::run_output_timeout(&mut cmd, 8)
        .map_err(|e| format!("Impossible d'exécuter tailscale : {e}"))?;

    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }

    let status: TsStatus = serde_json::from_slice(&out.stdout)
        .map_err(|e| format!("Réponse tailscale illisible : {e}"))?;

    let self_vm = status.self_peer.as_ref().map(|p| {
        let mut vm = PeerVm::from(p);
        vm.is_self = true;
        vm.online = true;
        vm
    });

    let mut peers: Vec<PeerVm> = status
        .peers
        .unwrap_or_default()
        .values()
        .map(PeerVm::from)
        .collect();

    peers.sort_by(|a, b| {
        b.online
            .cmp(&a.online)
            .then_with(|| a.hostname.to_lowercase().cmp(&b.hostname.to_lowercase()))
    });

    let exit_node = status
        .exit_node_status
        .as_ref()
        .and_then(|v| v["TailscaleIPs"][0].as_str())
        .unwrap_or("")
        .to_string();

    Ok(StatusVm {
        backend_state: status.backend_state.unwrap_or_else(|| "Unknown".into()),
        version: status.version.unwrap_or_default(),
        magicdns_suffix: status.magicdns_suffix.unwrap_or_default(),
        exit_node,
        self_device: self_vm,
        peers,
    })
}

#[tauri::command(async)]
fn get_status() -> Result<StatusVm, String> {
    let st = local_status()?;
    if let Some(me) = &st.self_device {
        if !me.ipv4.is_empty() {
            xfer::ensure_server(&me.ipv4);
        }
    }
    Ok(st)
}

#[derive(Deserialize)]
struct TsProfile {
    id: String,
    #[serde(default)]
    tailnet: String,
    #[serde(default)]
    nickname: String,
    #[serde(rename = "selected", default)]
    selected: bool,
}

#[derive(Serialize)]
struct ProfileVm {
    id: String,
    tailnet: String,
    current: bool,
}

#[tauri::command(async)]
fn list_profiles() -> Result<Vec<ProfileVm>, String> {
    let mut cmd = ts_command()?;
    cmd.args(["switch", "--list", "--json"]);
    let out = embedded::run_output_timeout(&mut cmd, 6)
        .map_err(|e| format!("Commande impossible : {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    let profiles: Vec<TsProfile> = serde_json::from_slice(&out.stdout)
        .map_err(|e| format!("Réponse tailscale illisible : {e}"))?;
    Ok(profiles
        .into_iter()
        .map(|p| ProfileVm {
            current: p.selected,
            id: p.id,
            tailnet: if p.tailnet.is_empty() { p.nickname } else { p.tailnet },
        })
        .collect())
}

#[tauri::command(async)]
fn switch_profile(id: String) -> Result<(), String> {
    if id.is_empty() || !id.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("Identifiant de réseau invalide.".into());
    }
    let out = ts_command()?
        .args(["switch", &id])
        .output()
        .map_err(|e| format!("Commande impossible : {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(())
}

#[tauri::command(async)]
fn ping_peer(ip: String) -> Result<String, String> {
    if !is_safe_host(&ip) {
        return Err("Adresse invalide.".into());
    }
    let out = ts_command()?
        .args(["ping", "-c", "4", "--timeout", "3s", &ip])
        .output()
        .map_err(|e| format!("Ping impossible : {e}"))?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    if stdout.trim().is_empty() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(stdout.trim().to_string())
}

#[tauri::command]
fn open_ssh(host: String) -> Result<(), String> {
    if !is_safe_host(&host) {
        return Err("Hôte invalide.".into());
    }
    #[cfg(windows)]
    {
        Command::new("cmd")
            .args(["/C", "start", "", "ssh", &host])
            .creation_flags(CREATE_NEW_CONSOLE)
            .spawn()
            .map_err(|e| format!("Impossible d'ouvrir SSH : {e}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Command::new("ssh").arg(&host).spawn().map_err(|e| format!("Impossible d'ouvrir SSH : {e}"))?;
        Ok(())
    }
}

#[tauri::command]
fn open_rdp(ip: String) -> Result<(), String> {
    if !is_safe_host(&ip) {
        return Err("Adresse invalide.".into());
    }
    #[cfg(windows)]
    {
        Command::new("cmd")
            .args(["/C", "start", "", "mstsc", &format!("/v:{ip}")])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Impossible d'ouvrir le Bureau à distance : {e}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    Err("Bureau à distance disponible uniquement sous Windows.".into())
}

#[tauri::command]
fn open_browser(url: String) -> Result<(), String> {
    let url = if url.starts_with("http://") || url.starts_with("https://") {
        url
    } else {
        format!("http://{url}")
    };
    if !url.chars().all(|c| {
        c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | ':' | '/' | '_' | '?' | '=' | '&')
    }) {
        return Err("URL invalide.".into());
    }
    #[cfg(windows)]
    {
        Command::new("cmd")
            .args(["/C", "start", "", &url])
            .creation_flags(CREATE_NO_WINDOW)
            .spawn()
            .map_err(|e| format!("Impossible d'ouvrir le navigateur : {e}"))?;
        Ok(())
    }
    #[cfg(not(windows))]
    {
        Command::new("xdg-open").arg(&url).spawn().map_err(|e| format!("Impossible d'ouvrir le navigateur : {e}"))?;
        Ok(())
    }
}

#[tauri::command(async)]
fn toggle_tailscale(up: bool) -> Result<(), String> {
    let out = ts_command()?
        .args(if up { ["up"] } else { ["down"] })
        .output()
        .map_err(|e| format!("Commande impossible : {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(())
}

#[derive(Serialize)]
struct ExitNodeVm {
    host: String,
    ip: String,
    location: String,
}

#[tauri::command(async)]
fn list_exit_nodes() -> Result<Vec<ExitNodeVm>, String> {
    let out = ts_command()?
        .args(["exit-node", "list"])
        .output()
        .map_err(|e| format!("Commande impossible : {e}"))?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut nodes = Vec::new();

    for line in stdout.lines() {
        let l = line.trim();
        if l.is_empty() || l.starts_with('-') || l.to_lowercase().starts_with("ip ") {
            continue;
        }
        let cols: Vec<&str> = l.split_whitespace().collect();
        if cols.len() < 2 {
            continue;
        }
        let ip = cols[0].to_string();
        let host = cols[1].to_string();

        let looks_like_row =
            ip.chars().next().map(|c| c.is_ascii_digit()).unwrap_or(false)
                || ip.as_str() == "-";
        if !looks_like_row || !is_safe_host(&host) {
            continue;
        }
        let location = if cols.len() > 2 {
            cols[2..].join(" ")
        } else {
            String::new()
        };
        nodes.push(ExitNodeVm { host, ip, location });
    }

    Ok(nodes)
}

#[tauri::command(async)]
fn set_exit_node(node: String) -> Result<(), String> {
    let arg = format!("--exit-node={node}");
    let out = ts_command()?
        .args(["set", &arg])
        .output()
        .map_err(|e| format!("Commande impossible : {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(())
}

#[derive(Serialize)]
struct DerpLatency {
    region: String,
    #[serde(skip_serializing_if = "String::is_empty")]
    name: String,
    ms: f64,
}

#[derive(Serialize)]
struct NetcheckVm {
    udp: Option<bool>,
    ipv4: Option<bool>,
    ipv6: Option<bool>,
    nat_varies: Option<bool>,
    port_map: String,
    upnp: String,
    pmp: String,
    pcp: String,
    preferred: String,
    derps: Vec<DerpLatency>,
}

fn parse_bool(v: &str) -> Option<bool> {
    match v.trim().to_lowercase().as_str() {
        "true" | "yes" => Some(true),
        "false" | "no" => Some(false),
        _ => None,
    }
}

#[tauri::command(async)]
fn netcheck() -> Result<NetcheckVm, String> {
    let out = ts_command()?
        .arg("netcheck")
        .output()
        .map_err(|e| format!("Netcheck impossible : {e}"))?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    let mut nc = NetcheckVm {
        udp: None,
        ipv4: None,
        ipv6: None,
        nat_varies: None,
        port_map: String::new(),
        upnp: String::new(),
        pmp: String::new(),
        pcp: String::new(),
        preferred: String::new(),
        derps: Vec::new(),
    };

    let mut in_derp = false;
    for line in stdout.lines() {
        let l = line.trim().strip_prefix("* ").unwrap_or(line.trim());
        if let Some(v) = l.strip_prefix("UDP:") {
            nc.udp = parse_bool(v);
        } else if let Some(v) = l.strip_prefix("IPv4:") {
            nc.ipv4 = parse_bool(v).or(Some(v.to_lowercase().contains("yes")));
        } else if let Some(v) = l.strip_prefix("IPv6:") {
            nc.ipv6 = parse_bool(v).or(Some(v.to_lowercase().contains("yes")));
        } else if let Some(v) = l.strip_prefix("MappingVariesByDestIP:") {
            nc.nat_varies = parse_bool(v);
        } else if let Some(v) = l.strip_prefix("UPnP:") {
            nc.upnp = v.trim().to_string();
        } else if let Some(v) = l.strip_prefix("PMP:") {
            nc.pmp = v.trim().to_string();
        } else if let Some(v) = l.strip_prefix("PCP:") {
            nc.pcp = v.trim().to_string();
        } else if let Some(v) = l.strip_prefix("PortMapping:") {
            nc.port_map = v.trim().to_string();
        } else if let Some(v) = l.strip_prefix("Nearest DERP:") {
            nc.preferred = v.trim().to_string();
        } else if let Some(v) = l.strip_prefix("PreferredDERP:") {
            nc.preferred = v.trim().to_string();
        } else if l.starts_with("DERP latency") {
            in_derp = true;
        } else if in_derp {
            if let Some(rest) = l.strip_prefix("- ") {
                if let Some((region, tail)) = rest.split_once(':') {
                    let tail = tail.trim();
                    let num: String = tail
                        .chars()
                        .take_while(|c| c.is_ascii_digit() || *c == '.')
                        .collect();
                    if let Ok(ms) = num.parse::<f64>() {
                        let name = tail
                            .split_once('(')
                            .and_then(|(_, n)| n.split_once(')').map(|(n, _)| n.trim()))
                            .unwrap_or_default()
                            .to_string();
                        nc.derps.push(DerpLatency {
                            region: region.trim().to_string(),
                            name,
                            ms,
                        });
                    }
                }
            }
        }
    }

    if nc.udp.is_none() && nc.derps.is_empty() {
        let snippet: String = stdout.lines().take(4).collect::<Vec<_>>().join(" / ");
        return Err(format!("Sortie netcheck illisible : {snippet}"));
    }

    if let Some(d) = nc.derps.iter().find(|d| d.name == nc.preferred) {
        nc.preferred = d.region.clone();
    }

    nc.derps.sort_by(|a, b| a.ms.total_cmp(&b.ms));
    Ok(nc)
}

#[tauri::command]
fn pick_files() -> Result<Vec<String>, String> {
    Ok(rfd::FileDialog::new()
        .set_title("Choisir des fichiers à envoyer")
        .pick_files()
        .map(|list| list.iter().map(|p| p.display().to_string()).collect())
        .unwrap_or_default())
}

#[tauri::command]
fn pick_dir() -> Result<Option<String>, String> {
    Ok(rfd::FileDialog::new()
        .set_title("Choisir un dossier de réception")
        .pick_folder()
        .map(|p| p.display().to_string()))
}

pub(crate) fn taildrop_copy(host: &str, path: &str) -> Result<(), String> {
    if !is_safe_host(host) {
        return Err("Hôte invalide.".into());
    }
    if path.is_empty() || !std::path::Path::new(path).exists() {
        return Err("Fichier introuvable.".into());
    }
    let target = format!("{host}:");
    let out = ts_command()?
        .args(["file", "cp", path, &target])
        .output()
        .map_err(|e| format!("Envoi impossible : {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .setup(|app| {
            xfer::init(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_status,
            ping_peer,
            open_ssh,
            open_rdp,
            open_browser,
            toggle_tailscale,
            list_exit_nodes,
            set_exit_node,
            netcheck,
            pick_files,
            pick_dir,
            xfer::xfer_state,
            xfer::xfer_send,
            xfer::xfer_decide,
            xfer::xfer_prefs,
            xfer::xfer_open_dir,
            xfer::xfer_clear_history,
            list_profiles,
            switch_profile,
            rdp::rdp_start,
            rdp::rdp_stop,
            rdp::rdp_input,
            get_default_lang,
            updater_check,
            updater_install,
            embedded::ts_probe,
            embedded::ts_login
        ])
        .run(tauri::generate_context!())
        .expect("Erreur au lancement de Taildesk");
}
