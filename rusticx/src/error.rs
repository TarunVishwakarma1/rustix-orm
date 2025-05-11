use std::fmt;

/// Represents the various errors that can occur in the Rusticx ORM.
///
/// This enum encapsulates different types of errors that might arise
/// during database operations, serialization/deserialization, validation,
/// or connection management.
#[derive(Debug)]
pub enum RusticxError {
    /// Represents a connection error with a message detailing the issue.
    ///
    /// This error typically occurs when establishing or managing connections
    /// to the database.
    ConnectionError(String),

    /// Represents a query execution error with a message detailing the issue.
    ///
    /// This error occurs when a SQL query fails to execute successfully
    /// on the database.
    QueryError(String),

    /// Represents a transaction error with a message detailing the issue.
    ///
    /// This error covers failures during the lifecycle of a database transaction,
    /// such as starting, committing, or rolling back.
    TransactionError(String),

    /// Represents a serialization error with a message detailing the issue.
    ///
    /// This error occurs when converting Rust data structures into a format
    /// suitable for the database (e.g., JSON, specific database types).
    SerializationError(String),

    /// Represents a validation error with a message detailing the issue.
    ///
    /// This error can be used for business logic validations that fail
    /// before interacting with the database.
    ValidationError(String),

    /// Represents an error when a requested item (e.g., a database record) is not found.
    NotFound(String),

    /// Represents an error when an invalid column name or definition is specified.
    InvalidColumn(String),

    /// Represents a general database error with a message detailing the issue.
    ///
    /// This is a catch-all for database-related errors that don't fit into
    /// more specific categories like `QueryError` or `ConnectionError`.
    DatabaseError(String),

    /// Represents an error when a requested feature (e.g., support for a specific database) is not enabled.
    FeatureNotEnabled(String),

    /// Represents an error during deserialization with a message detailing the issue.
    ///
    /// This error occurs when converting data received from the database
    /// (e.g., rows, JSON) into Rust data structures.
    DeserializationError(String),
}

/// Implements the `fmt::Display` trait for `RusticxError`.
///
/// This allows `RusticxError` instances to be easily printed using `{}`
/// format specifier, providing a user-friendly representation of the error.
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

/// Implements the `std::error::Error` trait for `RusticxError`.
///
/// This makes `RusticxError` a standard error type in Rust, allowing it
/// to be used with features like `?` operator for easy error propagation
/// and boxed dynamic error types (`Box<dyn std::error::Error>`).
impl std::error::Error for RusticxError {}

// Conversions from other specific error types to RusticxError

/// Implements conversion from `tokio_postgres::Error` to `RusticxError`.
///
/// This simplifies error handling by automatically converting errors from
/// the `tokio-postgres` crate into a `RusticxError::QueryError`.
#[cfg(feature = "postgres")]
impl From<tokio_postgres::Error> for RusticxError {
    fn from(err: tokio_postgres::Error) -> Self {
        RusticxError::QueryError(err.to_string())
    }
}

/// Implements conversion from `mysql::Error` to `RusticxError`.
///
/// This simplifies error handling by automatically converting errors from
/// the `mysql` crate into a `RusticxError::QueryError`.
#[cfg(feature = "mysql")]
impl From<mysql::Error> for RusticxError {
    fn from(err: mysql::Error) -> Self {
        RusticxError::QueryError(err.to_string())
    }
}

/// Implements conversion from `rusqlite::Error` to `RusticxError`.
///
/// This simplifies error handling by automatically converting errors from
/// the `rusqlite` crate into a `RusticxError::QueryError`.
#[cfg(feature = "rusqlite")]
impl From<rusqlite::Error> for RusticxError {
    fn from(err: rusqlite::Error) -> Self {
        RusticxError::QueryError(err.to_string())
    }
}

/// Implements conversion from `serde_json::Error` to `RusticxError`.
///
/// This simplifies error handling by automatically converting errors from
/// the `serde_json` crate into a `RusticxError::SerializationError`.
impl From<serde_json::Error> for RusticxError {
    fn from(err: serde_json::Error) -> Self {
        RusticxError::SerializationError(err.to_string())
    }
}