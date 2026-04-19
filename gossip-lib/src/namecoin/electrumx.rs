//! ElectrumX TCP/TLS client for Namecoin name lookups.
//!
//! Communicates with ElectrumX-NMC servers using JSON-RPC over TLS.
//! Uses the scripthash-based approach described in the Electrum protocol:
//! 1. Build canonical name index script
//! 2. Compute Electrum-style scripthash (reversed SHA-256)
//! 3. Query `blockchain.scripthash.get_history` for tx history
//! 4. Fetch and parse NAME_UPDATE from the latest transaction
//! 5. Check name expiry via `blockchain.headers.subscribe`
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::sync::Arc;
use std::time::Duration;

/// An ElectrumX server endpoint.
#[derive(Debug, Clone)]
pub struct ElectrumxServer {
    pub host: String,
    pub port: u16,
}

pub fn default_servers() -> Vec<ElectrumxServer> {
    vec![
        ElectrumxServer {
            host: "electrumx.testls.space".to_string(),
            port: 50002,
        },
        ElectrumxServer {
            host: "nmc2.bitcoins.sk".to_string(),
            port: 57002,
        },
        ElectrumxServer {
            host: "46.229.238.187".to_string(),
            port: 57002,
        },
    ]
}

/// Result from a name lookup.
#[derive(Debug, Clone)]
pub struct NameShowResult {
    /// The raw JSON value stored on-chain.
    pub value: String,
    /// Block height of the last NAME_UPDATE.
    pub height: i64,
    /// Blocks until expiry (None if not computed).
    pub expires_in: Option<i64>,
}

/// Namecoin names expire 36,000 blocks (~250 days) after their last update.
const NAME_EXPIRY_BLOCKS: i64 = 36_000;

/// Errors from ElectrumX operations.
#[derive(Debug)]
pub enum ElectrumxError {
    Io(std::io::Error),
    Tls(String),
    Json(serde_json::Error),
    RpcError(String),
    NameNotFound,
    NameExpired,
    NoServersAvailable,
    ParseError(String),
    Timeout,
}

impl std::fmt::Display for ElectrumxError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "IO error: {e}"),
            Self::Tls(e) => write!(f, "TLS error: {e}"),
            Self::Json(e) => write!(f, "JSON error: {e}"),
            Self::RpcError(e) => write!(f, "RPC error: {e}"),
            Self::NameNotFound => write!(f, "Name not found"),
            Self::NameExpired => write!(f, "Name expired"),
            Self::NoServersAvailable => write!(f, "No ElectrumX servers available"),
            Self::ParseError(e) => write!(f, "Parse error: {e}"),
            Self::Timeout => write!(f, "Connection timeout"),
        }
    }
}

impl std::error::Error for ElectrumxError {}

impl From<std::io::Error> for ElectrumxError {
    fn from(e: std::io::Error) -> Self {
        if e.kind() == std::io::ErrorKind::TimedOut {
            Self::Timeout
        } else {
            Self::Io(e)
        }
    }
}

impl From<serde_json::Error> for ElectrumxError {
    fn from(e: serde_json::Error) -> Self {
        Self::Json(e)
    }
}

/// A TLS certificate verifier that accepts self-signed certificates.
/// ElectrumX-NMC servers commonly use self-signed TLS certificates.
#[derive(Debug)]
struct AcceptAnyCert;

impl rustls::client::danger::ServerCertVerifier for AcceptAnyCert {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn verify_tls13_signature(
        &self,
        _message: &[u8],
        _cert: &rustls::pki_types::CertificateDer<'_>,
        _dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        Ok(rustls::client::danger::HandshakeSignatureValid::assertion())
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        vec![
            rustls::SignatureScheme::RSA_PKCS1_SHA256,
            rustls::SignatureScheme::RSA_PKCS1_SHA384,
            rustls::SignatureScheme::RSA_PKCS1_SHA512,
            rustls::SignatureScheme::ECDSA_NISTP256_SHA256,
            rustls::SignatureScheme::ECDSA_NISTP384_SHA384,
            rustls::SignatureScheme::ECDSA_NISTP521_SHA512,
            rustls::SignatureScheme::RSA_PSS_SHA256,
            rustls::SignatureScheme::RSA_PSS_SHA384,
            rustls::SignatureScheme::RSA_PSS_SHA512,
            rustls::SignatureScheme::ED25519,
            rustls::SignatureScheme::ED448,
        ]
    }
}

/// Build the canonical name index script for a Namecoin name.
///
/// ElectrumX-NMC indexes names using a canonical script derived from the name.
/// The format is:
///   `OP_NAME_UPDATE + push(name) + push(empty) + OP_2DROP + OP_DROP + OP_RETURN`
fn build_name_script(name: &str) -> Vec<u8> {
    let name_bytes = name.as_bytes();
    let mut script = Vec::with_capacity(6 + name_bytes.len());

    // OP_NAME_UPDATE = OP_3 = 0x53 in Namecoin
    script.push(0x53);

    // push_data(name_bytes)
    push_data(&mut script, name_bytes);

    // push_data(empty) — canonical index script uses empty value
    push_data(&mut script, &[]);

    // OP_2DROP OP_DROP OP_RETURN
    script.push(0x6d); // OP_2DROP
    script.push(0x75); // OP_DROP
    script.push(0x6a); // OP_RETURN

    script
}

/// Bitcoin-style push data encoding.
fn push_data(script: &mut Vec<u8>, data: &[u8]) {
    let len = data.len();
    if len < 0x4c {
        script.push(len as u8);
    } else if len <= 0xff {
        script.push(0x4c); // OP_PUSHDATA1
        script.push(len as u8);
    } else {
        script.push(0x4d); // OP_PUSHDATA2
        script.extend_from_slice(&(len as u16).to_le_bytes());
    }
    script.extend_from_slice(data);
}

/// Compute the Electrum-style scripthash: reversed SHA-256 of the script, as hex.
fn compute_scripthash(name: &str) -> String {
    let script = build_name_script(name);
    let hash = Sha256::digest(&script);
    let mut reversed = hash.to_vec();
    reversed.reverse();
    hex::encode(reversed)
}

/// Perform a JSON-RPC call over a buffered TLS stream and return the result.
fn rpc_call(
    reader: &mut BufReader<rustls::StreamOwned<rustls::ClientConnection, TcpStream>>,
    method: &str,
    params: &serde_json::Value,
    id: u64,
) -> Result<serde_json::Value, ElectrumxError> {
    let request = serde_json::json!({
        "jsonrpc": "2.0",
        "method": method,
        "params": params,
        "id": id,
    });

    let mut request_str = serde_json::to_string(&request)?;
    request_str.push('\n');
    reader.get_mut().write_all(request_str.as_bytes())?;
    reader.get_mut().flush()?;

    // Read response line (JSON-RPC uses newline-delimited messages)
    let mut response_line = String::new();
    reader.read_line(&mut response_line)?;

    let response: serde_json::Value = serde_json::from_str(&response_line)?;

    if let Some(error) = response.get("error") {
        if !error.is_null() {
            return Err(ElectrumxError::RpcError(error.to_string()));
        }
    }

    response
        .get("result")
        .cloned()
        .ok_or_else(|| ElectrumxError::RpcError("Missing result field".to_string()))
}

/// Read the SOCKS5 proxy setting. Empty string = no proxy (direct TCP).
///
/// Uses `catch_unwind` because `GLOBALS.db()` panics if storage isn't initialized
/// (e.g. in unit tests or very early boot). Empty string falls back to direct TCP,
/// matching the pre-SOCKS5 behaviour.
fn read_socks5_proxy_setting() -> String {
    match std::panic::catch_unwind(|| {
        crate::globals::GLOBALS
            .db()
            .read_setting_namecoin_socks5_proxy()
    }) {
        Ok(s) => s,
        Err(_) => String::new(),
    }
}

/// Connect to an ElectrumX server via TLS, optionally through a SOCKS5 proxy.
fn connect_tls(
    server: &ElectrumxServer,
) -> Result<rustls::StreamOwned<rustls::ClientConnection, TcpStream>, ElectrumxError> {
    use std::net::ToSocketAddrs;

    let proxy = read_socks5_proxy_setting();

    if !proxy.is_empty() {
        tracing::info!(
            "electrumx: dialing {}:{} via SOCKS5 proxy {}",
            server.host,
            server.port,
            proxy
        );
    } else {
        tracing::info!("electrumx: dialing {}:{} direct", server.host, server.port);
    }

    let tcp = if proxy.is_empty() {
        let addr_str = format!("{}:{}", server.host, server.port);
        let addrs: Vec<_> = addr_str
            .to_socket_addrs()
            .map_err(ElectrumxError::Io)?
            .collect();

        let first = addrs.first().ok_or_else(|| {
            ElectrumxError::Io(std::io::Error::other("DNS resolution failed"))
        })?;

        TcpStream::connect_timeout(first, Duration::from_secs(10))?
    } else {
        // Route through SOCKS5. DNS resolution happens at the proxy, which is
        // important for Tor (prevents local DNS leaks).
        let target = (server.host.as_str(), server.port);
        let socks_stream = socks::Socks5Stream::connect(proxy.as_str(), target).map_err(|e| {
            tracing::warn!(
                "electrumx: SOCKS5 dial to {} via {} failed: {}",
                server.host,
                proxy,
                e
            );
            ElectrumxError::Io(e)
        })?;
        socks_stream.into_inner()
    };

    tcp.set_read_timeout(Some(Duration::from_secs(15)))?;
    tcp.set_write_timeout(Some(Duration::from_secs(10)))?;

    // Build TLS config accepting self-signed certificates
    let config = rustls::ClientConfig::builder()
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AcceptAnyCert))
        .with_no_client_auth();

    // Use the hostname for SNI, falling back to a dummy name for IP addresses
    let server_name: rustls::pki_types::ServerName<'static> = server
        .host
        .clone()
        .try_into()
        .unwrap_or_else(|_| "electrumx".to_string().try_into().unwrap());

    let tls_conn = rustls::ClientConnection::new(Arc::new(config), server_name)
        .map_err(|e| ElectrumxError::Tls(e.to_string()))?;

    Ok(rustls::StreamOwned::new(tls_conn, tcp))
}

/// Parse a NAME_UPDATE value from a raw transaction output script.
fn parse_name_value_from_script(script_hex: &str) -> Option<String> {
    let script = hex::decode(script_hex).ok()?;

    if script.is_empty() {
        return None;
    }

    // Check for OP_NAME_UPDATE (0x53) at the start
    if script[0] != 0x53 {
        return None;
    }

    let mut pos = 1;

    // Skip first push data (the name)
    pos = skip_push_data(&script, pos)?;

    // Read second push data (the value)
    let (value_bytes, _) = read_push_data(&script, pos)?;

    String::from_utf8(value_bytes).ok()
}

/// Skip over a push data element in a script, returning the new position.
fn skip_push_data(script: &[u8], pos: usize) -> Option<usize> {
    if pos >= script.len() {
        return None;
    }

    let opcode = script[pos];

    if opcode < 76 {
        let len = opcode as usize;
        let end = pos + 1 + len;
        if end > script.len() {
            return None;
        }
        Some(end)
    } else if opcode == 0x4c {
        if pos + 1 >= script.len() {
            return None;
        }
        let len = script[pos + 1] as usize;
        let end = pos + 2 + len;
        if end > script.len() {
            return None;
        }
        Some(end)
    } else if opcode == 0x4d {
        if pos + 2 >= script.len() {
            return None;
        }
        let len = u16::from_le_bytes([script[pos + 1], script[pos + 2]]) as usize;
        let end = pos + 3 + len;
        if end > script.len() {
            return None;
        }
        Some(end)
    } else {
        None
    }
}

/// Read push data from a script, returning (data, new_position).
fn read_push_data(script: &[u8], pos: usize) -> Option<(Vec<u8>, usize)> {
    if pos >= script.len() {
        return None;
    }

    let opcode = script[pos];

    if opcode < 76 {
        let len = opcode as usize;
        let start = pos + 1;
        let end = start + len;
        if end > script.len() {
            return None;
        }
        Some((script[start..end].to_vec(), end))
    } else if opcode == 0x4c {
        if pos + 1 >= script.len() {
            return None;
        }
        let len = script[pos + 1] as usize;
        let start = pos + 2;
        let end = start + len;
        if end > script.len() {
            return None;
        }
        Some((script[start..end].to_vec(), end))
    } else if opcode == 0x4d {
        if pos + 2 >= script.len() {
            return None;
        }
        let len = u16::from_le_bytes([script[pos + 1], script[pos + 2]]) as usize;
        let start = pos + 3;
        let end = start + len;
        if end > script.len() {
            return None;
        }
        Some((script[start..end].to_vec(), end))
    } else {
        None
    }
}

/// Parse the raw hex transaction to find NAME_UPDATE outputs and extract the value.
fn extract_name_value_from_raw_tx(raw_hex: &str) -> Option<String> {
    let raw = hex::decode(raw_hex).ok()?;

    // Minimal Bitcoin transaction parsing to find output scripts
    let mut pos = 4; // skip version

    // Check for segwit marker
    let has_witness = pos + 1 < raw.len() && raw[pos] == 0x00 && raw[pos + 1] != 0x00;
    if has_witness {
        pos += 2; // skip marker + flag
    }

    // Skip inputs
    let (vin_count, new_pos) = read_varint(&raw, pos)?;
    pos = new_pos;

    for _ in 0..vin_count {
        pos += 32; // txid
        pos += 4; // vout index
        let (script_len, new_pos) = read_varint(&raw, pos)?;
        pos = new_pos + script_len as usize; // skip script
        pos += 4; // sequence
        if pos > raw.len() {
            return None;
        }
    }

    // Parse outputs - look for NAME_UPDATE
    let (vout_count, new_pos) = read_varint(&raw, pos)?;
    pos = new_pos;

    for _ in 0..vout_count {
        pos += 8; // value (satoshis)
        let (script_len, new_pos) = read_varint(&raw, pos)?;
        pos = new_pos;

        if pos + script_len as usize > raw.len() {
            return None;
        }

        let script = &raw[pos..pos + script_len as usize];

        // Check if this output starts with OP_NAME_UPDATE (0x53)
        if !script.is_empty() && script[0] == 0x53 {
            let script_hex = hex::encode(script);
            if let Some(value) = parse_name_value_from_script(&script_hex) {
                return Some(value);
            }
        }

        pos += script_len as usize;
    }

    None
}

/// Read a Bitcoin-style varint.
fn read_varint(data: &[u8], pos: usize) -> Option<(u64, usize)> {
    if pos >= data.len() {
        return None;
    }

    let first = data[pos];
    match first {
        0..=0xfc => Some((first as u64, pos + 1)),
        0xfd => {
            if pos + 2 >= data.len() {
                return None;
            }
            let val = u16::from_le_bytes([data[pos + 1], data[pos + 2]]) as u64;
            Some((val, pos + 3))
        }
        0xfe => {
            if pos + 4 >= data.len() {
                return None;
            }
            let val = u32::from_le_bytes([
                data[pos + 1],
                data[pos + 2],
                data[pos + 3],
                data[pos + 4],
            ]) as u64;
            Some((val, pos + 5))
        }
        0xff => {
            if pos + 8 >= data.len() {
                return None;
            }
            let val = u64::from_le_bytes([
                data[pos + 1],
                data[pos + 2],
                data[pos + 3],
                data[pos + 4],
                data[pos + 5],
                data[pos + 6],
                data[pos + 7],
                data[pos + 8],
            ]);
            Some((val, pos + 9))
        }
    }
}

/// Look up a Namecoin name via a single ElectrumX server.
fn name_show_single(
    server: &ElectrumxServer,
    name: &str,
) -> Result<NameShowResult, ElectrumxError> {
    let stream = connect_tls(server)?;
    let mut reader = BufReader::new(stream);
    let mut rpc_id: u64 = 1;

    let scripthash = compute_scripthash(name);

    let history = rpc_call(
        &mut reader,
        "blockchain.scripthash.get_history",
        &serde_json::json!([scripthash]),
        rpc_id,
    )?;
    rpc_id += 1;

    let history_arr = history.as_array().ok_or(ElectrumxError::NameNotFound)?;

    if history_arr.is_empty() {
        return Err(ElectrumxError::NameNotFound);
    }

    // Find the transaction with the highest height (most recent)
    let latest = history_arr
        .iter()
        .max_by_key(|entry| entry.get("height").and_then(|h| h.as_i64()).unwrap_or(0))
        .ok_or(ElectrumxError::NameNotFound)?;

    let tx_hash = latest
        .get("tx_hash")
        .and_then(|h| h.as_str())
        .ok_or(ElectrumxError::ParseError("missing tx_hash".to_string()))?;

    let tx_height = latest.get("height").and_then(|h| h.as_i64()).unwrap_or(0);

    let raw_tx = rpc_call(
        &mut reader,
        "blockchain.transaction.get",
        &serde_json::json!([tx_hash]),
        rpc_id,
    )?;
    rpc_id += 1;

    let raw_hex = raw_tx
        .as_str()
        .ok_or(ElectrumxError::ParseError("tx not a string".to_string()))?;

    let value = extract_name_value_from_raw_tx(raw_hex)
        .ok_or(ElectrumxError::ParseError("no NAME_UPDATE in tx".to_string()))?;

    let headers_result = rpc_call(
        &mut reader,
        "blockchain.headers.subscribe",
        &serde_json::json!([]),
        rpc_id,
    );

    let mut expires_in = None;

    if let Ok(headers) = headers_result {
        if let Some(current_height) = headers.get("height").and_then(|h| h.as_i64()) {
            if tx_height > 0 {
                let blocks_since_update = current_height - tx_height;
                if blocks_since_update >= NAME_EXPIRY_BLOCKS {
                    return Err(ElectrumxError::NameExpired);
                }
                expires_in = Some(NAME_EXPIRY_BLOCKS - blocks_since_update);
            }
        }
    }

    Ok(NameShowResult {
        value,
        height: tx_height,
        expires_in,
    })
}

/// Look up a Namecoin name, trying multiple servers with fallback.
pub fn name_show(
    servers: &[ElectrumxServer],
    name: &str,
) -> Result<NameShowResult, ElectrumxError> {
    if servers.is_empty() {
        return Err(ElectrumxError::NoServersAvailable);
    }

    let mut last_error = ElectrumxError::NoServersAvailable;

    for server in servers {
        tracing::debug!(
            "Trying ElectrumX server {}:{} for name '{}'",
            server.host,
            server.port,
            name
        );

        match name_show_single(server, name) {
            Ok(result) => return Ok(result),
            Err(e) => {
                tracing::warn!(
                    "ElectrumX server {}:{} failed for '{}': {}",
                    server.host,
                    server.port,
                    name,
                    e
                );
                last_error = e;
            }
        }
    }

    Err(last_error)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_scripthash() {
        let hash = compute_scripthash("d/testls");
        assert_eq!(hash.len(), 64);
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_build_name_script() {
        let script = build_name_script("d/testls");
        assert_eq!(script[0], 0x53); // OP_NAME_UPDATE
        assert_eq!(script[1], 8); // push length
        assert_eq!(&script[2..10], b"d/testls");
        assert_eq!(script[10], 0x00); // push(empty) — length 0
        assert_eq!(script[11], 0x6d); // OP_2DROP
        assert_eq!(script[12], 0x75); // OP_DROP
        assert_eq!(script[13], 0x6a); // OP_RETURN
        assert_eq!(script.len(), 14);
    }

    #[test]
    fn test_parse_name_value_simple() {
        let name = b"d/test";
        let value = b"{\"nostr\":\"abc\"}";

        let mut script = vec![0x53]; // OP_NAME_UPDATE
        script.push(name.len() as u8);
        script.extend_from_slice(name);
        script.push(value.len() as u8);
        script.extend_from_slice(value);

        let hex = hex::encode(&script);
        let parsed = parse_name_value_from_script(&hex);
        assert_eq!(parsed, Some("{\"nostr\":\"abc\"}".to_string()));
    }

    #[test]
    fn test_read_varint() {
        assert_eq!(read_varint(&[0x01], 0), Some((1, 1)));
        assert_eq!(read_varint(&[0xfc], 0), Some((252, 1)));
        assert_eq!(read_varint(&[0xfd, 0x00, 0x01], 0), Some((256, 3)));
    }
}
