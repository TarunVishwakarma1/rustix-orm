use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub enum RustixError {
    ConnectionError(String),
    QueryError(String),
    ValidationError(String),
    NotFound(String),
    MigrationError(String),
    SerializationError(String),
    DatabaseError(String),
}

impl fmt::Display for RustixError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            RustixError::ConnectionError(msg) => write!(f, "Connection error: {}", msg),
            RustixError::QueryError(msg) => write!(f, "Query error: {}", msg),
            RustixError::ValidationError(msg) => write!(f, "Validation error: {}", msg),
            RustixError::NotFound(msg) => write!(f, "Not found: {}", msg),
            RustixError::MigrationError(msg) => write!(f, "Migration error: {}", msg),
            RustixError::SerializationError(msg) => write!(f, "Serialization error: {}", msg),
            RustixError::DatabaseError(msg) => write!(f, "Database error: {}", msg),
        }
    }
}

impl Error for RustixError {}