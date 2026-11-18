//! LAN mode (ADR-031 item 3, ADR-042): the pairing token and its constant-time check.
use std::path::{Path, PathBuf};

/// Where the token lives: `api.lan.token_file`, else `<data_dir>/lan-token`.
pub fn token_path(config: &crate::config::Config) -> PathBuf {
    config
        .api
        .lan
        .token_file
        .clone()
        .unwrap_or_else(|| config.engine.data_dir.join("lan-token"))
}

/// Reads the token, creating one with 128 random bits on first use.
pub fn load_or_create(path: &Path) -> Result<String, String> {
    if let Ok(t) = std::fs::read_to_string(path) {
        let t = t.trim().to_owned();
        if t.len() >= 16 {
            return Ok(t);
        }
        return Err(format!(
            "{}: token is shorter than 16 characters",
            path.display()
        ));
    }
    let mut bytes = [0u8; 16];
    getrandom::fill(&mut bytes).map_err(|e| format!("random token: {e}"))?;
    let token: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
    }
    std::fs::write(path, &token).map_err(|e| format!("{}: {e}", path.display()))?;
    Ok(token)
}

/// Constant-time comparison: the time taken does not depend on where the strings differ.
pub fn matches(given: &str, token: &str) -> bool {
    let (a, b) = (given.as_bytes(), token.as_bytes());
    let mut diff = a.len() ^ b.len();
    for (i, x) in b.iter().enumerate() {
        diff |= usize::from(a.get(i).copied().unwrap_or(0) ^ x);
    }
    diff == 0
}

/// The address other machines on the network reach this PC at: the bind address unless it is
/// unspecified, else the interface the default route uses (a UDP `connect` sends no packet).
pub fn lan_address(bind: std::net::SocketAddr) -> Option<std::net::IpAddr> {
    if !bind.ip().is_unspecified() {
        return Some(bind.ip());
    }
    let s = std::net::UdpSocket::bind("0.0.0.0:0").ok()?;
    s.connect("192.0.2.1:9").ok()?;
    s.local_addr()
        .ok()
        .map(|a| a.ip())
        .filter(|ip| !ip.is_loopback())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn token_is_created_once_and_compared_whole() {
        let dir = tempfile::tempdir().unwrap();
        let p = dir.path().join("sub/lan-token");
        let t = load_or_create(&p).unwrap();
        assert_eq!(t.len(), 32);
        assert_eq!(load_or_create(&p).unwrap(), t);
        assert!(matches(&t, &t));
        assert!(!matches(&t[..31], &t));
        assert!(!matches(&format!("{t}x"), &t));
        assert!(!matches("", &t));
    }
}
