/// Represents the various SQL data types supported by the ORM.
#[derive(Debug, Clone, PartialEq)]
pub enum SqlType {
    Integer,
    BigInt,
    Float,
    Text,
    Boolean,
    Date,
    Time,
    DateTime,
    Blob,
    Custom(String), // Allows for custom SQL types
}

impl SqlType {
    /// Returns the PostgreSQL representation of the SQL type as a `String`.
    pub fn pg_type(&self) -> String {
        match self {
            SqlType::Integer => "INTEGER".to_string(),
            SqlType::BigInt => "BIGINT".to_string(),
            SqlType::Float => "REAL".to_string(),
            SqlType::Text => "TEXT".to_string(),
            SqlType::Boolean => "BOOLEAN".to_string(),
            SqlType::Date => "DATE".to_string(),
            SqlType::Time => "TIME".to_string(),
            SqlType::DateTime => "TIMESTAMP".to_string(),
            SqlType::Blob => "BYTEA".to_string(),
            SqlType::Custom(custom) => custom.clone(),
        }
    }

    /// Returns the MySQL representation of the SQL type as a `String`.
    pub fn mysql_type(&self) -> String {
        match self {
            SqlType::Integer => "INT".to_string(),
            SqlType::BigInt => "BIGINT".to_string(),
            SqlType::Float => "FLOAT".to_string(),
            SqlType::Text => "TEXT".to_string(),
            SqlType::Boolean => "BOOLEAN".to_string(),
            SqlType::Date => "DATE".to_string(),
            SqlType::Time => "TIME".to_string(),
            SqlType::DateTime => "DATETIME".to_string(),
            SqlType::Blob => "BLOB".to_string(),
            SqlType::Custom(custom) => custom.clone(),
        }
    }

    /// Returns the SQLite representation of the SQL type as a `String`.
    pub fn sqlite_type(&self) -> String {
        match self {
            SqlType::Integer => "INTEGER".to_string(),
            SqlType::BigInt => "INTEGER".to_string(), // SQLite uses INTEGER for BIGINT
            SqlType::Float => "REAL".to_string(),
            SqlType::Text => "TEXT".to_string(),
            SqlType::Boolean => "INTEGER".to_string(), // SQLite uses INTEGER for booleans
            SqlType::Date => "TEXT".to_string(),        // SQLite uses TEXT for dates
            SqlType::Time => "TEXT".to_string(),        // SQLite uses TEXT for times
            SqlType::DateTime => "TEXT".to_string(),    // SQLite uses TEXT for datetimes
            SqlType::Blob => "BLOB".to_string(),
            SqlType::Custom(custom) => custom.clone(),
        }
    }
}