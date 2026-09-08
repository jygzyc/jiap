//! Server port selection: validation, availability probing, and random
//! assignment in the default 30000–40000 range.

use std::net::TcpListener;

use crate::error::{DecxError, DecxResult};

pub const MIN_SERVER_PORT: u16 = 1001;
pub const MAX_SERVER_PORT: u16 = 65535;

/// Default range for randomly assigning a server port when none is requested.
pub const RANDOM_PORT_RANGE_MIN: u16 = 30000;
pub const RANDOM_PORT_RANGE_MAX: u16 = 40000;

/// Validate a user-supplied port value (digits only, within the legal range).
pub fn parse_server_port(value: &str) -> DecxResult<u16> {
    let port: u16 = value
        .parse()
        .map_err(|_| DecxError::invalid_port(value))?;
    if !(MIN_SERVER_PORT..=MAX_SERVER_PORT).contains(&port) {
        return Err(DecxError::invalid_port(value));
    }
    Ok(port)
}

/// Check whether a TCP port on 127.0.0.1 can be bound right now.
pub fn is_port_available(port: u16) -> bool {
    TcpListener::bind(("127.0.0.1", port)).is_ok()
}

fn random_u64() -> u64 {
    use std::hash::{BuildHasher, Hasher, RandomState};
    RandomState::new().build_hasher().finish()
}

fn random_port_in_range() -> u16 {
    let span = u32::from(RANDOM_PORT_RANGE_MAX - RANDOM_PORT_RANGE_MIN) + 1;
    RANDOM_PORT_RANGE_MIN + (random_u64() % u64::from(span)) as u16
}

/// Pick a server port: honor an explicit request when free, otherwise probe
/// random ports in the default range until one binds.
pub fn select_available_server_port(preferred: Option<u16>) -> DecxResult<u16> {
    if let Some(port) = preferred {
        if is_port_available(port) {
            return Ok(port);
        }
    }
    for _ in 0..100 {
        let port = random_port_in_range();
        if is_port_available(port) {
            return Ok(port);
        }
    }
    Err(DecxError::process(format!(
        "Failed to find an available port in [{RANDOM_PORT_RANGE_MIN}, {RANDOM_PORT_RANGE_MAX}]"
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_rejects_out_of_range() {
        assert!(parse_server_port("0").is_err());
        assert!(parse_server_port("1000").is_err());
        assert!(parse_server_port("65536").is_err());
        assert!(parse_server_port("abc").is_err());
        assert!(parse_server_port("-1").is_err());
        assert_eq!(parse_server_port("25419").unwrap(), 25419);
    }

    #[test]
    fn select_prefers_free_requested_port() {
        // bind one port so it is busy, and pick another free one explicitly
        let listener = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let busy = listener.local_addr().unwrap().port();
        let got = select_available_server_port(Some(busy)).unwrap();
        assert_ne!(got, busy, "must skip the busy port");
        assert!(is_port_available(got));
    }

    #[test]
    fn select_finds_port_in_default_range() {
        let port = select_available_server_port(None).unwrap();
        assert!((RANDOM_PORT_RANGE_MIN..=RANDOM_PORT_RANGE_MAX).contains(&port));
    }
}
