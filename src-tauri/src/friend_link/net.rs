use crate::friend_link::state;
use crate::friend_link::store::{
    get_session_mut, get_session_signing_private_key, read_store_at_path, store_path_from_app_data,
    write_store_at_path, FriendInviteUsageRecord, FriendLinkSessionRecord, FriendPeerRecord,
};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use chacha20poly1305::aead::{Aead, KeyInit};
use chacha20poly1305::{Key, XChaCha20Poly1305, XNonce};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use hkdf::Hkdf;
use igd::{search_gateway, PortMappingProtocol};
use rand::rngs::OsRng;
use rand::RngCore;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use std::collections::{HashMap, VecDeque};
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
const CONNECTION_RATE_WINDOW_MS: i64 = 10_000;
const MAX_CONNECTIONS_PER_IP_PER_WINDOW: usize = 30;
const FRAME_NONCE_BYTES: usize = 24;
const MAX_FRAME_BYTES: usize = 5 * 1024 * 1024;
const MAX_FRAME_PLAINTEXT_BYTES: usize = 1024 * 1024;
const SIGNING_FINGERPRINT_PREFIX: &str = "ed25519:";
const MAX_INVITE_USAGE_TIMESTAMPS: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelloPayload {
    pub peer_id: String,
    pub display_name: String,
    pub endpoint: String,
    #[serde(default)]
    pub invite_id: Option<String>,
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
    #[serde(default)]
    key_fingerprint: String,
    #[serde(default)]
    signing_public_key: Option<String>,
    #[serde(default)]
    signature: String,
}

#[derive(Debug, Clone, Serialize)]
struct SignableFrame {
    group_id: String,
    from_peer_id: String,
    timestamp_ms: i64,
    nonce: String,
    payload_type: String,
    payload: serde_json::Value,
    key_fingerprint: String,
}

#[derive(Debug, Clone)]
struct VerifiedFrameIdentity {
    public_key_b64: String,
}

#[derive(Debug, Clone)]
struct UpnpPortMapping {
    external_port: u16,
}

struct ListenerHandle {
    port: u16,
    binding_fingerprint: String,
    advertised_endpoints: Vec<String>,
    upnp_mapping: Option<UpnpPortMapping>,
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
    hasher.update([if session.allow_internet_endpoints {
        1
    } else {
        0
    }]);
    hasher.update([if session.allow_upnp_endpoints { 1 } else { 0 }]);
    if let Some(override_endpoint) = session.public_endpoint_override.as_ref() {
        hasher.update([0u8]);
        hasher.update(override_endpoint.as_bytes());
    }
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

fn parse_signing_private_key_b64(raw: &str) -> Result<SigningKey, String> {
    let decoded = BASE64_STANDARD
        .decode(raw.trim())
        .map_err(|e| format!("decode signing private key failed: {e}"))?;
    let bytes: [u8; 32] = decoded
        .as_slice()
        .try_into()
        .map_err(|_| "signing private key must be 32 bytes".to_string())?;
    Ok(SigningKey::from_bytes(&bytes))
}

fn parse_verifying_key_b64(raw: &str) -> Result<VerifyingKey, String> {
    let decoded = BASE64_STANDARD
        .decode(raw.trim())
        .map_err(|e| format!("decode signing public key failed: {e}"))?;
    let bytes: [u8; 32] = decoded
        .as_slice()
        .try_into()
        .map_err(|_| "signing public key must be 32 bytes".to_string())?;
    VerifyingKey::from_bytes(&bytes).map_err(|e| format!("invalid signing public key: {e}"))
}

fn encode_verifying_key_b64(key: &VerifyingKey) -> String {
    BASE64_STANDARD.encode(key.to_bytes())
}

fn signing_key_fingerprint(key: &VerifyingKey) -> String {
    use sha2::Digest as _;
    let mut hasher = Sha256::new();
    hasher.update(key.to_bytes());
    format!("{SIGNING_FINGERPRINT_PREFIX}{:x}", hasher.finalize())
}

fn signable_frame(frame: &SignedFrame) -> SignableFrame {
    SignableFrame {
        group_id: frame.group_id.clone(),
        from_peer_id: frame.from_peer_id.clone(),
        timestamp_ms: frame.timestamp_ms,
        nonce: frame.nonce.clone(),
        payload_type: frame.payload_type.clone(),
        payload: frame.payload.clone(),
        key_fingerprint: frame.key_fingerprint.clone(),
    }
}

fn sign_frame(frame: &mut SignedFrame, signing_key: &SigningKey) -> Result<(), String> {
    let verifying_key = signing_key.verifying_key();
    frame.key_fingerprint = signing_key_fingerprint(&verifying_key);
    frame.signing_public_key = Some(encode_verifying_key_b64(&verifying_key));
    let bytes = serde_json::to_vec(&signable_frame(frame))
        .map_err(|e| format!("serialize signable frame failed: {e}"))?;
    let signature = signing_key.sign(&bytes);
    frame.signature = BASE64_STANDARD.encode(signature.to_bytes());
    Ok(())
}

fn verify_frame_signature_with_public_key(
    frame: &SignedFrame,
    verifying_key: &VerifyingKey,
) -> Result<(), String> {
    if frame.key_fingerprint.trim().is_empty() {
        return Err("missing frame key fingerprint".to_string());
    }
    let expected_fingerprint = signing_key_fingerprint(verifying_key);
    if frame.key_fingerprint != expected_fingerprint {
        return Err("frame key fingerprint mismatch".to_string());
    }
    if frame.signature.trim().is_empty() {
        return Err("missing frame signature".to_string());
    }
    let signature_raw = BASE64_STANDARD
        .decode(frame.signature.trim())
        .map_err(|e| format!("decode frame signature failed: {e}"))?;
    let signature = Signature::from_slice(&signature_raw)
        .map_err(|e| format!("invalid frame signature bytes: {e}"))?;
    let bytes = serde_json::to_vec(&signable_frame(frame))
        .map_err(|e| format!("serialize signable frame failed: {e}"))?;
    verifying_key
        .verify(&bytes, &signature)
        .map_err(|_| "frame signature verification failed".to_string())
}

fn verify_incoming_frame_identity(
    session: &FriendLinkSessionRecord,
    frame: &SignedFrame,
) -> Result<VerifiedFrameIdentity, String> {
    let Some(pinned_key_b64) = session.peer_signing_public_keys.get(&frame.from_peer_id) else {
        if frame.payload_type != "hello" {
            return Err(
                "untrusted peer identity: peer key must be registered via hello first".to_string(),
            );
        }
        let frame_public_key_b64 = frame
            .signing_public_key
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "hello frame is missing signing public key".to_string())?;
        let verifying_key = parse_verifying_key_b64(&frame_public_key_b64)?;
        verify_frame_signature_with_public_key(frame, &verifying_key)?;
        return Ok(VerifiedFrameIdentity {
            public_key_b64: frame_public_key_b64,
        });
    };

    let verifying_key = parse_verifying_key_b64(pinned_key_b64)?;
    verify_frame_signature_with_public_key(frame, &verifying_key)?;
    if let Some(frame_public_key_b64) = frame.signing_public_key.as_ref() {
        if frame_public_key_b64.trim() != pinned_key_b64.trim() {
            return Err(
                "peer identity mismatch: frame signing key differs from pinned key".to_string(),
            );
        }
    }
    Ok(VerifiedFrameIdentity {
        public_key_b64: pinned_key_b64.clone(),
    })
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

fn build_signed_frame_for_identity(
    group_id: &str,
    local_peer_id: &str,
    local_signing_private_b64: &str,
    payload_type: &str,
    payload: serde_json::Value,
) -> Result<SignedFrame, String> {
    let signing_key = parse_signing_private_key_b64(local_signing_private_b64)?;
    let mut frame = SignedFrame {
        group_id: group_id.to_string(),
        from_peer_id: local_peer_id.to_string(),
        timestamp_ms: now_millis(),
        nonce: Uuid::new_v4().to_string(),
        payload_type: payload_type.to_string(),
        payload,
        key_fingerprint: String::new(),
        signing_public_key: None,
        signature: String::new(),
    };
    sign_frame(&mut frame, &signing_key)?;
    verify_frame(&frame)?;
    Ok(frame)
}

fn build_signed_frame(
    session: &FriendLinkSessionRecord,
    payload_type: &str,
    payload: serde_json::Value,
) -> Result<SignedFrame, String> {
    build_signed_frame_for_identity(
        &session.group_id,
        &session.local_peer_id,
        &session.local_signing_private_b64,
        payload_type,
        payload,
    )
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

fn endpoint_for_session_listener(session: &FriendLinkSessionRecord, port: u16) -> String {
    if session.allow_internet_endpoints {
        endpoint_for_port(port)
    } else {
        format!("{}:{}", Ipv4Addr::LOCALHOST, port)
    }
}

fn try_map_upnp_listener_port(port: u16) -> Option<(String, UpnpPortMapping)> {
    let local_v4 = match local_ip_guess() {
        IpAddr::V4(v4) if !v4.is_loopback() && !v4.is_unspecified() => v4,
        _ => return None,
    };
    let gateway = search_gateway(Default::default()).ok()?;
    let local_addr = std::net::SocketAddrV4::new(local_v4, port);
    if gateway
        .add_port(
            PortMappingProtocol::TCP,
            port,
            local_addr,
            3600,
            "openjar-friendlink",
        )
        .is_err()
    {
        return None;
    }
    let external_ip = gateway.get_external_ip().ok()?;
    Some((
        std::net::SocketAddr::new(IpAddr::V4(external_ip), port).to_string(),
        UpnpPortMapping {
            external_port: port,
        },
    ))
}

fn remove_upnp_mapping(mapping: &UpnpPortMapping) -> Result<(), String> {
    let gateway =
        search_gateway(Default::default()).map_err(|e| format!("gateway lookup failed: {e}"))?;
    gateway
        .remove_port(PortMappingProtocol::TCP, mapping.external_port)
        .map_err(|e| format!("remove port mapping failed: {e}"))?;
    Ok(())
}

pub fn listener_bootstrap_endpoints(instance_id: &str) -> Vec<String> {
    let Ok(map) = listener_map().lock() else {
        return Vec::new();
    };
    map.get(instance_id)
        .map(|entry| entry.advertised_endpoints.clone())
        .unwrap_or_default()
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

fn validate_endpoint_for_session(
    session: &FriendLinkSessionRecord,
    endpoint: &str,
) -> Result<String, String> {
    let trimmed = endpoint.trim();
    if let Ok(parsed) = trimmed.parse::<SocketAddr>() {
        if parsed.ip().is_unspecified() || parsed.ip().is_multicast() {
            return Err("peer endpoint has an invalid address".to_string());
        }
        if parsed.ip().is_loopback()
            && session.allow_internet_endpoints
            && !session.allow_loopback_endpoints
        {
            return Err("loopback peer endpoints are blocked by session policy".to_string());
        }
        if !parsed.ip().is_loopback() && !session.allow_internet_endpoints {
            return Err(
                "LAN/public internet peer endpoints are blocked by session policy. Enable internet mode to connect to other devices."
                    .to_string(),
            );
        }
        return Ok(parsed.to_string());
    }

    let (host, port_raw) = trimmed
        .rsplit_once(':')
        .ok_or_else(|| "peer endpoint must be host:port or IP:port".to_string())?;
    let host = host.trim();
    if host.is_empty() {
        return Err("peer endpoint host is empty".to_string());
    }
    let port: u16 = port_raw
        .trim()
        .parse()
        .map_err(|_| "peer endpoint port is invalid".to_string())?;
    if host.eq_ignore_ascii_case("localhost") {
        if !session.allow_loopback_endpoints {
            return Err("loopback peer endpoints are blocked by session policy".to_string());
        }
        return Ok(format!("localhost:{port}"));
    }
    if !session.allow_internet_endpoints {
        return Err(
            "LAN/public internet peer endpoints are blocked by session policy. Enable internet mode to connect to other devices."
                .to_string(),
        );
    }
    Ok(format!("{host}:{port}"))
}

fn trim_connection_rate_window(attempts: &mut HashMap<IpAddr, VecDeque<i64>>, now_ms: i64) {
    let cutoff = now_ms - CONNECTION_RATE_WINDOW_MS;
    attempts.retain(|_, values| {
        while values.front().is_some_and(|value| *value < cutoff) {
            let _ = values.pop_front();
        }
        !values.is_empty()
    });
}

fn is_connection_rate_limited(
    attempts: &mut HashMap<IpAddr, VecDeque<i64>>,
    ip: IpAddr,
    now_ms: i64,
) -> bool {
    trim_connection_rate_window(attempts, now_ms);
    let values = attempts.entry(ip).or_default();
    if values.len() >= MAX_CONNECTIONS_PER_IP_PER_WINDOW {
        return true;
    }
    values.push_back(now_ms);
    false
}

fn note_nonce_or_replay(
    seen_nonces: &mut HashMap<String, i64>,
    nonce: &str,
    now_ms: i64,
) -> Result<(), String> {
    let cutoff = now_ms - MAX_CLOCK_SKEW_MS;
    seen_nonces.retain(|_, seen_at| *seen_at >= cutoff);
    if seen_nonces.contains_key(nonce) {
        return Err("replayed nonce".to_string());
    }
    if seen_nonces.len() >= MAX_SEEN_NONCES {
        if let Some((oldest_nonce, _)) = seen_nonces
            .iter()
            .min_by_key(|(_, seen_at)| **seen_at)
            .map(|(nonce, seen_at)| (nonce.clone(), *seen_at))
        {
            seen_nonces.remove(&oldest_nonce);
        }
    }
    seen_nonces.insert(nonce.to_string(), now_ms);
    Ok(())
}

fn register_peer_signing_key(
    session: &mut FriendLinkSessionRecord,
    peer_id: &str,
    public_key_b64: &str,
) -> Result<(), String> {
    let peer_id = peer_id.trim();
    if peer_id.is_empty() {
        return Err("peer id is missing".to_string());
    }
    let key = public_key_b64.trim();
    if key.is_empty() {
        return Err("peer signing public key is missing".to_string());
    }
    if let Some(existing) = session.peer_signing_public_keys.get(peer_id) {
        if existing.trim() != key {
            return Err("peer signing key changed unexpectedly; reconnect required".to_string());
        }
        return Ok(());
    }
    session
        .peer_signing_public_keys
        .insert(peer_id.to_string(), key.to_string());
    Ok(())
}

fn enforce_invite_usage_policy(
    session: &mut FriendLinkSessionRecord,
    invite_id: Option<&str>,
) -> Result<(), String> {
    let Some(invite_id) = invite_id.map(str::trim).filter(|value| !value.is_empty()) else {
        return Ok(());
    };
    let Some(policy) = session.invite_policies.get(invite_id).cloned() else {
        return Err("Invite is invalid or no longer active. Request a fresh invite.".to_string());
    };
    let expires = chrono::DateTime::parse_from_rfc3339(&policy.expires_at)
        .map_err(|_| "Invite policy is invalid. Request a fresh invite.".to_string())?
        .with_timezone(&chrono::Utc);
    if chrono::Utc::now() > expires {
        return Err("Invite has expired. Request a fresh invite.".to_string());
    }

    let usage =
        session
            .invite_usage
            .entry(invite_id.to_string())
            .or_insert(FriendInviteUsageRecord {
                used_count: 0,
                used_at: Vec::new(),
            });
    if usage.used_count >= policy.max_uses {
        return Err("Invite has already been used the maximum number of times.".to_string());
    }
    usage.used_count = usage.used_count.saturating_add(1);
    usage.used_at.push(now_iso());
    if usage.used_at.len() > MAX_INVITE_USAGE_TIMESTAMPS {
        let overflow = usage.used_at.len() - MAX_INVITE_USAGE_TIMESTAMPS;
        usage.used_at.drain(0..overflow);
    }
    Ok(())
}

pub fn stop_listener(instance_id: &str) {
    if let Ok(mut map) = listener_map().lock() {
        if let Some(handle) = map.remove(instance_id) {
            let _ = handle.stop_tx.send(());
            if let Some(mapping) = handle.upnp_mapping.as_ref() {
                if let Err(err) = remove_upnp_mapping(mapping) {
                    eprintln!(
                        "friend-link upnp cleanup failed for instance '{}': {}",
                        instance_id, err
                    );
                }
            }
        }
    }
}

pub fn ensure_listener(
    app_data_dir: PathBuf,
    session: &mut FriendLinkSessionRecord,
) -> Result<String, String> {
    if session.local_signing_private_b64.trim().is_empty() {
        return Err("friend-link signing identity is not loaded".to_string());
    }
    let key = session.instance_id.clone();
    let binding_fingerprint = listener_binding_fingerprint(session);
    let mut restarted = false;
    if let Ok(mut map) = listener_map().lock() {
        if let Some(existing) = map.get(&key) {
            if existing.binding_fingerprint == binding_fingerprint {
                let endpoint = endpoint_for_session_listener(session, existing.port);
                session.listener_port = existing.port;
                session.listener_endpoint = Some(endpoint.clone());
                return Ok(endpoint);
            }
        }
        if let Some(existing) = map.remove(&key) {
            let _ = existing.stop_tx.send(());
            if let Some(mapping) = existing.upnp_mapping.as_ref() {
                if let Err(err) = remove_upnp_mapping(mapping) {
                    eprintln!(
                        "friend-link upnp cleanup failed for instance '{}': {}",
                        session.instance_id, err
                    );
                }
            }
            session.listener_port = 0;
            restarted = true;
        }
    }
    if restarted {
        thread::sleep(Duration::from_millis(120));
    }

    let bind_ip = if session.allow_internet_endpoints {
        Ipv4Addr::UNSPECIFIED
    } else {
        Ipv4Addr::LOCALHOST
    };
    let listener = TcpListener::bind((bind_ip, session.listener_port))
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
    let local_signing_private_b64 = session.local_signing_private_b64.clone();

    thread::spawn(move || {
        let mut seen_nonces = HashMap::<String, i64>::new();
        let mut recent_connection_attempts = HashMap::<IpAddr, VecDeque<i64>>::new();
        loop {
            if stop_rx.try_recv().is_ok() {
                break;
            }
            match listener.accept() {
                Ok((mut stream, addr)) => {
                    let now_ms = now_millis();
                    if is_connection_rate_limited(
                        &mut recent_connection_attempts,
                        addr.ip(),
                        now_ms,
                    ) {
                        let _ = stream.shutdown(Shutdown::Both);
                        continue;
                    }
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
                        if let Ok(frame) = build_signed_frame_for_identity(
                            &group_id,
                            &local_peer_id,
                            &local_signing_private_b64,
                            "error",
                            payload,
                        ) {
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

    let endpoint = endpoint_for_session_listener(session, port);
    let mut advertised_endpoints = vec![endpoint.clone()];
    let mut upnp_mapping = None;
    if session.allow_internet_endpoints && session.allow_upnp_endpoints {
        if let Some((mapped_endpoint, mapping)) = try_map_upnp_listener_port(port) {
            if !advertised_endpoints.contains(&mapped_endpoint) {
                advertised_endpoints.push(mapped_endpoint);
            }
            upnp_mapping = Some(mapping);
        }
    }

    session.listener_port = port;
    session.listener_endpoint = Some(endpoint.clone());
    if let Ok(mut map) = listener_map().lock() {
        map.insert(
            key,
            ListenerHandle {
                port,
                binding_fingerprint,
                advertised_endpoints,
                upnp_mapping,
                stop_tx,
            },
        );
    }
    Ok(endpoint)
}

fn handle_incoming_frame(
    app_data_dir: &PathBuf,
    instance_id: &str,
    group_id: &str,
    _local_peer_id: &str,
    shared_secret_b64: &str,
    stream: &mut TcpStream,
    seen_nonces: &mut HashMap<String, i64>,
) -> Result<(), String> {
    let incoming = read_frame(stream, shared_secret_b64)?;
    if incoming.group_id != group_id {
        return Err("group mismatch".to_string());
    }
    verify_frame(&incoming)?;

    let store_path = store_path_from_app_data(app_data_dir);
    let mut store = read_store_at_path(&store_path)?;
    let mut payload_type = "error".to_string();
    let mut payload = serde_json::json!({ "ok": false, "error": "unsupported payload" });
    let mut response_group_id = group_id.to_string();
    let mut response_peer_id = String::new();
    let mut response_signing_private_b64 = String::new();
    let mut store_changed = false;

    if incoming.payload_type == "hello" {
        let mut hello: HelloPayload = serde_json::from_value(incoming.payload.clone())
            .map_err(|e| format!("parse hello payload failed: {e}"))?;
        hello.endpoint = normalize_peer_endpoint(&hello.endpoint, stream.peer_addr().ok());
        if hello.peer_id != incoming.from_peer_id {
            return Err("hello peer id mismatch".to_string());
        }
        let peer_key = {
            let session = get_session_mut(&mut store, instance_id)
                .ok_or_else(|| "friend-link session not found".to_string())?;
            response_group_id = session.group_id.clone();
            response_peer_id = session.local_peer_id.clone();
            let _ = get_session_signing_private_key(session)?;
            response_signing_private_b64 = session.local_signing_private_b64.clone();

            let verified = verify_incoming_frame_identity(session, &incoming)?;
            note_nonce_or_replay(seen_nonces, &incoming.nonce, now_millis())?;
            enforce_invite_usage_policy(session, hello.invite_id.as_deref())?;
            hello.endpoint = validate_endpoint_for_session(session, &hello.endpoint)?;
            register_peer_signing_key(session, &hello.peer_id, &verified.public_key_b64)?;
            verified.public_key_b64
        };

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
            session
                .peer_signing_public_keys
                .insert(hello.peer_id.clone(), peer_key.clone());

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
        store_changed = true;

        payload_type = "hello_ack".to_string();
        payload = serde_json::to_value(HelloAckPayload {
            peer_id: response_peer_id.clone(),
            display_name: local_display_name,
            endpoint: local_endpoint,
            peers: peer_summaries,
        })
        .map_err(|e| format!("serialize hello ack failed: {e}"))?;
    } else if incoming.payload_type == "state_request" {
        let _request: StateRequestPayload =
            serde_json::from_value(incoming.payload.clone()).unwrap_or(StateRequestPayload {});
        let session = get_session_mut(&mut store, instance_id)
            .ok_or_else(|| "friend-link session not found".to_string())?;
        response_group_id = session.group_id.clone();
        response_peer_id = session.local_peer_id.clone();
        let _ = get_session_signing_private_key(session)?;
        response_signing_private_b64 = session.local_signing_private_b64.clone();

        let _verified = verify_incoming_frame_identity(session, &incoming)?;
        note_nonce_or_replay(seen_nonces, &incoming.nonce, now_millis())?;
        let Some(peer) = session
            .peers
            .iter_mut()
            .find(|peer| peer.peer_id == incoming.from_peer_id)
        else {
            return Err("peer is not registered in this session".to_string());
        };
        peer.online = true;
        peer.last_seen_at = Some(now_iso());
        store_changed = true;

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
            peer_id: response_peer_id.clone(),
            display_name: session.display_name.clone(),
            endpoint,
            state,
        })
        .map_err(|e| format!("serialize state response failed: {e}"))?;
    } else if incoming.payload_type == "file_request" {
        let request: FileRequestPayload = serde_json::from_value(incoming.payload.clone())
            .map_err(|e| format!("parse file request payload failed: {e}"))?;
        let session = get_session_mut(&mut store, instance_id)
            .ok_or_else(|| "friend-link session not found".to_string())?;
        response_group_id = session.group_id.clone();
        response_peer_id = session.local_peer_id.clone();
        let _ = get_session_signing_private_key(session)?;
        response_signing_private_b64 = session.local_signing_private_b64.clone();

        let _verified = verify_incoming_frame_identity(session, &incoming)?;
        note_nonce_or_replay(seen_nonces, &incoming.nonce, now_millis())?;
        let Some(peer) = session
            .peers
            .iter_mut()
            .find(|peer| peer.peer_id == incoming.from_peer_id)
        else {
            return Err("peer is not registered in this session".to_string());
        };
        peer.online = true;
        peer.last_seen_at = Some(now_iso());
        store_changed = true;

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

    if store_changed {
        write_store_at_path(&store_path, &store)?;
    }

    if response_peer_id.trim().is_empty() {
        let session = get_session_mut(&mut store, instance_id)
            .ok_or_else(|| "friend-link session not found".to_string())?;
        response_group_id = session.group_id.clone();
        response_peer_id = session.local_peer_id.clone();
        let _ = get_session_signing_private_key(session)?;
        response_signing_private_b64 = session.local_signing_private_b64.clone();
    }

    let response = build_signed_frame_for_identity(
        &response_group_id,
        &response_peer_id,
        &response_signing_private_b64,
        &payload_type,
        payload,
    )?;
    write_frame(stream, shared_secret_b64, &response)?;
    let _ = stream.shutdown(Shutdown::Both);
    Ok(())
}

fn send_frame(
    session: &FriendLinkSessionRecord,
    endpoint: &str,
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

    write_frame(&mut stream, &session.shared_secret_b64, frame).map_err(map_secure_frame_error)?;
    let _ = stream.shutdown(Shutdown::Write);
    let response =
        read_frame(&mut stream, &session.shared_secret_b64).map_err(map_secure_frame_error)?;
    if response.group_id != session.group_id {
        return Err("peer returned mismatched group id".to_string());
    }
    verify_frame(&response)?;

    if let Some(pinned_key_b64) = session.peer_signing_public_keys.get(&response.from_peer_id) {
        let verifying_key = parse_verifying_key_b64(pinned_key_b64)?;
        verify_frame_signature_with_public_key(&response, &verifying_key)?;
        if let Some(frame_public_key_b64) = response.signing_public_key.as_ref() {
            if frame_public_key_b64.trim() != pinned_key_b64.trim() {
                return Err("peer signing key mismatch in response".to_string());
            }
        }
    } else if frame.payload_type == "hello" && response.payload_type == "hello_ack" {
        let key_b64 = response
            .signing_public_key
            .as_ref()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .ok_or_else(|| "hello ack missing host signing public key".to_string())?;
        let verifying_key = parse_verifying_key_b64(&key_b64)?;
        verify_frame_signature_with_public_key(&response, &verifying_key)?;
    } else {
        return Err("peer signing key is not trusted for this response".to_string());
    }

    Ok(response)
}

pub fn send_hello(
    session: &mut FriendLinkSessionRecord,
    endpoint: &str,
    payload: HelloPayload,
) -> Result<HelloAckPayload, String> {
    let endpoint = validate_endpoint_for_session(session, endpoint)?;
    let request = build_signed_frame(
        session,
        "hello",
        serde_json::to_value(payload)
            .map_err(|e| format!("serialize hello payload failed: {e}"))?,
    )?;

    let response = send_frame(session, &endpoint, &request)?;
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
    if let Some(expected_host_peer_id) = session.bootstrap_host_peer_id.as_ref() {
        if &response.from_peer_id != expected_host_peer_id {
            return Err(
                "invite host identity mismatch; regenerate invite and reconnect".to_string(),
            );
        }
    }
    let parsed = serde_json::from_value::<HelloAckPayload>(response.payload)
        .map_err(|e| format!("parse hello ack failed: {e}"))?;
    if parsed.peer_id != response.from_peer_id {
        return Err("hello ack peer identity mismatch".to_string());
    }
    if !session
        .peer_signing_public_keys
        .contains_key(&response.from_peer_id)
    {
        if let Some(key_b64) = response.signing_public_key.as_ref() {
            register_peer_signing_key(session, &response.from_peer_id, key_b64)?;
        }
    }
    Ok(parsed)
}

pub fn request_state(
    session: &FriendLinkSessionRecord,
    endpoint: &str,
) -> Result<StateResponsePayload, String> {
    let endpoint = validate_endpoint_for_session(session, endpoint)?;
    let request = build_signed_frame(
        session,
        "state_request",
        serde_json::to_value(StateRequestPayload {})
            .map_err(|e| format!("serialize state request payload failed: {e}"))?,
    )?;

    let response = send_frame(session, &endpoint, &request)?;
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
    let request = build_signed_frame(
        session,
        "file_request",
        serde_json::to_value(FileRequestPayload {
            key: key.to_string(),
        })
        .map_err(|e| format!("serialize file request payload failed: {e}"))?,
    )?;

    let response = send_frame(session, &endpoint, &request)?;
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
    use std::collections::{HashMap, VecDeque};
    use std::io::Cursor;

    fn test_secret_b64() -> String {
        BASE64_STANDARD.encode([7u8; 32])
    }

    fn test_session(instance_id: &str, secret_b64: &str) -> FriendLinkSessionRecord {
        let signing_key = SigningKey::generate(&mut rand::rngs::OsRng);
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
            allow_upnp_endpoints: false,
            public_endpoint_override: None,
            local_signing_key_id: String::new(),
            local_signing_private_b64: BASE64_STANDARD.encode(signing_key.to_bytes()),
            local_signing_public_key_b64: BASE64_STANDARD
                .encode(signing_key.verifying_key().to_bytes()),
            peer_signing_public_keys: HashMap::new(),
            invite_policies: HashMap::new(),
            invite_usage: HashMap::new(),
        }
    }

    fn test_frame() -> SignedFrame {
        let signing_key = SigningKey::generate(&mut rand::rngs::OsRng);
        let mut frame = SignedFrame {
            group_id: "group".to_string(),
            from_peer_id: "peer".to_string(),
            timestamp_ms: now_millis(),
            nonce: "nonce".to_string(),
            payload_type: "state_request".to_string(),
            payload: serde_json::json!({}),
            key_fingerprint: String::new(),
            signing_public_key: None,
            signature: String::new(),
        };
        sign_frame(&mut frame, &signing_key).expect("sign frame");
        frame
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

        let request_key = SigningKey::generate(&mut rand::rngs::OsRng);
        let mut request = SignedFrame {
            group_id: "group".to_string(),
            from_peer_id: "peer_a".to_string(),
            timestamp_ms: now_millis(),
            nonce: "req".to_string(),
            payload_type: "state_request".to_string(),
            payload: serde_json::json!({}),
            key_fingerprint: String::new(),
            signing_public_key: None,
            signature: String::new(),
        };
        sign_frame(&mut request, &request_key).expect("sign request");
        let request_plain = serde_json::to_vec(&request).expect("serialize request");
        let request_wire = encrypt_frame(&secret, &request_plain).expect("encrypt request");

        // Relay only sees opaque ciphertext bytes and cannot read plaintext fields.
        assert!(String::from_utf8(request_wire.clone()).is_err());

        let decoded_request =
            decrypt_frame(&secret, &request_wire).expect("receiver decrypts request from relay");
        let parsed_request: SignedFrame =
            serde_json::from_slice(&decoded_request).expect("parse request");
        assert_eq!(parsed_request.payload_type, "state_request");

        let response_key = SigningKey::generate(&mut rand::rngs::OsRng);
        let mut response = SignedFrame {
            group_id: "group".to_string(),
            from_peer_id: "peer_b".to_string(),
            timestamp_ms: now_millis(),
            nonce: "resp".to_string(),
            payload_type: "state_response".to_string(),
            payload: serde_json::json!({ "ok": true }),
            key_fingerprint: String::new(),
            signing_public_key: None,
            signature: String::new(),
        };
        sign_frame(&mut response, &response_key).expect("sign response");
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

    #[test]
    fn listener_endpoint_defaults_to_loopback_when_internet_mode_is_disabled() {
        let session = test_session("inst-loopback", &test_secret_b64());
        let endpoint = endpoint_for_session_listener(&session, 45555);
        assert_eq!(endpoint, "127.0.0.1:45555");
    }

    #[test]
    fn nonce_replay_tracking_rejects_duplicates_within_time_window() {
        let mut seen = HashMap::<String, i64>::new();
        let now = now_millis();
        note_nonce_or_replay(&mut seen, "abc", now).expect("first nonce insert");
        let replay = note_nonce_or_replay(&mut seen, "abc", now + 1);
        assert!(replay.is_err(), "duplicate nonce must be rejected");
    }

    #[test]
    fn connection_rate_limit_rejects_after_threshold() {
        let mut attempts = HashMap::<IpAddr, VecDeque<i64>>::new();
        let ip = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 25));
        let base = now_millis();
        for idx in 0..MAX_CONNECTIONS_PER_IP_PER_WINDOW {
            assert!(
                !is_connection_rate_limited(&mut attempts, ip, base + idx as i64),
                "attempt {idx} should be allowed"
            );
        }
        assert!(
            is_connection_rate_limited(
                &mut attempts,
                ip,
                base + MAX_CONNECTIONS_PER_IP_PER_WINDOW as i64 + 1
            ),
            "next attempt should be rate-limited"
        );
    }

    #[test]
    fn incoming_frame_rejects_peer_id_impersonation_with_mismatched_key() {
        let mut session = test_session("impersonation", &test_secret_b64());
        let pinned_key = SigningKey::generate(&mut rand::rngs::OsRng);
        let attacker_key = SigningKey::generate(&mut rand::rngs::OsRng);
        session.peer_signing_public_keys.insert(
            "peer_remote".to_string(),
            BASE64_STANDARD.encode(pinned_key.verifying_key().to_bytes()),
        );

        let mut frame = SignedFrame {
            group_id: session.group_id.clone(),
            from_peer_id: "peer_remote".to_string(),
            timestamp_ms: now_millis(),
            nonce: Uuid::new_v4().to_string(),
            payload_type: "state_request".to_string(),
            payload: serde_json::json!({}),
            key_fingerprint: String::new(),
            signing_public_key: None,
            signature: String::new(),
        };
        sign_frame(&mut frame, &attacker_key).expect("sign frame");

        let err = verify_incoming_frame_identity(&session, &frame)
            .expect_err("mismatched key should be rejected");
        assert!(err.to_ascii_lowercase().contains("mismatch"));
    }

    #[test]
    fn invite_usage_policy_rejects_overuse() {
        let mut session = test_session("invite-policy", &test_secret_b64());
        let invite_id = "invite_123";
        session.invite_policies.insert(
            invite_id.to_string(),
            crate::friend_link::store::FriendInvitePolicyRecord {
                invite_version: 2,
                max_uses: 1,
                expires_at: (chrono::Utc::now() + chrono::Duration::minutes(5)).to_rfc3339(),
                created_at: chrono::Utc::now().to_rfc3339(),
            },
        );
        enforce_invite_usage_policy(&mut session, Some(invite_id)).expect("first use");
        let err = enforce_invite_usage_policy(&mut session, Some(invite_id))
            .expect_err("overuse must fail");
        assert!(err.to_ascii_lowercase().contains("maximum"));
    }

    #[test]
    fn ensure_listener_does_not_attempt_upnp_when_opt_out() {
        let instance_id = format!("inst-no-upnp-{}", Uuid::new_v4());
        let app_data_dir =
            std::env::temp_dir().join(format!("openjar-friend-net-upnp-{}", Uuid::new_v4()));
        let mut session = test_session(&instance_id, &test_secret_b64());
        session.allow_internet_endpoints = true;
        session.allow_upnp_endpoints = false;

        let _ = ensure_listener(app_data_dir, &mut session).expect("listener startup");
        let upnp_mapping_present = {
            let guard = listener_map().lock().expect("listener map lock");
            guard
                .get(&instance_id)
                .and_then(|handle| handle.upnp_mapping.as_ref())
                .is_some()
        };
        assert!(
            !upnp_mapping_present,
            "UPnP mapping should not be present when allow_upnp_endpoints is false"
        );
        stop_listener(&instance_id);
    }
}
