//! O Anfitrião de uma residência e a ligação das cópias com ele.
//!
//! [`HostServer`] serve o banco de uma residência na rede local, por
//! WebSocket sobre TLS. [`ReplicaLink`] mantém uma cópia em dia: envia as
//! alterações locais, recebe as do Anfitrião e repassa a ele o que só ele
//! pode executar. As regras de sincronização ficam em `finance-core`.

mod client;
mod host;
mod identity;
mod link;
mod pairing;
mod protocol;

pub use client::ClientError;
pub use host::HostServer;
pub use identity::security_code;
pub use link::{LinkConfig, LinkError, LinkEvent, LinkState, ReplicaLink};
pub use pairing::{HostInfo, PairingError, inspect, join, sign_in};
