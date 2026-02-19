use crate::friend_link::state;
use crate::friend_link::store::{
    get_session_mut, read_store_at_path, store_path_from_app_data, write_store_at_path, FriendLinkSessionRecord,
    FriendPeerRecord,
};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use hmac::{Hmac, Mac};
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

type HmacSha256 = Hmac<Sha256>;

const MAX_CLOCK_SKEW_MS: i64 = 120_000;
const MAX_SEEN_NONCES: usize = 4096;
const PEER_LIMIT: usize = 8;

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
    signature: String,
}

#[derive(Debug, Clone, Serialize)]
struct SignableFrame<'a> {
    group_id: &'a str,
    from_peer_id: &'a str,
    timestamp_ms: i64,
    nonce: &'a str,
    payload_type: &'a str,
    payload: &'a serde_json::Value,
}

struct ListenerHandle {
    port: u16,
    stop_tx: mpsc::Sender<()>,
}

fn listener_map() -> &'static Mutex<HashMap<String, ListenerHandle>> {
    static LISTENERS: OnceLock<Mutex<HashMap<String, ListenerHandle>>> = OnceLock::new();
    LISTENERS.get_or_init(|| Mutex::new(HashMap::new()))
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

fn make_signature(secret_b64: &str, frame: &SignedFrame) -> Result<String, String> {
    let secret = BASE64_STANDARD
        .decode(secret_b64)
        .map_err(|e| format!("decode shared secret failed: {e}"))?;
    let signable = SignableFrame {
        group_id: &frame.group_id,
        from_peer_id: &frame.from_peer_id,
        timestamp_ms: frame.timestamp_ms,
        nonce: &frame.nonce,
        payload_type: &frame.payload_type,
        payload: &frame.payload,
    };
    let raw = serde_json::to_vec(&signable).map_err(|e| format!("serialize signable frame failed: {e}"))?;
    let mut mac = HmacSha256::new_from_slice(&secret).map_err(|e| format!("hmac init failed: {e}"))?;
    mac.update(&raw);
    let bytes = mac.finalize().into_bytes();
    Ok(BASE64_STANDARD.encode(bytes))
}

fn sign_frame(secret_b64: &str, frame: &mut SignedFrame) -> Result<(), String> {
    frame.signature = make_signature(secret_b64, frame)?;
    Ok(())
}

fn verify_frame(secret_b64: &str, frame: &SignedFrame) -> Result<(), String> {
    let now = now_millis();
    if (now - frame.timestamp_ms).abs() > MAX_CLOCK_SKEW_MS {
        return Err("frame timestamp outside allowed skew window".to_string());
    }
    let expected = make_signature(secret_b64, frame)?;
    if expected != frame.signature {
        return Err("invalid frame signature".to_string());
    }
    Ok(())
}

fn read_frame(stream: &mut TcpStream) -> Result<SignedFrame, String> {
    let mut raw = Vec::new();
    stream
        .read_to_end(&mut raw)
        .map_err(|e| format!("read frame failed: {e}"))?;
    if raw.is_empty() {
        return Err("empty frame".to_string());
    }
    serde_json::from_slice::<SignedFrame>(&raw).map_err(|e| format!("parse frame failed: {e}"))
}

fn write_frame(stream: &mut TcpStream, frame: &SignedFrame) -> Result<(), String> {
    let raw = serde_json::to_vec(frame).map_err(|e| format!("serialize frame failed: {e}"))?;
    stream
        .write_all(&raw)
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
    if let Ok(map) = listener_map().lock() {
        if let Some(existing) = map.get(&key) {
            let endpoint = endpoint_for_port(existing.port);
            session.listener_port = existing.port;
            session.listener_endpoint = Some(endpoint.clone());
            return Ok(endpoint);
        }
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
                        let mut frame = SignedFrame {
                            group_id: group_id.clone(),
                            from_peer_id: local_peer_id.clone(),
                            timestamp_ms: now_millis(),
                            nonce: Uuid::new_v4().to_string(),
                            payload_type: "error".to_string(),
                            payload,
                            signature: String::new(),
                        };
                        if sign_frame(&shared_secret_b64, &mut frame).is_ok() {
                            let _ = write_frame(&mut stream, &frame);
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
    let incoming = read_frame(stream)?;
    if incoming.group_id != group_id {
        return Err("group mismatch".to_string());
    }
    verify_frame(shared_secret_b64, &incoming)?;
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
        let _request: StateRequestPayload = serde_json::from_value(incoming.payload)
            .unwrap_or(StateRequestPayload {});
        let session = get_session_mut(&mut store, instance_id)
            .ok_or_else(|| "friend-link session not found".to_string())?;
        let instances_dir = app_data_dir.join("instances");
        let state = state::collect_sync_state(&instances_dir, &session.instance_id, &session.allowlist)?;
        payload_type = "state_response".to_string();
        payload = serde_json::to_value(StateResponsePayload {
            peer_id: local_peer_id.to_string(),
            display_name: session.display_name.clone(),
            endpoint: session
                .listener_endpoint
                .clone()
                .unwrap_or_else(|| endpoint_for_port(session.listener_port)),
            state,
        })
        .map_err(|e| format!("serialize state response failed: {e}"))?;
    } else if incoming.payload_type == "file_request" {
        let request: FileRequestPayload = serde_json::from_value(incoming.payload)
            .map_err(|e| format!("parse file request payload failed: {e}"))?;
        let session = get_session_mut(&mut store, instance_id)
            .ok_or_else(|| "friend-link session not found".to_string())?;
        let instances_dir = app_data_dir.join("instances");
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

    let mut response = SignedFrame {
        group_id: group_id.to_string(),
        from_peer_id: local_peer_id.to_string(),
        timestamp_ms: now_millis(),
        nonce: Uuid::new_v4().to_string(),
        payload_type,
        payload,
        signature: String::new(),
    };
    sign_frame(shared_secret_b64, &mut response)?;
    write_frame(stream, &response)?;
    let _ = stream.shutdown(Shutdown::Both);
    Ok(())
}

fn send_frame(endpoint: &str, frame: &SignedFrame) -> Result<SignedFrame, String> {
    let mut stream = TcpStream::connect(endpoint).map_err(|e| format!("connect peer failed: {e}"))?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("set read timeout failed: {e}"))?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|e| format!("set write timeout failed: {e}"))?;

    write_frame(&mut stream, frame)?;
    let _ = stream.shutdown(Shutdown::Write);
    read_frame(&mut stream)
}

pub fn send_hello(
    session: &FriendLinkSessionRecord,
    endpoint: &str,
    payload: HelloPayload,
) -> Result<HelloAckPayload, String> {
    let mut request = SignedFrame {
        group_id: session.group_id.clone(),
        from_peer_id: session.local_peer_id.clone(),
        timestamp_ms: now_millis(),
        nonce: Uuid::new_v4().to_string(),
        payload_type: "hello".to_string(),
        payload: serde_json::to_value(payload).map_err(|e| format!("serialize hello payload failed: {e}"))?,
        signature: String::new(),
    };
    sign_frame(&session.shared_secret_b64, &mut request)?;

    let response = send_frame(endpoint, &request)?;
    verify_frame(&session.shared_secret_b64, &response)?;
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
    let mut request = SignedFrame {
        group_id: session.group_id.clone(),
        from_peer_id: session.local_peer_id.clone(),
        timestamp_ms: now_millis(),
        nonce: Uuid::new_v4().to_string(),
        payload_type: "state_request".to_string(),
        payload: serde_json::to_value(StateRequestPayload {})
            .map_err(|e| format!("serialize state request payload failed: {e}"))?,
        signature: String::new(),
    };
    sign_frame(&session.shared_secret_b64, &mut request)?;

    let response = send_frame(endpoint, &request)?;
    verify_frame(&session.shared_secret_b64, &response)?;
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
    let mut request = SignedFrame {
        group_id: session.group_id.clone(),
        from_peer_id: session.local_peer_id.clone(),
        timestamp_ms: now_millis(),
        nonce: Uuid::new_v4().to_string(),
        payload_type: "file_request".to_string(),
        payload: serde_json::to_value(FileRequestPayload {
            key: key.to_string(),
        })
        .map_err(|e| format!("serialize file request payload failed: {e}"))?,
        signature: String::new(),
    };
    sign_frame(&session.shared_secret_b64, &mut request)?;

    let response = send_frame(endpoint, &request)?;
    verify_frame(&session.shared_secret_b64, &response)?;
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
