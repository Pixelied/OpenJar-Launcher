use crate::friend_link::state;
use crate::friend_link::store::{
    get_session_mut, read_store_at_path, store_path_from_app_data, write_store_at_path,
    FriendLinkSessionRecord, FriendPeerRecord,
};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use hkdf::Hkdf;
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::{HashMap, HashSet};
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream, UdpSocket};
use std::path::PathBuf;
use std::sync::mpsc;
use std::sync::{Mutex, OnceLock};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use uuid::Uuid;

const MAX_CLOCK_SKEW_MS: i64 = 120_000;
const MAX_SEEN_NONCES: usize = 4096;
const PEER_LIMIT: usize = 8;
const FRAME_NONCE_BYTES: usize = 24;
const MAX_FRAME_BYTES: usize = 5 * 1024 * 1024;
const MAX_FRAME_PLAINTEXT_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloPayload {
    pub peer_id: String,
    pub display_name: String,
    pub endpoint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerSummary {
    pub peer_id: String,
    pub display_name: String,
    pub endpoint: String,
    pub online: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloAckPayload {
    pub peer_id: String,
    pub display_name: String,
    pub endpoint: String,
    #[serde(default)]
    pub peers: Vec<PeerSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateRequestPayload {}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateResponsePayload {
    pub peer_id: String,
    pub display_name: String,
    pub endpoint: String,
    pub state: state::SyncState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileRequestPayload {
    pub key: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileResponsePayload {
    pub key: String,
    pub found: bool,
    #[serde(default)]
    pub sha256: Option<String>,
    #[serde(default)]
    pub bytes_b64: Option<String>,
    #[serde(default)]
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SignedFrame {
    group_id: String,
    from_peer_id: String,
    timestamp_ms: i64,
    nonce: String,
    payload_type: String,
    payload: serde_json::Value,
}

struct ListenerHandle {
    port: u16,
    binding_fingerprint: String,
    stop_tx: mpsc::Sender<()>,
}

fn listener_map() -> &'static Mutex<HashMap<String, ListenerHandle>> {
    static LISTENERS: OnceLock<Mutex<HashMap<String, ListenerHandle>>> = OnceLock::new();
    LISTENERS.get_or_init(|| Mutex::new(HashMap::new()))
}

fn listener_binding_fingerprint(session: &FriendLinkSessionRecord) -> String {
    use sha2::Digest as _;
    let mut hasher = Sha256::new();
    hasher.update(session.group_id.as_bytes());
    hasher.update([0u8]);
    hasher.update(session.local_peer_id.as_bytes());
    hasher.update([0u8]);
    hasher.update(session.shared_secret_b64.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn map_secure_frame_error(err: String) -> String {
    if err
        .to_ascii_lowercase()
        .contains("frame decrypt/auth failed")
    {
        return "Secure frame authentication failed. The invite/session may be stale after credentials rotated, or traffic was tampered. Regenerate an invite and retry.".to_string();
    }
    err
}

fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|v| v.as_millis() as i64)
        .unwrap_or(0)
}

fn now_iso() -> String {
    chrono::Utc::now().to_rfc3339()
}

fn derive_frame_key(secret_b64: &str) -> Result<[u8; 32], String> {
    let secret = BASE64_STANDARD
        .decode(secret_b64)
        .map_err(|e| format!("decode shared secret failed: {e}"))?;
    let hkdf = Hkdf::<Sha256>::new(Some(b"openjar-friendlink-v2"), &secret);
    let mut key = [0u8; 32];
    hkdf.expand(b"frame-key", &mut key)
        .map_err(|_| "derive frame key failed".to_string())?;
    Ok(key)
}

fn encrypt_frame(secret_b64: &str, plaintext: &[u8]) -> Result<Vec<u8>, String> {
    if plaintext.len() > MAX_FRAME_PLAINTEXT_BYTES {
        return Err("frame plaintext exceeds allowed size".to_string());
    }
    let key = derive_frame_key(secret_b64)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    let mut nonce_bytes = [0u8; FRAME_NONCE_BYTES];
    OsRng.fill_bytes(&mut nonce_bytes);
    let nonce = XNonce::from_slice(&nonce_bytes);
    let ciphertext = cipher
        .encrypt(nonce, plaintext)
        .map_err(|_| "frame encryption failed".to_string())?;

    let mut out = Vec::with_capacity(FRAME_NONCE_BYTES + ciphertext.len());
    out.extend_from_slice(&nonce_bytes);
    out.extend_from_slice(&ciphertext);
    if out.len() > MAX_FRAME_BYTES {
        return Err("encrypted frame exceeds allowed size".to_string());
    }
    Ok(out)
}

fn decrypt_frame(secret_b64: &str, encrypted: &[u8]) -> Result<Vec<u8>, String> {
    if encrypted.len() < FRAME_NONCE_BYTES {
        return Err("encrypted frame is too short".to_string());
    }
    if encrypted.len() > MAX_FRAME_BYTES {
        return Err("encrypted frame exceeds allowed size".to_string());
    }
    let key = derive_frame_key(secret_b64)?;
    let cipher = XChaCha20Poly1305::new(Key::from_slice(&key));
    let (nonce_bytes, ciphertext) = encrypted.split_at(FRAME_NONCE_BYTES);
    let nonce = XNonce::from_slice(nonce_bytes);
    let plaintext = cipher
        .decrypt(nonce, ciphertext)
        .map_err(|_| "frame decrypt/auth failed".to_string())?;
    if plaintext.len() > MAX_FRAME_PLAINTEXT_BYTES {
        return Err("frame plaintext exceeds allowed size".to_string());
    }
    Ok(plaintext)
}

fn verify_frame(frame: &SignedFrame) -> Result<(), String> {
    let now = now_millis();
    if (now - frame.timestamp_ms).abs() > MAX_CLOCK_SKEW_MS {
        return Err("frame timestamp outside allowed skew window".to_string());
    }
    Ok(())
}

fn read_prefixed_payload<R: Read>(reader: &mut R) -> Result<Vec<u8>, String> {
    let mut len_buf = [0u8; 4];
    reader
        .read_exact(&mut len_buf)
        .map_err(|e| format!("read frame length failed: {e}"))?;
    let frame_len = u32::from_be_bytes(len_buf) as usize;
    if frame_len == 0 {
        return Err("empty frame".to_string());
    }
    if frame_len > MAX_FRAME_BYTES {
        return Err("frame exceeds maximum allowed size".to_string());
    }
    let mut payload = vec![0u8; frame_len];
    reader
        .read_exact(&mut payload)
        .map_err(|e| format!("read frame payload failed: {e}"))?;
    Ok(payload)
}

fn read_frame(stream: &mut TcpStream, secret_b64: &str) -> Result<SignedFrame, String> {
    let payload = read_prefixed_payload(stream)?;
    let plaintext = decrypt_frame(secret_b64, &payload)?;
    serde_json::from_slice::<SignedFrame>(&plaintext)
        .map_err(|e| format!("parse frame failed: {e}"))
}

fn write_frame(
    stream: &mut TcpStream,
    secret_b64: &str,
    frame: &SignedFrame,
) -> Result<(), String> {
    let raw = serde_json::to_vec(frame).map_err(|e| format!("serialize frame failed: {e}"))?;
    let encrypted = encrypt_frame(secret_b64, &raw)?;
    let len = u32::try_from(encrypted.len())
        .map_err(|_| "encrypted frame length exceeds protocol limit".to_string())?;
    stream
        .write_all(&len.to_be_bytes())
        .map_err(|e| format!("write frame length failed: {e}"))?;
    stream
        .write_all(&encrypted)
        .map_err(|e| format!("write frame failed: {e}"))?;
    Ok(())
}

fn local_ip_guess() -> IpAddr {
    let try_socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0));
    if let Ok(socket) = try_socket {
        if socket.connect((Ipv4Addr::new(8, 8, 8, 8), 80)).is_ok() {
            if let Ok(addr) = socket.local_addr() {
                return addr.ip();
            }
        }
    }
    IpAddr::V4(Ipv4Addr::LOCALHOST)
}

fn is_friend_link_dev_mode_enabled() -> bool {
    let raw = std::env::var("MPM_DEV_MODE").unwrap_or_default();
    matches!(
        raw.trim().to_ascii_lowercase().as_str(),
        "1" | "true" | "yes" | "on"
    )
}

pub fn endpoint_for_port(port: u16) -> String {
    format!("{}:{}", local_ip_guess(), port)
}

fn normalize_peer_endpoint(advertised_endpoint: &str, stream_peer: Option<SocketAddr>) -> String {
    let Some(stream_peer_addr) = stream_peer else {
        return advertised_endpoint.to_string();
    };
    let Ok(advertised_addr) = advertised_endpoint.parse::<SocketAddr>() else {
        return advertised_endpoint.to_string();
    };
    let advertised_ip = advertised_addr.ip();
    let observed_ip = stream_peer_addr.ip();
    if advertised_ip.is_loopback()
        || advertised_ip.is_unspecified()
        || (advertised_ip != observed_ip && advertised_ip.is_ipv4() == observed_ip.is_ipv4())
    {
        return format!("{}:{}", observed_ip, advertised_addr.port());
    }
    advertised_endpoint.to_string()
}

fn is_private_or_local_ip(ip: &IpAddr) -> bool {
    match ip {
        IpAddr::V4(v4) => v4.is_private() || v4.is_link_local() || v4.is_documentation(),
        IpAddr::V6(v6) => v6.is_unique_local() || v6.is_unicast_link_local(),
    }
}

fn validate_endpoint_for_session(
    session: &FriendLinkSessionRecord,
    endpoint: &str,
) -> Result<String, String> {
    let trimmed = endpoint.trim();
    let parsed: SocketAddr = trimmed
        .parse()
        .map_err(|_| "peer endpoint must be an explicit IP:port".to_string())?;
    if parsed.ip().is_unspecified() || parsed.ip().is_multicast() {
        return Err("peer endpoint has an invalid address".to_string());
    }
    if parsed.ip().is_loopback() && !session.allow_loopback_endpoints {
        return Err("loopback peer endpoints are blocked by session policy".to_string());
    }
    if !parsed.ip().is_loopback()
        && !is_private_or_local_ip(&parsed.ip())
        && !session.allow_internet_endpoints
    {
        return Err("public internet peer endpoints are blocked by session policy".to_string());
    }
    Ok(parsed.to_string())
}

pub fn stop_listener(instance_id: &str) {
    if let Ok(mut map) = listener_map().lock() {
        if let Some(handle) = map.remove(instance_id) {
            let _ = handle.stop_tx.send(());
        }
    }
}

pub fn ensure_listener(
    app_data_dir: PathBuf,
    session: &mut FriendLinkSessionRecord,
) -> Result<String, String> {
    let key = session.instance_id.clone();
    let binding_fingerprint = listener_binding_fingerprint(session);
    let mut restarted = false;
    if let Ok(mut map) = listener_map().lock() {
        if let Some(existing) = map.get(&key) {
            if existing.binding_fingerprint == binding_fingerprint {
                let endpoint = endpoint_for_port(existing.port);
                session.listener_port = existing.port;
                session.listener_endpoint = Some(endpoint.clone());
                return Ok(endpoint);
            }
        }
        if let Some(existing) = map.remove(&key) {
            let _ = existing.stop_tx.send(());
            session.listener_port = 0;
            restarted = true;
        }
    }
    if restarted {
        thread::sleep(Duration::from_millis(120));
    }

    let listener = TcpListener::bind((Ipv4Addr::UNSPECIFIED, session.listener_port))
        .map_err(|e| format!("bind friend-link listener failed: {e}"))?;
    let port = listener
        .local_addr()
        .map_err(|e| format!("read listener addr failed: {e}"))?
        .port();
    listener
        .set_nonblocking(true)
        .map_err(|e| format!("set listener nonblocking failed: {e}"))?;

    let (stop_tx, stop_rx) = mpsc::channel::<()>();
    let instance_id = session.instance_id.clone();
    let group_id = session.group_id.clone();
    let local_peer_id = session.local_peer_id.clone();
    let shared_secret_b64 = session.shared_secret_b64.clone();

    thread::spawn(move || {
        let mut seen_nonces = HashSet::<String>::new();
        loop {
            if stop_rx.try_recv().is_ok() {
                break;
            }
            match listener.accept() {
                Ok((mut stream, _addr)) => {
                    let _ = stream.set_read_timeout(Some(Duration::from_secs(5)));
                    let _ = stream.set_write_timeout(Some(Duration::from_secs(5)));
                    let response = handle_incoming_frame(
                        &app_data_dir,
                        &instance_id,
                        &group_id,
                        &local_peer_id,
                        &shared_secret_b64,
                        &mut stream,
                        &mut seen_nonces,
                    );
                    if let Err(err) = response {
                        let payload = serde_json::json!({ "ok": false, "error": err });
                        let frame = SignedFrame {
                            group_id: group_id.clone(),
                            from_peer_id: local_peer_id.clone(),
                            timestamp_ms: now_millis(),
                            nonce: Uuid::new_v4().to_string(),
                            payload_type: "error".to_string(),
                            payload,
                        };
                        if verify_frame(&frame).is_ok() {
                            let _ = write_frame(&mut stream, &shared_secret_b64, &frame);
                            let _ = stream.shutdown(Shutdown::Both);
                        }
                    }
                }
                Err(err) if err.kind() == std::io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(90));
                }
                Err(_) => {
                    thread::sleep(Duration::from_millis(200));
                }
            }
        }
    });

    if let Ok(mut map) = listener_map().lock() {
        map.insert(
            key,
            ListenerHandle {
                port,
                binding_fingerprint,
                stop_tx,
            },
        );
    }

    let endpoint = endpoint_for_port(port);
    session.listener_port = port;
    session.listener_endpoint = Some(endpoint.clone());
    Ok(endpoint)
}

fn handle_incoming_frame(
    app_data_dir: &PathBuf,
    instance_id: &str,
    group_id: &str,
    local_peer_id: &str,
    shared_secret_b64: &str,
    stream: &mut TcpStream,
    seen_nonces: &mut HashSet<String>,
) -> Result<(), String> {
    let incoming = read_frame(stream, shared_secret_b64)?;
    if incoming.group_id != group_id {
        return Err("group mismatch".to_string());
    }
    verify_frame(&incoming)?;
    if !seen_nonces.insert(incoming.nonce.clone()) {
        return Err("replayed nonce".to_string());
    }
    if seen_nonces.len() > MAX_SEEN_NONCES {
        let first = seen_nonces.iter().next().cloned();
        if let Some(old) = first {
            seen_nonces.remove(&old);
        }
    }

    let store_path = store_path_from_app_data(app_data_dir);
    let mut store = read_store_at_path(&store_path)?;

    let mut payload_type = "error".to_string();
    let mut payload = serde_json::json!({ "ok": false, "error": "unsupported payload" });

    if incoming.payload_type == "hello" {
        let mut hello: HelloPayload = serde_json::from_value(incoming.payload)
            .map_err(|e| format!("parse hello payload failed: {e}"))?;
        hello.endpoint = normalize_peer_endpoint(&hello.endpoint, stream.peer_addr().ok());

        let (peer_summaries, local_display_name, local_endpoint) = {
            let session = get_session_mut(&mut store, instance_id)
                .ok_or_else(|| "friend-link session not found".to_string())?;
            hello.endpoint = validate_endpoint_for_session(session, &hello.endpoint)?;

            let mut existing = session
                .peers
                .iter()
                .position(|p| p.peer_id == hello.peer_id);
            if existing.is_none() {
                if session.peers.len() >= PEER_LIMIT.saturating_sub(1) {
                    return Err("group is full (max 8 peers)".to_string());
                }
                session.peers.push(FriendPeerRecord {
                    peer_id: hello.peer_id.clone(),
                    display_name: hello.display_name.clone(),
                    endpoint: hello.endpoint.clone(),
                    added_at: now_iso(),
                    last_seen_at: Some(now_iso()),
                    online: true,
                    last_state_hash: None,
                });
                existing = Some(session.peers.len() - 1);
            }
            if let Some(idx) = existing {
                let peer = &mut session.peers[idx];
                peer.display_name = hello.display_name.clone();
                peer.endpoint = hello.endpoint.clone();
                peer.last_seen_at = Some(now_iso());
                peer.online = true;
            }

            let peer_summaries = session
                .peers
                .iter()
                .map(|p| PeerSummary {
                    peer_id: p.peer_id.clone(),
                    display_name: p.display_name.clone(),
                    endpoint: p.endpoint.clone(),
                    online: p.online,
                })
                .collect::<Vec<_>>();

            let local_display_name = session.display_name.clone();
            let local_endpoint = session
                .listener_endpoint
                .clone()
                .unwrap_or_else(|| endpoint_for_port(session.listener_port));
            let local_endpoint = validate_endpoint_for_session(session, &local_endpoint)?;
            (peer_summaries, local_display_name, local_endpoint)
        };

        write_store_at_path(&store_path, &store)?;

        payload_type = "hello_ack".to_string();
        payload = serde_json::to_value(HelloAckPayload {
            peer_id: local_peer_id.to_string(),
            display_name: local_display_name,
            endpoint: local_endpoint,
            peers: peer_summaries,
        })
        .map_err(|e| format!("serialize hello ack failed: {e}"))?;
    } else if incoming.payload_type == "state_request" {
        let _request: StateRequestPayload =
            serde_json::from_value(incoming.payload).unwrap_or(StateRequestPayload {});
        let session = get_session_mut(&mut store, instance_id)
            .ok_or_else(|| "friend-link session not found".to_string())?;
        let instances_dir = state::safe_join_under(app_data_dir, "instances")?;
        let state =
            state::collect_sync_state(&instances_dir, &session.instance_id, &session.allowlist)?;
        let endpoint = session
            .listener_endpoint
            .clone()
            .unwrap_or_else(|| endpoint_for_port(session.listener_port));
        let endpoint = validate_endpoint_for_session(session, &endpoint)?;
        payload_type = "state_response".to_string();
        payload = serde_json::to_value(StateResponsePayload {
            peer_id: local_peer_id.to_string(),
            display_name: session.display_name.clone(),
            endpoint,
            state,
        })
        .map_err(|e| format!("serialize state response failed: {e}"))?;
    } else if incoming.payload_type == "file_request" {
        let request: FileRequestPayload = serde_json::from_value(incoming.payload)
            .map_err(|e| format!("parse file request payload failed: {e}"))?;
        let session = get_session_mut(&mut store, instance_id)
            .ok_or_else(|| "friend-link session not found".to_string())?;
        let instances_dir = state::safe_join_under(app_data_dir, "instances")?;
        let entries = state::read_lock_entries(&instances_dir, &session.instance_id)?;
        let map = state::lock_entry_map(&entries);
        let response = if let Some(entry) = map.get(&request.key) {
            match state::read_lock_entry_bytes(&instances_dir, &session.instance_id, entry)? {
                Some(bytes) => {
                    let mut hasher = sha2::Sha256::new();
                    use sha2::Digest as _;
                    hasher.update(&bytes);
                    let digest = format!("{:x}", hasher.finalize());
                    FileResponsePayload {
                        key: request.key,
                        found: true,
                        sha256: Some(digest),
                        bytes_b64: Some(BASE64_STANDARD.encode(bytes)),
                        message: None,
                    }
                }
                None => FileResponsePayload {
                    key: request.key,
                    found: false,
                    sha256: None,
                    bytes_b64: None,
                    message: Some("entry exists but content file is missing".to_string()),
                },
            }
        } else {
            FileResponsePayload {
                key: request.key,
                found: false,
                sha256: None,
                bytes_b64: None,
                message: Some("entry not found".to_string()),
            }
        };
        payload_type = "file_response".to_string();
        payload = serde_json::to_value(response)
            .map_err(|e| format!("serialize file response failed: {e}"))?;
    }

    let response = SignedFrame {
        group_id: group_id.to_string(),
        from_peer_id: local_peer_id.to_string(),
        timestamp_ms: now_millis(),
        nonce: Uuid::new_v4().to_string(),
        payload_type,
        payload,
    };
    verify_frame(&response)?;
    write_frame(stream, shared_secret_b64, &response)?;
    let _ = stream.shutdown(Shutdown::Both);
    Ok(())
}

fn send_frame(
    endpoint: &str,
    secret_b64: &str,
    frame: &SignedFrame,
) -> Result<SignedFrame, String> {
    let connect_err_to_string = |e: std::io::Error| {
        format!(
            "connect peer failed: {e}. Verify the host is running, reachable, and sharing a fresh invite."
        )
    };

    let mut stream = match TcpStream::connect(endpoint) {
        Ok(stream) => stream,
        Err(primary_err) => {
            let fallback_loopback = endpoint
                .parse::<SocketAddr>()
                .ok()
                .filter(|addr| addr.is_ipv4())
                .filter(|addr| !addr.ip().is_loopback())
                .filter(|_| is_friend_link_dev_mode_enabled())
                .filter(|addr| addr.ip() == local_ip_guess())
                .map(|addr| SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), addr.port()));
            if let Some(loopback_addr) = fallback_loopback {
                match TcpStream::connect(loopback_addr) {
                    Ok(stream) => stream,
                    Err(_) => return Err(connect_err_to_string(primary_err)),
                }
            } else {
                return Err(connect_err_to_string(primary_err));
            }
        }
    };
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("set read timeout failed: {e}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("set write timeout failed: {e}"))?;

    write_frame(&mut stream, secret_b64, frame).map_err(map_secure_frame_error)?;
    let _ = stream.shutdown(Shutdown::Write);
    read_frame(&mut stream, secret_b64).map_err(map_secure_frame_error)
}

pub fn send_hello(
    session: &FriendLinkSessionRecord,
    endpoint: &str,
    payload: HelloPayload,
) -> Result<HelloAckPayload, String> {
    let endpoint = validate_endpoint_for_session(session, endpoint)?;
    let request = SignedFrame {
        group_id: session.group_id.clone(),
        from_peer_id: session.local_peer_id.clone(),
        timestamp_ms: now_millis(),
        nonce: Uuid::new_v4().to_string(),
        payload_type: "hello".to_string(),
        payload: serde_json::to_value(payload)
            .map_err(|e| format!("serialize hello payload failed: {e}"))?,
    };
    verify_frame(&request)?;

    let response = send_frame(&endpoint, &session.shared_secret_b64, &request)?;
    verify_frame(&response)?;
    if response.payload_type == "error" {
        let err = response
            .payload
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("hello failed")
            .to_string();
        return Err(err);
    }
    if response.payload_type != "hello_ack" {
        return Err("peer returned unexpected payload type for hello".to_string());
    }
    serde_json::from_value::<HelloAckPayload>(response.payload)
        .map_err(|e| format!("parse hello ack failed: {e}"))
}

pub fn request_state(
    session: &FriendLinkSessionRecord,
    endpoint: &str,
) -> Result<StateResponsePayload, String> {
    let endpoint = validate_endpoint_for_session(session, endpoint)?;
    let request = SignedFrame {
        group_id: session.group_id.clone(),
        from_peer_id: session.local_peer_id.clone(),
        timestamp_ms: now_millis(),
        nonce: Uuid::new_v4().to_string(),
        payload_type: "state_request".to_string(),
        payload: serde_json::to_value(StateRequestPayload {})
            .map_err(|e| format!("serialize state request payload failed: {e}"))?,
    };
    verify_frame(&request)?;

    let response = send_frame(&endpoint, &session.shared_secret_b64, &request)?;
    verify_frame(&response)?;
    if response.payload_type == "error" {
        let err = response
            .payload
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("state request failed")
            .to_string();
        return Err(err);
    }
    if response.payload_type != "state_response" {
        return Err("peer returned unexpected payload type for state request".to_string());
    }
    serde_json::from_value::<StateResponsePayload>(response.payload)
        .map_err(|e| format!("parse state response failed: {e}"))
}

pub fn request_lock_entry_file(
    session: &FriendLinkSessionRecord,
    endpoint: &str,
    key: &str,
) -> Result<FileResponsePayload, String> {
    let endpoint = validate_endpoint_for_session(session, endpoint)?;
    let request = SignedFrame {
        group_id: session.group_id.clone(),
        from_peer_id: session.local_peer_id.clone(),
        timestamp_ms: now_millis(),
        nonce: Uuid::new_v4().to_string(),
        payload_type: "file_request".to_string(),
        payload: serde_json::to_value(FileRequestPayload {
            key: key.to_string(),
        })
        .map_err(|e| format!("serialize file request payload failed: {e}"))?,
    };
    verify_frame(&request)?;

    let response = send_frame(&endpoint, &session.shared_secret_b64, &request)?;
    verify_frame(&response)?;
    if response.payload_type == "error" {
        let err = response
            .payload
            .get("error")
            .and_then(|v| v.as_str())
            .unwrap_or("file request failed")
            .to_string();
        return Err(err);
    }
    if response.payload_type != "file_response" {
        return Err("peer returned unexpected payload type for file request".to_string());
    }
    serde_json::from_value::<FileResponsePayload>(response.payload)
        .map_err(|e| format!("parse file response failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::io::Cursor;

    fn test_secret_b64() -> String {
        BASE64_STANDARD.encode([7u8; 32])
    }

    fn test_session(instance_id: &str, secret_b64: &str) -> FriendLinkSessionRecord {
        FriendLinkSessionRecord {
            instance_id: instance_id.to_string(),
            group_id: format!("group_{instance_id}"),
            local_peer_id: format!("peer_{instance_id}"),
            display_name: "Test".to_string(),
            shared_secret_key_id: String::new(),
            shared_secret_b64: secret_b64.to_string(),
            protocol_version: 1,
            listener_port: 0,
            listener_endpoint: None,
            peers: vec![],
            allowlist: state::default_allowlist(),
            last_peer_sync_at: HashMap::new(),
            last_good_snapshot: None,
            pending_conflicts: vec![],
            cached_peer_state: HashMap::new(),
            bootstrap_host_peer_id: None,
            trusted_peer_ids: vec![],
            trusted_peer_ids_initialized: false,
            guardrails_updated_at_ms: 0,
            peer_aliases: HashMap::new(),
            allow_loopback_endpoints: false,
            allow_internet_endpoints: false,
            max_auto_changes: 25,
            sync_mods: true,
            sync_resourcepacks: false,
            sync_shaderpacks: true,
            sync_datapacks: true,
        }
    }

    fn test_frame() -> SignedFrame {
        SignedFrame {
            group_id: "group".to_string(),
            from_peer_id: "peer".to_string(),
            timestamp_ms: now_millis(),
            nonce: "nonce".to_string(),
            payload_type: "state_request".to_string(),
            payload: serde_json::json!({}),
        }
    }

    #[test]
    fn encrypted_frame_tamper_fails_authentication() {
        let secret = test_secret_b64();
        let raw = serde_json::to_vec(&test_frame()).expect("serialize frame");
        let mut encrypted = encrypt_frame(&secret, &raw).expect("encrypt frame");
        encrypted[FRAME_NONCE_BYTES + 1] ^= 0x01;
        let err = decrypt_frame(&secret, &encrypted).expect_err("decrypt should fail");
        assert!(err.contains("decrypt/auth"));
    }

    #[test]
    fn oversized_frame_length_is_rejected() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&((MAX_FRAME_BYTES as u32) + 1).to_be_bytes());
        let mut cursor = Cursor::new(bytes);
        let err = read_prefixed_payload(&mut cursor).expect_err("oversized frame must fail");
        assert!(err.contains("maximum allowed size"));
    }

    #[test]
    fn encrypted_frame_roundtrip() {
        let secret = test_secret_b64();
        let frame = test_frame();
        let raw = serde_json::to_vec(&frame).expect("serialize frame");
        let encrypted = encrypt_frame(&secret, &raw).expect("encrypt");
        let decrypted = decrypt_frame(&secret, &encrypted).expect("decrypt");
        let parsed: SignedFrame = serde_json::from_slice(&decrypted).expect("parse frame");
        assert_eq!(parsed.group_id, "group");
        assert_eq!(parsed.from_peer_id, "peer");
    }

    #[test]
    fn cross_network_relay_style_harness_exchanges_minimal_encrypted_message() {
        let secret = test_secret_b64();

        let request = SignedFrame {
            group_id: "group".to_string(),
            from_peer_id: "peer_a".to_string(),
            timestamp_ms: now_millis(),
            nonce: "req".to_string(),
            payload_type: "state_request".to_string(),
            payload: serde_json::json!({}),
        };
        let request_plain = serde_json::to_vec(&request).expect("serialize request");
        let request_wire = encrypt_frame(&secret, &request_plain).expect("encrypt request");

        // Relay only sees opaque ciphertext bytes and cannot read plaintext fields.
        assert!(String::from_utf8(request_wire.clone()).is_err());

        let decoded_request =
            decrypt_frame(&secret, &request_wire).expect("receiver decrypts request from relay");
        let parsed_request: SignedFrame =
            serde_json::from_slice(&decoded_request).expect("parse request");
        assert_eq!(parsed_request.payload_type, "state_request");

        let response = SignedFrame {
            group_id: "group".to_string(),
            from_peer_id: "peer_b".to_string(),
            timestamp_ms: now_millis(),
            nonce: "resp".to_string(),
            payload_type: "state_response".to_string(),
            payload: serde_json::json!({ "ok": true }),
        };
        let response_plain = serde_json::to_vec(&response).expect("serialize response");
        let response_wire = encrypt_frame(&secret, &response_plain).expect("encrypt response");
        let decoded_response =
            decrypt_frame(&secret, &response_wire).expect("sender decrypts response from relay");
        let parsed_response: SignedFrame =
            serde_json::from_slice(&decoded_response).expect("parse response");
        assert_eq!(parsed_response.payload_type, "state_response");
    }

    #[test]
    fn ensure_listener_restarts_when_binding_changes() {
        let instance_id = format!("inst-listener-{}", Uuid::new_v4());
        let app_data_dir =
            std::env::temp_dir().join(format!("openjar-friend-net-{}", Uuid::new_v4()));
        let mut session = test_session(&instance_id, &test_secret_b64());

        let _ =
            ensure_listener(app_data_dir.clone(), &mut session).expect("first listener startup");
        let first_fingerprint = {
            let guard = listener_map().lock().expect("listener map lock");
            guard
                .get(&instance_id)
                .expect("listener handle")
                .binding_fingerprint
                .clone()
        };

        session.shared_secret_b64 = BASE64_STANDARD.encode([9u8; 32]);
        let _ = ensure_listener(app_data_dir, &mut session)
            .expect("listener restart after secret rotation");
        let second_fingerprint = {
            let guard = listener_map().lock().expect("listener map lock");
            guard
                .get(&instance_id)
                .expect("listener handle after restart")
                .binding_fingerprint
                .clone()
        };

        assert_ne!(first_fingerprint, second_fingerprint);
        stop_listener(&instance_id);
    }
}
