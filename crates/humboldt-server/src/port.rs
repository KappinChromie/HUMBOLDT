use humboldt_core::{HumboldtError, Result};
use std::net::SocketAddr;
use tokio::net::TcpListener;
use tracing::info;

/// Dynamic port allocation matching HUMBOLDT Architecture Spec Section 3.1.
/// Tries start_port..=end_port sequentially on EADDRINUSE.
pub async fn bind_dynamic_listener(
    bind_ip: &str,
    start_port: u16,
    end_port: u16,
) -> Result<(TcpListener, u16)> {
    let ip: std::net::IpAddr = bind_ip
        .parse()
        .map_err(|e| HumboldtError::Config(format!("Invalid bind IP address '{}': {}", bind_ip, e)))?;

    for port in start_port..=end_port {
        let addr = SocketAddr::new(ip, port);
        match TcpListener::bind(addr).await {
            Ok(listener) => {
                info!("Successfully bound TCP listener to {}", addr);
                return Ok((listener, port));
            }
            Err(err) if err.kind() == std::io::ErrorKind::AddrInUse => {
                info!("Port {} is already in use, trying next candidate...", port);
                continue;
            }
            Err(err) => {
                return Err(HumboldtError::Io(err));
            }
        }
    }

    Err(HumboldtError::PortAllocationExhausted {
        start: start_port,
        end: end_port,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_bind_dynamic_listener_success() {
        // Find an available port in a high test range
        let (listener, port) = bind_dynamic_listener("127.0.0.1", 18374, 18380)
            .await
            .expect("Should bind to an available port");

        assert!((18374..=18380).contains(&port));
        assert_eq!(listener.local_addr().unwrap().port(), port);
    }

    #[tokio::test]
    async fn test_bind_dynamic_listener_fallback_on_in_use() {
        // Pre-bind the first port to force fallback to the second
        let blocker = TcpListener::bind("127.0.0.1:18385").await.unwrap();
        let blocker_port = blocker.local_addr().unwrap().port();

        let (listener, port) = bind_dynamic_listener("127.0.0.1", blocker_port, blocker_port + 2)
            .await
            .expect("Should fallback to next port");

        assert_eq!(port, blocker_port + 1);
        drop(blocker);
        drop(listener);
    }
}
