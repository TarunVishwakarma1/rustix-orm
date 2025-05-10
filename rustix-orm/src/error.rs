use std::fmt;

#[derive(Debug)]
pub enum RustixError {
    ConnectionError(String),
    QueryError(String),
    TransactionError(String),
    SerializationError(String),
    ValidationError(String),
    NotFound(String),
    InvalidColumn(String),
    DatabaseError(String),
    FeatureNotEnabled(String),
    DeserializationError(String), // Add this line
}

impl fmt::Display for RustixError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RustixError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            RustixError::QueryError(msg) => write!(f, "Query error: {}", msg),
            RustixError::TransactionError(msg) => write!(f, "Transaction error: {}", msg),
            RustixError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            RustixError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            RustixError::NotFound(msg) => write!(f, "Not found: {}", msg),
            RustixError::InvalidColumn(msg) => write!(f, "Invalid column: {}", msg),
            RustixError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            RustixError::FeatureNotEnabled(msg) => write!(f, "Feature not enabled: {}", msg),
            RustixError::DeserializationError(msg) => write!(f, "Deserialization error: {}", msg), // Add this line
        }
    }
}

impl std::error::Error for RustixError {}

// Conversions from other error types to RustixError

#[cfg(feature = "postgres")]
impl From<tokio_postgres::Error> for RustixError {
    fn from(err: tokio_postgres::Error) -> Self {
        RustixError::QueryError(err.to_string())
    }
}

#[cfg(feature = "mysql")]
impl From<mysql::Error> for RustixError {
    fn from(err: mysql::Error) -> Self {
        RustixError::QueryError(err.to_string())
    }
}

#[cfg(feature = "rusqlite")]
impl From<rusqlite::Error> for RustixError {
    fn from(err: rusqlite::Error) -> Self {
        RustixError::QueryError(err.to_string())
    }
}

impl From<serde_json::Error> for RustixError {
    fn from(err: serde_json::Error) -> Self {
        RustixError::SerializationError(err.to_string())
    }
}
