use serde::Serialize;

/// O erro que chega ao frontend: um código estável e a mensagem a exibir.
#[derive(Debug, Serialize)]
pub struct CommandError {
    code: String,
    message: String,
}

impl CommandError {
    pub fn new(code: &str, message: &str) -> Self {
        CommandError {
            code: code.to_owned(),
            message: message.to_owned(),
        }
    }
}

impl From<finance_core::Error> for CommandError {
    fn from(error: finance_core::Error) -> Self {
        CommandError::new(error.code(), &error.to_string())
    }
}
