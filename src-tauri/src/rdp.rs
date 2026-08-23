use std::io::{ErrorKind, Read, Write};
use std::net::{TcpListener, TcpStream, ToSocketAddrs};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex as StdMutex};
use std::time::{Duration, Instant};

use ironrdp::connector::{self, ConnectionResult};
use ironrdp::graphics::image_processing::PixelFormat as GfxPixelFormat;
use ironrdp::pdu::gcc::KeyboardType;
use ironrdp::pdu::input::fast_path::{FastPathInputEvent, KeyboardFlags};
use ironrdp::pdu::input::mouse::PointerFlags;
use ironrdp::pdu::input::MousePdu;
use ironrdp::pdu::{Encode, WriteCursor};
use ironrdp::session::image::DecodedImage;
use ironrdp::session::{ActiveStageBuilder, ActiveStageOutput};
use ironrdp::pdu::rdp::capability_sets::MajorPlatformType;
use serde::{Deserialize, Serialize};
use sspi::network_client::reqwest_network_client::ReqwestNetworkClient;

struct FrameStore {
    jpeg: StdMutex<Vec<u8>>,
    version: AtomicU64,
}

impl FrameStore {
    fn new() -> Self {
        Self {
            jpeg: StdMutex::new(Vec::new()),
            version: AtomicU64::new(0),
        }
    }

    fn publish(&self, data: Vec<u8>) {
        *self.jpeg.lock().unwrap() = data;
        self.version.fetch_add(1, Ordering::SeqCst);
    }

    fn snapshot(&self, last_seen: u64) -> Option<(u64, Vec<u8>)> {
        let v = self.version.load(Ordering::SeqCst);
        if v == last_seen {
            return None;
        }
        Some((v, self.jpeg.lock().unwrap().clone()))
    }
}

enum RdpIn {
    Input(RdpInputEvent),
    Shutdown,
}

struct SessionHandle {
    tx: StdMutex<std::sync::mpsc::Sender<RdpIn>>,
    alive: Arc<AtomicBool>,
}

static SESSION: StdMutex<Option<SessionHandle>> = StdMutex::new(None);

fn stop_session() {
    let mut guard = SESSION.lock().unwrap();
    if let Some(h) = guard.take() {
        let _ = h.tx.lock().unwrap().send(RdpIn::Shutdown);
        h.alive.store(false, Ordering::SeqCst);
    }
}

#[derive(Serialize)]
pub struct RdpStartResp {
    pub port: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Deserialize)]
pub struct RdpInputEvent {

    pub kind: String,
    pub x: Option<f64>,
    pub y: Option<f64>,

    #[serde(default)]
    pub button: Option<String>,
    #[serde(default)]
    pub down: bool,

    #[serde(default)]
    pub dy: i16,

    #[serde(default)]
    pub scancode: u8,
    #[serde(default)]
    pub extended: bool,
    #[serde(default)]
    pub release: bool,
}

fn emit_status(app: &tauri::AppHandle, stage: &str, message: &str, is_error: bool) {
    use tauri::Emitter as _;
    let _ = app.emit(
        "rdp-status",
        serde_json::json!({ "stage": stage, "message": message, "error": is_error }),
    );
}

#[tauri::command]
pub fn rdp_start(
    app: tauri::AppHandle,
    target: String,
    username: String,
    password: String,
    width: u16,
    height: u16,
) -> Result<RdpStartResp, String> {
    if !crate::is_safe_host(&target) {
        return Err("Hôte invalide.".into());
    }
    if username.trim().is_empty() || width < 640 || height < 480 {
        return Err("Paramètres de connexion invalides.".into());
    }

    stop_session();

    let store = Arc::new(FrameStore::new());
    let alive = Arc::new(AtomicBool::new(true));
    let (tx, rx) = std::sync::mpsc::channel::<RdpIn>();
    let (port_tx, port_rx) = std::sync::mpsc::channel::<u16>();

    {
        let store = store.clone();
        let alive = alive.clone();
        std::thread::spawn(move || serve_mjpeg(store, alive, port_tx));
    }

    {
        let store = store.clone();
        let alive = alive.clone();
        let app = app.clone();
        let target = target.clone();
        let username = username.clone();
        std::thread::spawn(move || {
            let result = run_session(
                &app, &store, &rx, alive.clone(), &target, &username, &password, width, height,
            );
            alive.store(false, Ordering::SeqCst);
            match result {
                Ok(()) => emit_status(&app, "closed", "Session terminée", false),
                Err(e) => {
                    emit_status(&app, "error", &e, true);
                    eprintln!("RDP : {e}");
                }
            }
        });
    }

    let port = match port_rx.recv_timeout(Duration::from_secs(4)) {
        Ok(p) => p,
        Err(_) => return Err("Le serveur vidéo local n'a pas démarré.".into()),
    };

    SESSION
        .lock()
        .unwrap()
        .replace(SessionHandle { tx: StdMutex::new(tx), alive });

    Ok(RdpStartResp { port, width, height })
}

#[tauri::command]
pub fn rdp_stop() -> Result<(), String> {
    stop_session();
    Ok(())
}

#[tauri::command]
pub fn rdp_input(evt: RdpInputEvent) -> Result<(), String> {
    let guard = SESSION.lock().unwrap();
    match guard.as_ref() {
        Some(h) if h.alive.load(Ordering::SeqCst) => {
            h.tx.lock()
                .unwrap()
                .send(RdpIn::Input(evt))
                .map_err(|_| "Session RDP fermée".to_string())
        }
        _ => Err("Aucune session RDP active".into()),
    }
}

fn translate(evt: &RdpInputEvent, w: u16, h: u16) -> Vec<FastPathInputEvent> {
    match evt.kind.as_str() {
        "move" => {
            let x = evt.x.unwrap_or(0.0).clamp(0.0, f64::from(w - 1)) as u16;
            let y = evt.y.unwrap_or(0.0).clamp(0.0, f64::from(h - 1)) as u16;
            vec![FastPathInputEvent::MouseEvent(MousePdu {
                flags: PointerFlags::MOVE,
                number_of_wheel_rotation_units: 0,
                x_position: x,
                y_position: y,
            })]
        }
        "button" => {
            let flags = match evt.button.as_deref() {
                Some("left") => PointerFlags::LEFT_BUTTON,
                Some("right") => PointerFlags::RIGHT_BUTTON,
                _ => PointerFlags::MIDDLE_BUTTON_OR_WHEEL,
            };
            let flags = if evt.down {
                flags | PointerFlags::DOWN
            } else {
                flags
            };
            let x = evt.x.unwrap_or(0.0).clamp(0.0, f64::from(w - 1)) as u16;
            let y = evt.y.unwrap_or(0.0).clamp(0.0, f64::from(h - 1)) as u16;
            vec![FastPathInputEvent::MouseEvent(MousePdu {
                flags,
                number_of_wheel_rotation_units: 0,
                x_position: x,
                y_position: y,
            })]
        }
        "wheel" => {

            let units = evt.dy.clamp(-120, 120);
            let mut flags = PointerFlags::VERTICAL_WHEEL;
            if units < 0 {
                flags |= PointerFlags::WHEEL_NEGATIVE;
            }
            vec![FastPathInputEvent::MouseEvent(MousePdu {
                flags,
                number_of_wheel_rotation_units: units.abs(),
                x_position: 0,
                y_position: 0,
            })]
        }
        "key" => {
            let mut flags = KeyboardFlags::empty();
            if evt.release {
                flags |= KeyboardFlags::RELEASE;
            }
            if evt.extended {
                flags |= KeyboardFlags::EXTENDED;
            }
            vec![FastPathInputEvent::KeyboardEvent(flags, evt.scancode)]
        }
        _ => Vec::new(),
    }
}

fn encode_events(events: &[FastPathInputEvent]) -> Vec<u8> {
    let mut out = Vec::new();
    for e in events {
        let size = e.size();
        let mut buf = vec![0u8; size];
        let mut cursor = WriteCursor::new(&mut buf);

        let _ = e.encode(&mut cursor);
        out.extend_from_slice(&buf);
    }
    out
}

type UpgradedFramed =
    ironrdp_blocking::Framed<tokio_rustls::rustls::StreamOwned<tokio_rustls::rustls::ClientConnection, TcpStream>>;

#[allow(clippy::too_many_lines)]
fn run_session(
    app: &tauri::AppHandle,
    store: &Arc<FrameStore>,
    rx: &std::sync::mpsc::Receiver<RdpIn>,
    alive: Arc<AtomicBool>,
    target: &str,
    username: &str,
    password: &str,
    width: u16,
    height: u16,
) -> Result<(), String> {
    emit_status(app, "connect", format!("Connexion à {target}…").as_str(), false);

    let connector_config = build_config(username, password, width, height)?;

    let server_addr = (target, 3389_u16)
        .to_socket_addrs()
        .map_err(|e| format!("Résolution impossible : {e}"))?
        .next()
        .ok_or("Adresse introuvable")?;

    let tcp_stream =
        TcpStream::connect_timeout(&server_addr, Duration::from_secs(6)).map_err(|e| {
            alive.store(false, Ordering::SeqCst);
            format!("Connexion TCP refusée ({e})")
        })?;
    tcp_stream
        .set_nodelay(true)
        .map_err(|e| format!("set_nodelay : {e}"))?;

    let client_addr = tcp_stream.local_addr().map_err(|e| e.to_string())?;

    let mut framed = ironrdp_blocking::Framed::new(tcp_stream);

    let mut connector = connector::ClientConnector::new(connector_config, client_addr);

    emit_status(app, "negotiate", "Négociation du protocole…", false);

    let should_upgrade = ironrdp_blocking::connect_begin(&mut framed, &mut connector)
        .map_err(|e| format!("Poignée de main échouée : {e}"))?;

    let initial_stream = framed.into_inner_no_leftover();

    emit_status(app, "tls", "Chiffrement TLS…", false);

    let (upgraded_stream, server_public_key) =
        tls_upgrade(initial_stream, target.to_string()).map_err(|e| format!("TLS : {e}"))?;

    let upgraded = ironrdp_blocking::mark_as_upgraded(should_upgrade, &mut connector);

    let mut upgraded_framed = UpgradedFramed::new(upgraded_stream);

    let mut network_client = ReqwestNetworkClient;

    emit_status(app, "auth", "Authentification (CredSSP)…", false);

    let server_name = connector::ServerName::new(target.to_string());

    let connection_result: ConnectionResult = ironrdp_blocking::connect_finalize(
        upgraded,
        connector,
        &mut upgraded_framed,
        &mut network_client,
        server_name,
        server_public_key,
        None,
    )
    .map_err(|e| format!("Authentification refusée : {e}"))?;

    let dw = connection_result.desktop_size.width;
    let dh = connection_result.desktop_size.height;

    emit_status(
        app,
        "active",
        &format!("Bureau distant connecté ({dw}×{dh})"),
        false,
    );

    let mut image = DecodedImage::new(GfxPixelFormat::RgbA32, dw, dh);

    let mut active_stage = ActiveStageBuilder {
        static_channels: connection_result.static_channels,
        user_channel_id: connection_result.user_channel_id,
        io_channel_id: connection_result.io_channel_id,
        message_channel_id: connection_result.message_channel_id,
        share_id: connection_result.share_id,
        compression_type: connection_result.compression_type,
        enable_server_pointer: connection_result.enable_server_pointer,
        pointer_software_rendering: connection_result.pointer_software_rendering,
    }
    .build();

    {
        let (inner_stream, _) = upgraded_framed.get_inner();
        let _ = inner_stream.sock.set_read_timeout(Some(Duration::from_millis(60)));
    }

    let mut last_publish = Instant::now() - Duration::from_secs(1);

    loop {
        if !alive.load(Ordering::SeqCst) {
            return Ok(());
        }

        loop {
            match rx.try_recv() {
                Ok(RdpIn::Shutdown) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    return Ok(())
                }
                Ok(RdpIn::Input(evt)) => {
                    let events = translate(&evt, dw, dh);
                    if !events.is_empty() {
                        let bytes = encode_events(&events);
                        if let Err(e) = upgraded_framed.write_all(&bytes) {
                            return Err(format!("Flux interrompu (entrée) : {e}"));
                        }
                    }
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => break,
            }
        }

        let (action, payload) = match upgraded_framed.read_pdu() {
            Ok(x) => x,
            Err(e) if e.kind() == ErrorKind::WouldBlock => {
                publish_if_due(store, &image, &mut last_publish);
                continue;
            }
            Err(e) => return Err(format!("Flux interrompu : {e}")),
        };

        let outputs = active_stage
            .process(&mut image, action, &payload)
            .map_err(|e| format!("Protocole : {e}"))?;

        for out in outputs {
            match out {
                ActiveStageOutput::ResponseFrame(frame) => {
                    upgraded_framed
                        .write_all(&frame)
                        .map_err(|e| format!("Écriture réponse : {e}"))?;
                }
                ActiveStageOutput::Terminate(_) => return Ok(()),
                _ => {}
            }
        }

        publish_if_due(store, &image, &mut last_publish);
    }
}

fn publish_if_due(store: &FrameStore, image: &DecodedImage, last_publish: &mut Instant) {
    if last_publish.elapsed() < Duration::from_millis(90) {
        return;
    }
    *last_publish = Instant::now();

    let (w, h) = (image.width(), image.height());
    if w == 0 || h == 0 {
        return;
    }

    let mut jpeg = Vec::with_capacity(64 * 1024);
    {
        let encoder = jpeg_encoder::Encoder::new(&mut jpeg, 68);
        if encoder
            .encode(image.data(), w, h, jpeg_encoder::ColorType::Rgba)
            .is_err()
        {
            return;
        }
    }
    store.publish(jpeg);
}

fn build_config(username: &str, password: &str, width: u16, height: u16) -> Result<connector::Config, String> {
    Ok(connector::Config {
        desktop_size: connector::DesktopSize { width, height },
        desktop_scale_factor: 0,
        enable_tls: true,
        enable_credssp: true,
        credentials: connector::Credentials::UsernamePassword {
            username: username.to_owned(),
            password: password.to_owned(),
        },
        domain: None,
        client_build: 100,
        client_name: "BrainConnect".to_owned(),
        keyboard_type: KeyboardType::IbmEnhanced,
        keyboard_subtype: 0,
        keyboard_functional_keys_count: 12,
        keyboard_layout: 0,
        ime_file_name: String::new(),
        bitmap: None,
        dig_product_id: String::new(),
        client_dir: "C:\\Windows\\System32\\mstscax.dll".to_owned(),
        alternate_shell: String::new(),
        work_dir: String::new(),
        platform: MajorPlatformType::WINDOWS,
        hardware_id: None,
        request_data: None,
        autologon: false,
        enable_audio_playback: false,
        performance_flags: ironrdp::pdu::rdp::client_info::PerformanceFlags::default(),
        license_cache: None,
        timezone_info: ironrdp::pdu::rdp::client_info::TimezoneInfo::default(),
        compression_type: None,
        enable_server_pointer: false,
        pointer_software_rendering: true,
        multitransport_flags: None,
    })
}

fn tls_upgrade(
    stream: TcpStream,
    server_name: String,
) -> Result<
    (
        tokio_rustls::rustls::StreamOwned<
            tokio_rustls::rustls::ClientConnection,
            TcpStream,
        >,
        Vec<u8>,
    ),
    String,
> {
    let mut config = tokio_rustls::rustls::client::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(danger::NoCertificateVerification))
        .with_no_client_auth();

    config.resumption = tokio_rustls::rustls::client::Resumption::disabled();

    let config = Arc::new(config);

    let server_name: tokio_rustls::rustls::pki_types::ServerName<'_> = server_name
        .clone()
        .try_into()
        .map_err(|_| format!("Nom de serveur TLS invalide : {server_name}"))?;

    let client = tokio_rustls::rustls::ClientConnection::new(config, server_name)
        .map_err(|e| e.to_string())?;

    let mut tls_stream = tokio_rustls::rustls::StreamOwned::new(client, stream);
    tls_stream.flush().map_err(|e| e.to_string())?;

    let cert = tls_stream
        .conn
        .peer_certificates()
        .and_then(|certs| certs.first())
        .ok_or("Certificat serveur manquant")?;

    let server_public_key = extract_tls_server_public_key(cert)?;

    Ok((tls_stream, server_public_key))
}

fn extract_tls_server_public_key(cert: &[u8]) -> Result<Vec<u8>, String> {
    use x509_cert::der::Decode as _;

    let cert =
        x509_cert::Certificate::from_der(cert).map_err(|e| format!("Certificat illisible : {e}"))?;

    cert.tbs_certificate()
        .subject_public_key_info()
        .subject_public_key
        .as_bytes()
        .map(<[u8]>::to_vec)
        .ok_or_else(|| "Clé publique mal alignée".to_string())
}

mod danger {
    use std::sync::Arc;

    use tokio_rustls::rustls::client::danger::{
        HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier,
    };
    use tokio_rustls::rustls::pki_types;
    use tokio_rustls::rustls::{DigitallySignedStruct, Error, SignatureScheme};

    #[derive(Debug)]
    pub(super) struct NoCertificateVerification;

    impl ServerCertVerifier for NoCertificateVerification {
        fn verify_server_cert(
            &self,
            _: &pki_types::CertificateDer<'_>,
            _: &[pki_types::CertificateDer<'_>],
            _: &pki_types::ServerName<'_>,
            _: &[u8],
            _: pki_types::UnixTime,
        ) -> Result<ServerCertVerified, Error> {
            Ok(ServerCertVerified::assertion())
        }

        fn verify_tls12_signature(
            &self,
            _: &[u8],
            _: &pki_types::CertificateDer<'_>,
            _: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, Error> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn verify_tls13_signature(
            &self,
            _: &[u8],
            _: &pki_types::CertificateDer<'_>,
            _: &DigitallySignedStruct,
        ) -> Result<HandshakeSignatureValid, Error> {
            Ok(HandshakeSignatureValid::assertion())
        }

        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> {
            vec![
                SignatureScheme::RSA_PKCS1_SHA1,
                SignatureScheme::ECDSA_SHA1_Legacy,
                SignatureScheme::RSA_PKCS1_SHA256,
                SignatureScheme::ECDSA_NISTP256_SHA256,
                SignatureScheme::RSA_PKCS1_SHA384,
                SignatureScheme::ECDSA_NISTP384_SHA384,
                SignatureScheme::RSA_PKCS1_SHA512,
                SignatureScheme::ECDSA_NISTP521_SHA512,
                SignatureScheme::RSA_PSS_SHA256,
                SignatureScheme::RSA_PSS_SHA384,
                SignatureScheme::RSA_PSS_SHA512,
                SignatureScheme::ED25519,
                SignatureScheme::ED448,
            ]
        }
    }

    #[allow(dead_code)]
    fn _witness(_: Arc<NoCertificateVerification>) {}
}

fn serve_mjpeg(store: Arc<FrameStore>, alive: Arc<AtomicBool>, port_tx: std::sync::mpsc::Sender<u16>) {
    let listener = match TcpListener::bind(("127.0.0.1", 0)) {
        Ok(l) => l,
        Err(_) => return,
    };
    let port = match listener.local_addr() {
        Ok(a) => a.port(),
        Err(_) => return,
    };
    if port_tx.send(port).is_err() {
        return;
    }
    let _ = listener.set_nonblocking(true);

    while alive.load(Ordering::SeqCst) {
        match listener.accept() {
            Ok((stream, _)) => handle_viewer(store.clone(), stream),
            Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                std::thread::sleep(Duration::from_millis(150));
            }
            Err(_) => break,
        }
    }
}

fn handle_viewer(store: Arc<FrameStore>, mut stream: TcpStream) {

    let _ = stream.set_read_timeout(Some(Duration::from_secs(3)));
    let mut buf = [0u8; 4096];
    let mut seen = 0usize;
    loop {
        match stream.read(&mut buf[seen..]) {
            Ok(0) => return,
            Ok(n) => {
                seen += n;
                if buf[..seen].windows(4).any(|w| w == b"\r\n\r\n") || seen > 2048 {
                    break;
                }
            }
            Err(_) => return,
        }
    }

    let headers = "HTTP/1.1 200 OK\r\n\
        Content-Type: multipart/x-mixed-replace; boundary=bcframe\r\n\
        Cache-Control: no-cache, no-store\r\n\
        Pragma: no-cache\r\n\
        Connection: close\r\n\r\n";
    if stream.write_all(headers.as_bytes()).is_err() {
        return;
    }

    let mut last_version: u64 = 0;
    let mut last_sent = Instant::now();

    loop {
        match store.snapshot(last_version) {
            Some((v, jpeg)) => {
                last_version = v;
                if jpeg.is_empty() {
                    std::thread::sleep(Duration::from_millis(60));
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
                    return;
                }
                last_sent = Instant::now();
            }
            None => {

                if last_version > 0 && last_sent.elapsed() > Duration::from_millis(800) {
                    let jpeg = store.jpeg.lock().unwrap().clone();
                    if !jpeg.is_empty() {
                        let part = format!(
                            "--bcframe\r\nContent-Type: image/jpeg\r\nContent-Length: {}\r\n\r\n",
                            jpeg.len()
                        );
                        if stream.write_all(part.as_bytes()).is_err()
                            || stream.write_all(&jpeg).is_err()
                            || stream.write_all(b"\r\n").is_err()
                        {
                            return;
                        }
                        last_sent = Instant::now();
                    }
                }
                std::thread::sleep(Duration::from_millis(25));
            }
        }
    }
}
