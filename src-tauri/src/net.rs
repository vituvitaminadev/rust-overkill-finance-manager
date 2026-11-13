//! A presença deste computador na rede de casa: os Anfitriões que ele hospeda,
//! a ligação da cópia aberta com o Anfitrião dela e a descoberta de um e outro.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use finance_core::{Connection, Household, HouseholdId, MemberId, SystemClock};
use finance_server::{HostServer, LinkConfig, LinkEvent, LinkState, ReplicaLink};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use serde::Serialize;
use tauri::{AppHandle, Emitter};

/// O serviço que os Anfitriões anunciam na rede local.
const SERVICE: &str = "_finmgr._tcp.local.";
const PORT_KEY: &str = "host_port";

/// Algo mudou nos dados da residência aberta: vale reler tudo.
pub const HOUSEHOLD_CHANGED: &str = "household-changed";
/// O estado da sincronização mudou (online, offline, cópias conectadas).
pub const SYNC_CHANGED: &str = "sync-changed";

/// Um Anfitrião encontrado na rede.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Discovered {
    pub household_id: HouseholdId,
    pub name: String,
    pub address: SocketAddr,
    #[serde(skip)]
    fullname: String,
}

pub struct Network {
    app: AppHandle,
    hosts: Mutex<HashMap<HouseholdId, HostServer>>,
    link: Mutex<Option<ActiveLink>>,
    /// Ausente quando o sistema não deixa usar mDNS; aí só o endereço digitado funciona.
    mdns: Option<ServiceDaemon>,
    discovered: Arc<Mutex<HashMap<HouseholdId, Discovered>>>,
}

struct ActiveLink {
    household: HouseholdId,
    member: MemberId,
    link: Arc<ReplicaLink>,
}

pub struct HostStatus {
    pub port: u16,
    pub fingerprint: String,
    pub connected: usize,
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

impl Network {
    pub fn new(app: AppHandle) -> Network {
        let discovered = Arc::new(Mutex::new(HashMap::new()));
        let mdns = ServiceDaemon::new().ok();
        if let Some(receiver) = mdns.as_ref().and_then(|daemon| daemon.browse(SERVICE).ok()) {
            let discovered = discovered.clone();
            std::thread::spawn(move || {
                while let Ok(event) = receiver.recv() {
                    match event {
                        ServiceEvent::ServiceResolved(service) => {
                            let id = service
                                .get_property_val_str("id")
                                .and_then(|id| id.parse().ok());
                            let ip = service.get_addresses_v4().into_iter().next();
                            if let (Some(id), Some(ip)) = (id, ip) {
                                let found = Discovered {
                                    household_id: id,
                                    name: service
                                        .get_property_val_str("name")
                                        .unwrap_or_default()
                                        .to_owned(),
                                    address: SocketAddr::new(IpAddr::V4(ip), service.port),
                                    fullname: service.fullname.clone(),
                                };
                                lock(&discovered).insert(id, found);
                            }
                        }
                        ServiceEvent::ServiceRemoved(_, fullname) => {
                            lock(&discovered).retain(|_, found| found.fullname != fullname);
                        }
                        _ => {}
                    }
                }
            });
        }
        Network {
            app,
            hosts: Mutex::new(HashMap::new()),
            link: Mutex::new(None),
            mdns,
            discovered,
        }
    }

    /// Passa a servir na rede a residência do arquivo `path`, que foi criada
    /// nesta máquina. Não faz nada se ela já está sendo servida.
    pub async fn host(&self, path: &Path) -> Result<(), String> {
        let (info, preferred) = {
            let household = open(path)?;
            let preferred = household
                .local(PORT_KEY)
                .ok()
                .flatten()
                .and_then(|port| port.parse::<u16>().ok());
            (household.info().map_err(|e| e.to_string())?, preferred)
        };
        if lock(&self.hosts).contains_key(&info.id) {
            return Ok(());
        }

        // A mesma porta de sempre, para que uma cópia que só lembra o último
        // endereço ainda encontre o Anfitrião; se estiver ocupada, qualquer uma.
        let mut server = None;
        for port in preferred.into_iter().chain([0]) {
            let app = self.app.clone();
            let started = HostServer::start(
                path.to_path_buf(),
                Arc::new(SystemClock),
                SocketAddr::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port),
                move || {
                    let _ = app.emit(HOUSEHOLD_CHANGED, ());
                },
            )
            .await;
            match started {
                Ok(started) => {
                    server = Some(started);
                    break;
                }
                Err(error) if port == 0 => return Err(error.to_string()),
                Err(_) => {}
            }
        }
        let Some(server) = server else {
            return Err("Não foi possível abrir uma porta de rede.".into());
        };
        let _ = open(path)?.set_local(PORT_KEY, &server.port().to_string());

        if let Some(daemon) = &self.mdns {
            let id = info.id.to_string();
            let properties = [("id", id.as_str()), ("name", info.name.as_str())];
            let service = ServiceInfo::new(
                SERVICE,
                &id,
                &format!("{id}.local."),
                "",
                server.port(),
                &properties[..],
            )
            .map(ServiceInfo::enable_addr_auto);
            if let Ok(service) = service {
                let _ = daemon.register(service);
            }
        }
        lock(&self.hosts).insert(info.id, server);
        Ok(())
    }

    pub fn host_status(&self, household: HouseholdId) -> Option<HostStatus> {
        lock(&self.hosts).get(&household).map(|server| HostStatus {
            port: server.port(),
            fingerprint: server.fingerprint().to_owned(),
            connected: server.connected(),
        })
    }

    /// Os Anfitriões vistos na rede, fora os que esta própria máquina hospeda.
    pub fn discovered(&self) -> Vec<Discovered> {
        let hosts = lock(&self.hosts);
        let mut found: Vec<Discovered> = lock(&self.discovered)
            .values()
            .filter(|found| !hosts.contains_key(&found.household_id))
            .cloned()
            .collect();
        found.sort_by(|a, b| a.name.cmp(&b.name));
        found
    }

    /// Onde o Anfitrião da residência está agora: o que a rede anuncia ou,
    /// na falta disso, o último endereço que funcionou.
    pub fn locate(
        &self,
        household: HouseholdId,
        fallback: Option<SocketAddr>,
    ) -> Option<SocketAddr> {
        lock(&self.discovered)
            .get(&household)
            .map(|found| found.address)
            .or(fallback)
    }

    /// Mantém a cópia em `path` sincronizada em nome de `member`. Chamar de
    /// novo para a mesma residência e o mesmo membro não reinicia nada.
    pub fn start_link(
        &self,
        path: PathBuf,
        household: HouseholdId,
        member: MemberId,
        fallback: Option<SocketAddr>,
    ) {
        let mut active = lock(&self.link);
        if active
            .as_ref()
            .is_some_and(|link| link.household == household && link.member == member)
        {
            return;
        }
        let discovered = self.discovered.clone();
        let mut config = LinkConfig::new(
            path,
            Arc::new(SystemClock),
            member,
            Arc::new(move || {
                lock(&discovered)
                    .get(&household)
                    .map(|found| found.address)
                    .or(fallback)
            }),
        );
        let app = self.app.clone();
        config.on_event = Arc::new(move |event| {
            let name = match event {
                LinkEvent::Synced => HOUSEHOLD_CHANGED,
                LinkEvent::State(_) => SYNC_CHANGED,
            };
            let _ = app.emit(name, ());
        });
        *active = Some(ActiveLink {
            household,
            member,
            link: Arc::new(ReplicaLink::start(config)),
        });
    }

    pub fn stop_link(&self) {
        *lock(&self.link) = None;
    }

    pub fn link(&self) -> Option<Arc<ReplicaLink>> {
        lock(&self.link).as_ref().map(|active| active.link.clone())
    }

    /// Avisa a ligação de que há alteração local a enviar.
    pub fn poke(&self) {
        if let Some(link) = self.link() {
            link.poke();
        }
    }

    pub fn link_state(&self) -> Option<LinkState> {
        self.link().map(|link| link.state())
    }
}

fn open(path: &Path) -> Result<Household, String> {
    let conn = Connection::open(path).map_err(|e| e.to_string())?;
    Household::open(conn, Arc::new(SystemClock)).map_err(|e| e.to_string())
}

/// O endereço deste computador na rede local, para quem precisar digitá-lo
/// em outro. Nenhum pacote é enviado: o sistema só escolhe a interface de saída.
pub fn local_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind("0.0.0.0:0").ok()?;
    socket.connect("192.0.2.1:9").ok()?;
    Some(socket.local_addr().ok()?.ip())
}
