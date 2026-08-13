use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    #[error("configuration error: {0}")]
    Config(String),

    #[error("serialization error: {0}")]
    Serialization(#[from] toml::ser::Error),

    #[error("deserialization error: {0}")]
    Deserialization(#[from] toml::de::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("dat parsing error: {0}")]
    DatParsing(String),

    #[error("scan error: {0}")]
    Scan(String),

    #[error("rule engine error: {0}")]
    RuleEngine(String),

    #[error("file operation error: {0}")]
    FileOperation(String),

    #[error("update error: {0}")]
    Update(String),

    #[error("plugin error: {0}")]
    Plugin(String),

    #[error("{0}")]
    Other(#[from] anyhow::Error),
}

pub type AppResult<T> = Result<T, AppError>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Severity {
    Info,
    Warning,
    Error,
    Critical,
}

impl AppError {
    pub fn severity(&self) -> Severity {
        match self {
            AppError::Io(_) => Severity::Error,
            AppError::Config(_) => Severity::Warning,
            AppError::Serialization(_) | AppError::Deserialization(_) | AppError::Json(_) => {
                Severity::Error
            }
            AppError::DatParsing(_) => Severity::Error,
            AppError::Scan(_) => Severity::Warning,
            AppError::RuleEngine(_) => Severity::Error,
            AppError::FileOperation(_) => Severity::Critical,
            AppError::Update(_) => Severity::Warning,
            AppError::Plugin(_) => Severity::Warning,
            AppError::Other(_) => Severity::Error,
        }
    }

    pub fn user_message(&self) -> String {
        self.to_string()
    }
}
