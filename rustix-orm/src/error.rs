use std::fmt;

/// Represents the various errors that can occur in the Rustix ORM.
#[derive(Debug)]
pub enum RustixError {
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
            RustixError::DeserializationError(msg) => write!(f, "Deserialization error: {}", msg),
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
