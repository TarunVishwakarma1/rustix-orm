use std::fmt;

/// Represents the various errors that can occur in the Rustix ORM.
#[derive(Debug)]
pub enum RusticxError {
    /// Represents a connection error with a message detailing the issue.
    ConnectionError(String),
    
    /// Represents a query execution error with a message detailing the issue.
    QueryError(String),
    
    /// Represents a transaction error with a message detailing the issue.
    TransactionError(String),
    
    /// Represents a serialization error with a message detailing the issue.
    SerializationError(String),
    
    /// Represents a validation error with a message detailing the issue.
    ValidationError(String),
    
    /// Represents an error when a requested item is not found.
    NotFound(String),
    
    /// Represents an error when an invalid column is specified.
    InvalidColumn(String),
    
    /// Represents a general database error with a message detailing the issue.
    DatabaseError(String),
    
    /// Represents an error when a requested feature is not enabled.
    FeatureNotEnabled(String),
    
    /// Represents an error during deserialization with a message detailing the issue.
    DeserializationError(String),
}

impl fmt::Display for RusticxError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RusticxError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            RusticxError::QueryError(msg) => write!(f, "Query error: {}", msg),
            RusticxError::TransactionError(msg) => write!(f, "Transaction error: {}", msg),
            RusticxError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            RusticxError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            RusticxError::NotFound(msg) => write!(f, "Not found: {}", msg),
            RusticxError::InvalidColumn(msg) => write!(f, "Invalid column: {}", msg),
            RusticxError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
            RusticxError::FeatureNotEnabled(msg) => write!(f, "Feature not enabled: {}", msg),
            RusticxError::DeserializationError(msg) => write!(f, "Deserialization error: {}", msg),
        }
    }
}

impl std::error::Error for RusticxError {}

// Conversions from other error types to RustixError

#[cfg(feature = "postgres")]
impl From<tokio_postgres::Error> for RusticxError {
    fn from(err: tokio_postgres::Error) -> Self {
        RusticxError::QueryError(err.to_string())
    }
}

#[cfg(feature = "mysql")]
impl From<mysql::Error> for RusticxError {
    fn from(err: mysql::Error) -> Self {
        RusticxError::QueryError(err.to_string())
    }
}

#[cfg(feature = "rusqlite")]
impl From<rusqlite::Error> for RusticxError {
    fn from(err: rusqlite::Error) -> Self {
        RusticxError::QueryError(err.to_string())
    }
}

impl From<serde_json::Error> for RusticxError {
    fn from(err: serde_json::Error) -> Self {
        RusticxError::SerializationError(err.to_string())
    }
}
