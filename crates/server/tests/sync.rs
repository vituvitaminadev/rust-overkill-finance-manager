//! Um Anfitrião e suas cópias conversando de verdade, por TLS, na própria máquina.

use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use chrono::Local;
use finance_core::{
    Connection, Entry, EntryInput, EntryKind, HostCommand, Household, Member, NewCategory,
    NewHousehold, NewUser, SyncStatus, SystemClock,
};
use finance_server::{
    ClientError, HostServer, LinkConfig, LinkError, LinkState, PairingError, ReplicaLink, inspect,
    join, sign_in,
};

const PASSWORD: &str = "senha-secreta";

/// Uma casa: o Anfitrião no ar e a conexão que a interface dele usaria.
struct Home {
    dir: PathBuf,
    ui: Household,
    admin: Member,
    server: Option<HostServer>,
    addr: SocketAddr,
    /// Onde as cópias acham o Anfitrião; `None` simula ele fora da rede.
    reachable: Arc<Mutex<Option<SocketAddr>>>,
}

/// Um computador pareado: a conexão da interface dele e a ligação com o Anfitrião.
struct Computer {
    db: PathBuf,
    ui: Household,
    member: Member,
    link: ReplicaLink,
}

fn open(db: &Path) -> Household {
    Household::open(Connection::open(db).unwrap(), Arc::new(SystemClock)).unwrap()
}

async fn serve(db: PathBuf, bind: SocketAddr) -> HostServer {
    // Logo depois de parar, a porta pode levar um instante para ficar livre.
    for _ in 0..100 {
        match HostServer::start(db.clone(), Arc::new(SystemClock), bind, || {}).await {
            Ok(server) => return server,
            Err(_) => tokio::time::sleep(Duration::from_millis(50)).await,
        }
    }
    panic!("o Anfitrião não conseguiu abrir a porta {bind}");
}

async fn home() -> Home {
    let dir = std::env::temp_dir().join(format!("finance-server-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    let db = dir.join("host.db");
    let (ui, admin) = Household::create(
        Connection::open(&db).unwrap(),
        Arc::new(SystemClock),
        NewHousehold {
            name: "Casa".into(),
            user: NewUser::New {
                username: "vitu".into(),
                password: PASSWORD.into(),
            },
        },
    )
    .unwrap();
    let server = serve(db, "127.0.0.1:0".parse().unwrap()).await;
    let addr = SocketAddr::from(([127, 0, 0, 1], server.port()));
    Home {
        dir,
        ui,
        admin,
        server: Some(server),
        addr,
        reachable: Arc::new(Mutex::new(Some(addr))),
    }
}

impl Home {
    fn fingerprint(&self) -> String {
        self.server.as_ref().unwrap().fingerprint().to_owned()
    }

    /// Um computador novo entra na residência com um convite e começa a sincronizar.
    async fn pair(&self, username: &str) -> Computer {
        let invite = self.ui.create_invite(self.admin.id).unwrap();
        let db = self.dir.join(format!("{username}.db"));
        let member = join(
            self.addr,
            &self.fingerprint(),
            &invite.code,
            username,
            PASSWORD,
            &db,
            Arc::new(SystemClock),
        )
        .await
        .unwrap();
        let link = self.link(&db, &member, finance_core::PROTOCOL_VERSION);
        Computer {
            ui: open(&db),
            db,
            member,
            link,
        }
    }

    fn link(&self, db: &Path, member: &Member, protocol: u32) -> ReplicaLink {
        let reachable = self.reachable.clone();
        let mut config = LinkConfig::new(
            db.to_path_buf(),
            Arc::new(SystemClock),
            member.id,
            Arc::new(move || *reachable.lock().unwrap()),
        );
        config.retry_every = Duration::from_millis(50);
        config.protocol = protocol;
        ReplicaLink::start(config)
    }

    /// O computador anfitrião é desligado.
    fn go_dark(&mut self) {
        self.server.take().unwrap().shutdown();
        *self.reachable.lock().unwrap() = None;
    }

    /// E ligado de novo, no mesmo endereço.
    async fn come_back(&mut self) {
        self.server = Some(serve(self.dir.join("host.db"), self.addr).await);
        *self.reachable.lock().unwrap() = Some(self.addr);
    }

    fn category(&self, name: &str) -> uuid::Uuid {
        self.ui
            .categories(self.admin.id)
            .unwrap()
            .into_iter()
            .find(|category| category.name == name)
            .unwrap()
            .id
    }

    fn expense(&self, amount_cents: i64, description: &str) -> EntryInput {
        EntryInput {
            kind: EntryKind::Expense,
            amount_cents,
            date: Local::now().date_naive(),
            description: description.into(),
            category_id: self.category("Mercado"),
            assignee_id: None,
            status: None,
        }
    }
}

impl Drop for Home {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Computer {
    fn entries(&self) -> Vec<Entry> {
        self.ui
            .entries(self.member.id, &Default::default())
            .unwrap()
    }

    fn descriptions(&self) -> Vec<String> {
        let mut descriptions: Vec<String> =
            self.entries().into_iter().map(|e| e.description).collect();
        descriptions.sort();
        descriptions
    }

    fn status(&self) -> SyncStatus {
        self.ui.sync_status().unwrap()
    }

    /// A interface grava localmente e avisa a ligação, como o app faz.
    fn add(&self, input: EntryInput) -> Entry {
        let entry = self.ui.create_entry(self.member.id, input).unwrap();
        self.link.poke();
        entry
    }

    fn edit(&self, id: uuid::Uuid, input: EntryInput) {
        self.ui.update_entry(self.member.id, id, input).unwrap();
        self.link.poke();
    }
}

/// Espera, por alguns segundos, que a sincronização chegue ao estado descrito.
async fn eventually(what: &str, mut reached: impl FnMut() -> bool) {
    for _ in 0..400 {
        if reached() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("não aconteceu a tempo: {what}");
}

fn host_descriptions(home: &Home) -> Vec<String> {
    let mut descriptions: Vec<String> = home
        .ui
        .entries(home.admin.id, &Default::default())
        .unwrap()
        .into_iter()
        .map(|e| e.description)
        .collect();
    descriptions.sort();
    descriptions
}

#[tokio::test(flavor = "multi_thread")]
async fn host_identifies_itself_before_any_credentials() {
    let home = home().await;

    let info = inspect(home.addr).await.unwrap();

    assert_eq!(info.household.name, "Casa");
    assert_eq!(info.fingerprint, home.fingerprint());
    assert_eq!(finance_server::security_code(&info.fingerprint).len(), 9);
}

#[tokio::test(flavor = "multi_thread")]
async fn pairing_with_an_invite_brings_a_full_copy() {
    let home = home().await;
    home.ui
        .create_entry(home.admin.id, home.expense(8_450, "feira"))
        .unwrap();

    let ana = home.pair("ana").await;

    assert!(ana.ui.is_replica());
    assert_eq!(ana.ui.info().unwrap().name, "Casa");
    assert_eq!(ana.descriptions(), vec!["feira"]);
    assert_eq!(ana.ui.authenticate("ana", PASSWORD).unwrap(), ana.member);
    assert_eq!(ana.ui.members(ana.member.id).unwrap().len(), 2);
    eventually("a cópia fica online", || {
        ana.link.state() == LinkState::Online
    })
    .await;
    eventually("o Anfitrião conta a cópia conectada", || {
        home.server.as_ref().unwrap().connected() == 1
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn nobody_joins_without_a_valid_invite() {
    let home = home().await;
    let db = home.dir.join("intruso.db");

    let result = join(
        home.addr,
        &home.fingerprint(),
        "NAOEXISTE",
        "intruso",
        PASSWORD,
        &db,
        Arc::new(SystemClock),
    )
    .await;

    assert!(matches!(&result, Err(error) if error.code() == "invite_invalid"));
    assert!(!db.exists());
    assert_eq!(home.ui.members(home.admin.id).unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn replica_refuses_a_server_that_is_not_its_host() {
    let home = home().await;
    let impostor = "0".repeat(64);
    let invite = home.ui.create_invite(home.admin.id).unwrap();

    let result = join(
        home.addr,
        &impostor,
        &invite.code,
        "ana",
        PASSWORD,
        &home.dir.join("ana.db"),
        Arc::new(SystemClock),
    )
    .await;

    assert!(matches!(
        result,
        Err(PairingError::Client(ClientError::WrongHost))
    ));
    // O convite não chegou a ser enviado, então continua valendo.
    assert_eq!(home.ui.invites(home.admin.id).unwrap().len(), 1);
}

#[tokio::test(flavor = "multi_thread")]
async fn entry_added_on_one_computer_shows_up_on_the_others() {
    let home = home().await;
    let ana = home.pair("ana").await;
    let bia = home.pair("bia").await;

    ana.add(home.expense(8_450, "feira"));

    eventually("chega ao Anfitrião", || {
        host_descriptions(&home) == ["feira"]
    })
    .await;
    eventually("chega ao outro computador", || {
        bia.descriptions() == ["feira"]
    })
    .await;
    eventually("deixa de estar pendente", || ana.status().pending == 0).await;
    assert_eq!(bia.entries()[0].assignee_name, "ana");
}

#[tokio::test(flavor = "multi_thread")]
async fn change_made_on_the_host_computer_reaches_the_replicas() {
    let home = home().await;
    let ana = home.pair("ana").await;
    eventually("online", || ana.link.state() == LinkState::Online).await;

    home.ui
        .create_entry(home.admin.id, home.expense(150_000, "aluguel"))
        .unwrap();

    eventually("chega à cópia", || ana.descriptions() == ["aluguel"]).await;
}

#[tokio::test(flavor = "multi_thread")]
async fn offline_changes_wait_and_are_sent_when_the_host_returns() {
    let mut home = home().await;
    let ana = home.pair("ana").await;
    eventually("online", || ana.link.state() == LinkState::Online).await;

    home.go_dark();
    eventually("percebe que está offline", || {
        ana.link.state() == LinkState::Offline
    })
    .await;
    ana.add(home.expense(8_450, "feira"));
    ana.add(home.expense(2_000, "padaria"));
    assert_eq!(ana.status().pending, 2);
    assert!(host_descriptions(&home).is_empty());

    home.come_back().await;

    eventually("o Anfitrião recebe tudo", || {
        host_descriptions(&home) == ["feira", "padaria"]
    })
    .await;
    eventually("nada mais pendente", || ana.status().pending == 0).await;
    assert_eq!(ana.link.state(), LinkState::Online);
}

#[tokio::test(flavor = "multi_thread")]
async fn conflicting_offline_edit_waits_for_a_choice() {
    let mut home = home().await;
    let entry = home
        .ui
        .create_entry(home.admin.id, home.expense(10_000, "mercado"))
        .unwrap();
    // Um segundo computador do próprio administrador, que pode editar o lançamento.
    let invite_free_db = home.dir.join("notebook.db");
    let invite = home.ui.create_invite(home.admin.id).unwrap();
    join(
        home.addr,
        &home.fingerprint(),
        &invite.code,
        "convidado",
        PASSWORD,
        &invite_free_db,
        Arc::new(SystemClock),
    )
    .await
    .unwrap();
    let admin_here = sign_in(
        home.addr,
        &invite_free_db,
        Arc::new(SystemClock),
        "vitu",
        PASSWORD,
    )
    .await
    .unwrap();
    assert_eq!(admin_here, home.admin);
    let notebook = Computer {
        ui: open(&invite_free_db),
        link: home.link(&invite_free_db, &home.admin, finance_core::PROTOCOL_VERSION),
        db: invite_free_db,
        member: home.admin.clone(),
    };
    eventually("online", || notebook.link.state() == LinkState::Online).await;

    home.go_dark();
    eventually("offline", || notebook.link.state() == LinkState::Offline).await;
    notebook.edit(entry.id, home.expense(12_000, "mercado, valor certo"));
    home.ui
        .update_entry(
            home.admin.id,
            entry.id,
            home.expense(9_000, "mercado e feira"),
        )
        .unwrap();
    home.come_back().await;

    eventually("o conflito aparece", || notebook.status().conflicts == 1).await;
    assert_eq!(host_descriptions(&home), vec!["mercado e feira"]);
    assert_eq!(notebook.descriptions(), vec!["mercado, valor certo"]);
    let conflict = &notebook.ui.conflicts(notebook.member.id).unwrap()[0];
    assert_eq!(conflict.mine.as_ref().unwrap().amount_cents, 12_000);
    assert_eq!(conflict.theirs.as_ref().unwrap().amount_cents, 9_000);

    notebook
        .ui
        .resolve_conflict(notebook.member.id, entry.id, true)
        .unwrap();
    notebook.link.poke();

    eventually("a versão escolhida vai para o Anfitrião", || {
        host_descriptions(&home) == ["mercado, valor certo"]
    })
    .await;
    eventually("sem conflitos nem pendências", || {
        notebook.status()
            == SyncStatus {
                pending: 0,
                conflicts: 0,
            }
    })
    .await;
    let _ = &notebook.db;
}

#[tokio::test(flavor = "multi_thread")]
async fn host_only_commands_travel_through_the_link() {
    let mut home = home().await;
    let ana = home.pair("ana").await;
    home.ui
        .set_member_role(home.admin.id, ana.member.id, finance_core::Role::Admin)
        .unwrap();
    eventually("online", || ana.link.state() == LinkState::Online).await;
    let pets = || {
        HostCommand::CreateCategory(NewCategory {
            name: "Pets".into(),
            kind: EntryKind::Expense,
        })
    };

    let created = ana.link.command(pets()).await.unwrap();

    // Quando o comando responde, o resultado já está na cópia local.
    assert_eq!(created["name"], "Pets");
    let has_pets = |household: &Household, member: &Member| {
        household
            .categories(member.id)
            .unwrap()
            .iter()
            .any(|category| category.name == "Pets")
    };
    assert!(has_pets(&ana.ui, &ana.member));
    assert!(has_pets(&home.ui, &home.admin));

    // O que o Anfitrião recusa volta com o motivo dele.
    let duplicate = ana.link.command(pets()).await;
    assert!(matches!(&duplicate, Err(LinkError::Host { code, .. }) if code == "invalid"));

    home.go_dark();
    eventually("offline", || ana.link.state() == LinkState::Offline).await;
    assert!(matches!(
        ana.link.command(pets()).await,
        Err(LinkError::Offline)
    ));
}

#[tokio::test(flavor = "multi_thread")]
async fn different_protocol_versions_do_not_sync() {
    let home = home().await;
    let ana = home.pair("ana").await;
    let outdated = home.link(&ana.db, &ana.member, finance_core::PROTOCOL_VERSION + 1);

    eventually("a diferença de versão é percebida", || {
        outdated.state() == LinkState::ProtocolMismatch
    })
    .await;
}

#[tokio::test(flavor = "multi_thread")]
async fn removed_member_stops_syncing() {
    let home = home().await;
    let ana = home.pair("ana").await;
    eventually("online", || ana.link.state() == LinkState::Online).await;

    home.ui.remove_member(home.admin.id, ana.member.id).unwrap();

    eventually("perde a autorização", || {
        ana.link.state() == LinkState::Unauthorized
    })
    .await;
    ana.add(home.expense(1, "não deveria chegar"));
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(host_descriptions(&home).is_empty());
}
