pub type Result<T> = std::result::Result<T, Error>;

/// As mensagens são exibidas ao usuário como estão.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("Usuário ou senha inválidos.")]
    InvalidCredentials,
    #[error("Você não tem permissão para fazer isso.")]
    Forbidden,
    #[error("Registro não encontrado.")]
    NotFound,
    #[error("{0}")]
    Invalid(String),
    #[error("Este nome de usuário já está em uso nesta residência.")]
    UsernameTaken,
    #[error("Convite inválido.")]
    InviteInvalid,
    #[error("Este convite expirou.")]
    InviteExpired,
    #[error("Este convite já foi usado.")]
    InviteUsed,
    #[error("Esta ação precisa de conexão com o Anfitrião.")]
    NeedsHost,
    #[error("A residência precisa ter pelo menos um administrador.")]
    LastAdmin,
    #[error("Sessão inválida. Entre de novo.")]
    InvalidSession,
    #[error("Este arquivo foi criado por uma versão mais nova do app.")]
    NewerSchema,
    #[error("Erro no banco de dados: {0}")]
    Db(#[from] rusqlite::Error),
    #[error("Erro ao processar a senha: {0}")]
    PasswordHash(String),
}

impl Error {
    /// Identificador estável do erro, para quem precisa distingui-los sem ler a mensagem.
    pub fn code(&self) -> &'static str {
        match self {
            Error::InvalidCredentials => "invalid_credentials",
            Error::Forbidden => "forbidden",
            Error::NotFound => "not_found",
            Error::Invalid(_) => "invalid",
            Error::UsernameTaken => "username_taken",
            Error::InviteInvalid => "invite_invalid",
            Error::InviteExpired => "invite_expired",
            Error::InviteUsed => "invite_used",
            Error::NeedsHost => "needs_host",
            Error::LastAdmin => "last_admin",
            Error::InvalidSession => "invalid_session",
            Error::NewerSchema => "newer_schema",
            Error::Db(_) => "db",
            Error::PasswordHash(_) => "password_hash",
        }
    }

    pub(crate) fn invalid(message: &str) -> Self {
        Error::Invalid(message.to_owned())
    }
}
