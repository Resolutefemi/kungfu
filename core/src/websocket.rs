//! WebSocket support (RFC 6455).
//!
//! Provides frame parsing, frame encoding, and an upgrade path from HTTP.
//!
//! ## Example
//!
//! ```ignore
//! use kungfu::websocket::{WebSocket, WebSocketMessage};
//! use kungfu::prelude::*;
//!
//! Kungfu::new()
//!     .ws("/chat", |mut ws: WebSocket| async move {
//!         while let Some(msg) = ws.recv().await {
//!             match msg {
//!                 WebSocketMessage::Text(text) => {
//!                     ws.send_text(format!("echo: {text}")).await;
//!                 }
//!                 WebSocketMessage::Close => break,
//!                 _ => {}
//!             }
//!         }
//!     })
//!     .run("0.0.0.0:3000")
//! ```

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;

use base64::Engine;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

/// A WebSocket message received from the client.
#[derive(Debug, Clone)]
pub enum WebSocketMessage {
    Text(String),
    Binary(Vec<u8>),
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Close,
}

/// A WebSocket connection. Owns the underlying TCP stream after the HTTP
/// upgrade handshake completes.
pub struct WebSocket {
    stream: TcpStream,
}

impl WebSocket {
    pub fn new(stream: TcpStream) -> Self {
        Self { stream }
    }

    /// Receive the next message from the client. Returns `None` on EOF or
    /// protocol error.
    pub async fn recv(&mut self) -> Option<WebSocketMessage> {
        loop {
            // Read frame header (minimum 2 bytes).
            let mut header = [0u8; 2];
            if self.stream.read_exact(&mut header).await.is_err() {
                return None;
            }

            let fin = header[0] & 0x80 != 0;
            let opcode = header[0] & 0x0f;
            let masked = header[1] & 0x80 != 0;
            let payload_len = (header[1] & 0x7f) as u64;

            // Extended payload length.
            let actual_len = match payload_len {
                0..=125 => payload_len,
                126 => {
                    let mut ext = [0u8; 2];
                    if self.stream.read_exact(&mut ext).await.is_err() {
                        return None;
                    }
                    u16::from_be_bytes(ext) as u64
                }
                127 => {
                    let mut ext = [0u8; 8];
                    if self.stream.read_exact(&mut ext).await.is_err() {
                        return None;
                    }
                    u64::from_be_bytes(ext)
                }
                _ => unreachable!(),
            };

            // Masking key (4 bytes, if masked).
            let mut mask = [0u8; 4];
            if masked {
                if self.stream.read_exact(&mut mask).await.is_err() {
                    return None;
                }
            }

            // Payload.
            let mut payload = vec![0u8; actual_len as usize];
            if actual_len > 0 {
                if self.stream.read_exact(&mut payload).await.is_err() {
                    return None;
                }
                if masked {
                    for (i, b) in payload.iter_mut().enumerate() {
                        *b ^= mask[i % 4];
                    }
                }
            }

            match opcode {
                0x0 => continue, // continuation — not fully supported in V1
                0x1 => {
                    // text
                    let text = String::from_utf8(payload).unwrap_or_default();
                    return Some(WebSocketMessage::Text(text));
                }
                0x2 => {
                    // binary
                    return Some(WebSocketMessage::Binary(payload));
                }
                0x8 => {
                    // close
                    return Some(WebSocketMessage::Close);
                }
                0x9 => {
                    // ping — auto-respond with pong
                    let _ = self.send_frame(0xA, &payload).await;
                    if fin {
                        return Some(WebSocketMessage::Ping(payload));
                    }
                }
                0xA => {
                    // pong
                    if fin {
                        return Some(WebSocketMessage::Pong(payload));
                    }
                }
                _ => return None,
            }
        }
    }

    /// Send a text message.
    pub async fn send_text(&mut self, text: impl Into<String>) {
        let payload = text.into().into_bytes();
        self.send_frame(0x1, &payload).await;
    }

    /// Send a binary message.
    pub async fn send_binary(&mut self, data: &[u8]) {
        self.send_frame(0x2, data).await;
    }

    /// Send a close frame and end the connection.
    pub async fn close(&mut self) {
        self.send_frame(0x8, &[]).await;
        let _ = self.stream.shutdown().await;
    }

    /// Write a single WebSocket frame. Servers MUST NOT mask frames.
    async fn send_frame(&mut self, opcode: u8, payload: &[u8]) {
        let mut header = Vec::with_capacity(14);
        // FIN + opcode
        header.push(0x80 | opcode);
        // Length (no mask bit set).
        let len = payload.len();
        if len <= 125 {
            header.push(len as u8);
        } else if len <= 65535 {
            header.push(126);
            header.extend_from_slice(&(len as u16).to_be_bytes());
        } else {
            header.push(127);
            header.extend_from_slice(&(len as u64).to_be_bytes());
        }
        let _ = self.stream.write_all(&header).await;
        if !payload.is_empty() {
            let _ = self.stream.write_all(payload).await;
        }
        let _ = self.stream.flush().await;
    }
}

/// Compute the `Sec-WebSocket-Accept` header value from the client's
/// `Sec-WebSocket-Key`. Per RFC 6455 §1.3.
pub fn compute_accept_key(client_key: &str) -> String {
    const MAGIC: &str = "258EAFA5-E914-47DA-95CA-C5AB0DC85B11";
    let mut input = String::with_capacity(client_key.len() + MAGIC.len());
    input.push_str(client_key);
    input.push_str(MAGIC);
    let hash = ring::digest::digest(&ring::digest::SHA1_FOR_LEGACY_USE_ONLY, input.as_bytes());
    base64::engine::general_purpose::STANDARD.encode(hash)
}

/// Trait for WebSocket handlers. The handler receives an upgraded
/// `WebSocket` connection and runs until the client disconnects.
pub type WebSocketHandler = Arc<
    dyn Fn(WebSocket) -> Pin<Box<dyn Future<Output = ()> + Send>>
        + Send
        + Sync
        + 'static,
>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn computes_accept_key_correctly() {
        // RFC 6455 §4.2.2 example.
        let key = "dGhlIHNhbXBsZSBub25jZQ==";
        let accept = compute_accept_key(key);
        assert_eq!(accept, "s3pPLMBiTxaQ9kYGzzhZRbK+xOo=");
    }
}
