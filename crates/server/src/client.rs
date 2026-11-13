//! O lado da cópia na conversa com o Anfitrião: uma conexão, um pedido por vez.

use std::collections::VecDeque;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{Connector, MaybeTlsStream, WebSocketStream};

use crate::identity::{Pin, client_config};
use crate::protocol::{ClientMessage, Request, Response, ServerMessage, WireError};

const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const REPLY_TIMEOUT: Duration = Duration::from_secs(30);

pub struct Client {
    socket: WebSocketStream<MaybeTlsStream<TcpStream>>,
    next_id: u64,
    /// Avisos de mudança que chegaram enquanto uma resposta era esperada.
    backlog: VecDeque<i64>,
    /// A impressão digital do certificado que o servidor apresentou.
    pub fingerprint: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ClientError {
    #[error("Não foi possível falar com o Anfitrião.")]
    Unreachable,
    #[error("Este não é o Anfitrião com que este computador foi pareado.")]
    WrongHost,
    #[error("{}", .0.message)]
    Host(WireError),
}

impl ClientError {
    /// O código do erro que o Anfitrião devolveu, se foi ele quem recusou.
    pub fn code(&self) -> Option<&str> {
        match self {
            ClientError::Host(error) => Some(&error.code),
            _ => None,
        }
    }
}

impl Client {
    pub async fn connect(addr: SocketAddr, pin: Pin) -> Result<Client, ClientError> {
        let expected = match &pin {
            Pin::Fingerprint(fingerprint) => Some(fingerprint.clone()),
            Pin::TrustFirst => None,
        };
        let (config, seen) = client_config(pin).map_err(|_| ClientError::Unreachable)?;
        let connecting = tokio_tungstenite::connect_async_tls_with_config(
            format!("wss://{addr}/sync"),
            None,
            true,
            Some(Connector::Rustls(Arc::new(config))),
        );
        let connected = tokio::time::timeout(CONNECT_TIMEOUT, connecting)
            .await
            .map_err(|_| ClientError::Unreachable)?;
        let fingerprint = seen.lock().unwrap_or_else(|e| e.into_inner()).clone();
        match (connected, fingerprint) {
            (Ok((socket, _)), Some(fingerprint)) => Ok(Client {
                socket,
                next_id: 0,
                backlog: VecDeque::new(),
                fingerprint,
            }),
            // Alguém atendeu, mas com outro certificado.
            (Err(_), Some(presented))
                if expected.is_some_and(|expected| !expected.eq_ignore_ascii_case(&presented)) =>
            {
                Err(ClientError::WrongHost)
            }
            _ => Err(ClientError::Unreachable),
        }
    }

    pub async fn request(&mut self, request: Request) -> Result<Response, ClientError> {
        self.next_id += 1;
        let id = self.next_id;
        let json = serde_json::to_string(&ClientMessage { id, request })
            .map_err(|_| ClientError::Unreachable)?;
        self.socket
            .send(Message::Text(json.into()))
            .await
            .map_err(|_| ClientError::Unreachable)?;

        let reply = async {
            loop {
                match self.read().await? {
                    ServerMessage::Changed { seq } => self.backlog.push_back(seq),
                    ServerMessage::Reply { id: got, result } if got == id => {
                        return result.map_err(ClientError::Host);
                    }
                    ServerMessage::Reply { .. } => {}
                }
            }
        };
        tokio::time::timeout(REPLY_TIMEOUT, reply)
            .await
            .map_err(|_| ClientError::Unreachable)?
    }

    /// Espera o próximo aviso de que a residência mudou. Falha quando a conexão cai.
    pub async fn changed(&mut self) -> Result<i64, ClientError> {
        if let Some(seq) = self.backlog.pop_front() {
            return Ok(seq);
        }
        loop {
            if let ServerMessage::Changed { seq } = self.read().await? {
                return Ok(seq);
            }
        }
    }

    async fn read(&mut self) -> Result<ServerMessage, ClientError> {
        loop {
            match self.socket.next().await {
                Some(Ok(Message::Text(text))) => {
                    return serde_json::from_str(text.as_str())
                        .map_err(|_| ClientError::Unreachable);
                }
                Some(Ok(Message::Close(_))) | Some(Err(_)) | None => {
                    return Err(ClientError::Unreachable);
                }
                Some(Ok(_)) => {}
            }
        }
    }
}
