//! O servidor do Anfitrião: um WebSocket sobre TLS por onde as cópias pareiam,
//! sincronizam e pedem o que só ele executa.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::Response as HttpResponse;
use axum::routing::get;
use axum_server::Handle;
use axum_server::tls_rustls::RustlsConfig;
use finance_core::{Clock, Connection, Household, MemberId, PROTOCOL_VERSION};
use tokio::sync::{broadcast, watch};

use crate::identity::{Identity, IdentityError};
use crate::protocol::{
    ClientMessage, PROTOCOL_MISMATCH, Request, Response, ServerMessage, UNAUTHENTICATED, WireError,
};

/// De quanto em quanto tempo o servidor confere se a residência mudou, seja
/// por uma cópia, seja pela interface desta mesma máquina.
const WATCH_INTERVAL: Duration = Duration::from_millis(250);

pub struct HostServer {
    addr: SocketAddr,
    fingerprint: String,
    handle: Handle<SocketAddr>,
    connected: Arc<AtomicUsize>,
    watcher: tokio::task::JoinHandle<()>,
    stop: watch::Sender<bool>,
}

#[derive(Debug, thiserror::Error)]
pub enum HostError {
    #[error(transparent)]
    Core(#[from] finance_core::Error),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error("Não foi possível abrir a porta de rede: {0}")]
    Io(#[from] std::io::Error),
}

#[derive(Clone)]
struct Shared {
    household: Arc<Mutex<Household>>,
    changed: broadcast::Sender<i64>,
    connected: Arc<AtomicUsize>,
    /// Vira verdadeiro quando o servidor para; cada conexão aberta se encerra.
    stop: watch::Receiver<bool>,
}

impl HostServer {
    /// Passa a servir a residência do arquivo `db` em `bind`. `on_change` é
    /// chamado sempre que algo muda nela, venha de onde vier.
    pub async fn start(
        db: PathBuf,
        clock: Arc<dyn Clock>,
        bind: SocketAddr,
        on_change: impl Fn() + Send + Sync + 'static,
    ) -> Result<HostServer, HostError> {
        let conn = Connection::open(&db).map_err(finance_core::Error::from)?;
        let household = Household::open(conn, clock)?;
        let identity = Identity::load_or_create(&household)?;
        let fingerprint = identity.fingerprint();
        let tls = RustlsConfig::from_config(Arc::new(identity.server_config()?));

        let listener = std::net::TcpListener::bind(bind)?;
        listener.set_nonblocking(true)?;
        let addr = listener.local_addr()?;

        let (changed, _) = broadcast::channel(16);
        let (stop, stopped) = watch::channel(false);
        let connected = Arc::new(AtomicUsize::new(0));
        let mut last_seen = household.clock_value()?;
        let shared = Shared {
            household: Arc::new(Mutex::new(household)),
            changed,
            connected: connected.clone(),
            stop: stopped,
        };

        let watched = shared.clone();
        let watcher = tokio::spawn(async move {
            let mut ticker = tokio::time::interval(WATCH_INTERVAL);
            loop {
                ticker.tick().await;
                let now = lock(&watched.household).clock_value().unwrap_or(last_seen);
                if now != last_seen {
                    last_seen = now;
                    let _ = watched.changed.send(now);
                    on_change();
                }
            }
        });

        let app = Router::new()
            .route("/sync", get(upgrade))
            .with_state(shared);
        let handle = Handle::new();
        let server = axum_server::from_tcp_rustls(listener, tls)?.handle(handle.clone());
        tokio::spawn(async move {
            let _ = server.serve(app.into_make_service()).await;
        });

        Ok(HostServer {
            addr,
            fingerprint,
            handle,
            connected,
            watcher,
            stop,
        })
    }

    pub fn port(&self) -> u16 {
        self.addr.port()
    }

    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// Quantas cópias estão conectadas agora.
    pub fn connected(&self) -> usize {
        self.connected.load(Ordering::Relaxed)
    }

    pub fn shutdown(&self) {
        self.watcher.abort();
        let _ = self.stop.send(true);
        self.handle.shutdown();
    }
}

impl Drop for HostServer {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn lock(household: &Mutex<Household>) -> std::sync::MutexGuard<'_, Household> {
    household.lock().unwrap_or_else(|e| e.into_inner())
}

async fn upgrade(ws: WebSocketUpgrade, State(shared): State<Shared>) -> HttpResponse {
    ws.on_upgrade(|socket| async move {
        shared.connected.fetch_add(1, Ordering::Relaxed);
        serve(socket, &shared).await;
        shared.connected.fetch_sub(1, Ordering::Relaxed);
    })
}

/// O que o servidor sabe sobre a cópia do outro lado desta conexão.
#[derive(Default)]
struct Peer {
    compatible: bool,
    member: Option<MemberId>,
}

async fn serve(mut socket: WebSocket, shared: &Shared) {
    let mut changes = shared.changed.subscribe();
    let mut stop = shared.stop.clone();
    let mut peer = Peer::default();
    loop {
        let outgoing = tokio::select! {
            _ = stop.changed() => return,
            incoming = socket.recv() => {
                let Some(Ok(message)) = incoming else { return };
                let Message::Text(text) = message else { continue };
                let Ok(ClientMessage { id, request }) = serde_json::from_str(text.as_str()) else {
                    return;
                };
                let (result, next) = handle(shared.clone(), peer, request).await;
                peer = next;
                ServerMessage::Reply { id, result }
            }
            seq = changes.recv() => {
                let seq = match seq {
                    Ok(seq) => seq,
                    // Perder avisos não importa: o último já diz que há o que buscar.
                    Err(broadcast::error::RecvError::Lagged(_)) => 0,
                    Err(broadcast::error::RecvError::Closed) => return,
                };
                // Só quem já se identificou fica sabendo do que acontece na residência.
                if peer.member.is_none() {
                    continue;
                }
                ServerMessage::Changed { seq }
            }
        };
        let Ok(json) = serde_json::to_string(&outgoing) else {
            return;
        };
        if socket.send(Message::Text(json.into())).await.is_err() {
            return;
        }
    }
}

/// Atende um pedido fora do executor assíncrono, porque conferir senha e
/// gravar no banco bloqueiam.
async fn handle(
    shared: Shared,
    peer: Peer,
    request: Request,
) -> (Result<Response, WireError>, Peer) {
    tokio::task::spawn_blocking(move || {
        let mut peer = peer;
        let result = respond(&lock(&shared.household), &mut peer, request);
        (result, peer)
    })
    .await
    .unwrap_or_else(|_| {
        (
            Err(WireError {
                code: "internal".into(),
                message: "O Anfitrião falhou ao atender o pedido.".into(),
            }),
            Peer::default(),
        )
    })
}

fn respond(
    household: &Household,
    peer: &mut Peer,
    request: Request,
) -> Result<Response, WireError> {
    if let Request::Hello { protocol } = request {
        peer.compatible = protocol == PROTOCOL_VERSION;
        return Ok(Response::Hello {
            protocol: PROTOCOL_VERSION,
            household: household.info()?,
        });
    }
    if !peer.compatible {
        return Err(WireError {
            code: PROTOCOL_MISMATCH.into(),
            message: "Atualize o app para sincronizar com o Anfitrião.".into(),
        });
    }

    match request {
        Request::Hello { .. } => unreachable!("tratado acima"),
        Request::Join {
            code,
            username,
            password,
        } => {
            let member = household.accept_invite(&code, &username, &password)?;
            let token = household.issue_token(member.id)?;
            peer.member = Some(member.id);
            Ok(Response::Session { token, member })
        }
        Request::Login { username, password } => {
            let member = household.authenticate(&username, &password)?;
            let token = household.issue_token(member.id)?;
            peer.member = Some(member.id);
            Ok(Response::Session { token, member })
        }
        Request::Resume { token } => {
            let member = household.authenticate_token(&token)?;
            peer.member = Some(member.id);
            Ok(Response::Resumed { member })
        }
        Request::Pull { since } => {
            let actor = authenticated(household, peer)?;
            let _ = actor;
            Ok(Response::Changes {
                changes: household.changes_since(since)?,
            })
        }
        Request::Push { changes } => {
            let actor = authenticated(household, peer)?;
            Ok(Response::Pushed {
                outcomes: household.apply_push(actor, &changes)?,
            })
        }
        Request::Command { command } => {
            let actor = authenticated(household, peer)?;
            Ok(Response::Done {
                value: household.execute(actor, command)?,
            })
        }
    }
}

/// O membro desta conexão, conferido de novo a cada pedido: quem foi removido
/// da residência perde o acesso na hora, mesmo com a conexão aberta.
fn authenticated(household: &Household, peer: &Peer) -> Result<MemberId, WireError> {
    let unauthenticated = || WireError {
        code: UNAUTHENTICATED.into(),
        message: "Sessão inválida. Entre de novo.".into(),
    };
    let member = peer.member.ok_or_else(unauthenticated)?;
    match household.member(member) {
        Ok(found) if found.active => Ok(member),
        _ => Err(unauthenticated()),
    }
}
