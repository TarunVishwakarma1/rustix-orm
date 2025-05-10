use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use crate::error::RustixError;
#[cfg(feature = "mysql")]
use mysql::prelude::Queryable;
#[cfg(feature = "rusqlite")]
use base64::Engine;

// Re-export needed types for external users
#[cfg(feature = "postgres")]
pub use tokio_postgres;
#[cfg(feature = "mysql")]
pub use mysql;
#[cfg(feature = "rusqlite")]
pub use rusqlite;

pub trait TransactionExecutor {
    /// Executes an SQL statement with parameters
    /// Returns the number of rows affected.
    fn execute(&mut self, sql: &str, params: &[&dyn Debug]) -> Result<u64, RustixError>;
}

pub trait QueryExecutor {
    /// Executes a query and returns the results as a vector of deserialized objects.
    /// Note: Due to Rust's trait object limitations with generic methods,
    /// `query_raw` makes this trait not fully dyn compatible if `T` varies at runtime.
    /// For true dynamic dispatch on return types, consider returning a standard
    /// intermediate representation (like `serde_json::Value`).
    fn query_raw<T>(&mut self, sql: &str, params: &[&dyn Debug]) -> Result<Vec<T>, RustixError>
    where
        T: for<'de> serde::Deserialize<'de>;
}

// PostgreSQL transaction executor implementation
#[cfg(feature = "postgres")]
pub struct PostgresTransactionExecutor<'a> {
    pub(crate) tx: &'a tokio_postgres::Transaction<'a>,
}

#[cfg(feature = "postgres")]
impl<'a> TransactionExecutor for PostgresTransactionExecutor<'a> {
    fn execute(&mut self, sql: &str, params: &[&dyn Debug]) -> Result<u64, RustixError> {
        // Consider using a shared runtime or moving to async for execute if possible
        let rt = tokio::runtime::Runtime::new().map_err(|e| {
            RustixError::QueryError(format!("Failed to create runtime: {}", e))
        })?;

        // TODO: Implement proper parameter binding for tokio-postgres
        // This requires converting &[&dyn Debug] to &[&(dyn tokio_postgres::types::ToSql + Sync)]
        let result = rt
            .block_on(async { self.tx.execute(sql, &[]).await }) // Using &[] as placeholder
            .map_err(|e| RustixError::QueryError(e.to_string()))?;

        Ok(result)
    }
}

#[cfg(feature = "postgres")]
impl<'a> QueryExecutor for PostgresTransactionExecutor<'a> {
    fn query_raw<T>(&mut self, sql: &str, params: &[&dyn Debug]) -> Result<Vec<T>, RustixError>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        // Consider using a shared runtime or moving to async for query_raw if possible
        let rt = tokio::runtime::Runtime::new().map_err(|e| {
            RustixError::QueryError(format!("Failed to create runtime: {}", e))
        })?;

        // TODO: Implement proper parameter binding for tokio-postgres
        let rows = rt
            .block_on(async { self.tx.query(sql, &[]).await }) // Using &[] as placeholder
            .map_err(|e| RustixError::QueryError(e.to_string()))?;

        let mut models = Vec::with_capacity(rows.len());
        for row in rows {
            let mut json_obj = serde_json::Map::new();

            for column in row.columns() {
                let name = column.name();
                // Use the helper function to extract and convert the value
                let value = pg_row_value_to_json(&row, column).unwrap_or(serde_json::Value::Null);
                json_obj.insert(name.to_string(), value);
            }

            let model = serde_json::from_value(serde_json::Value::Object(json_obj))
                .map_err(|e| RustixError::SerializationError(e.to_string()))?;

            models.push(model);
        }

        Ok(models)
    }
}

// Helper function to extract value from Postgres row and convert to serde_json::Value
#[cfg(feature = "postgres")]
pub fn pg_row_value_to_json(
    row: &tokio_postgres::Row,
    column: &tokio_postgres::Column,
) -> Result<serde_json::Value, tokio_postgres::Error> {
    let name = column.name();
    let type_oid = column.type_().oid();

    match type_oid {
        // int4/int8
        23 | 20 => {
            if let Ok(val) = row.try_get::<_, i32>(name) {
                Ok(serde_json::Value::Number(serde_json::Number::from(val)))
            } else if let Ok(val) = row.try_get::<_, i64>(name) {
                Ok(serde_json::Value::Number(serde_json::Number::from(val)))
            } else {
                Ok(serde_json::Value::Null)
            }
        }
        // text/varchar
        25 | 1043 => row.try_get::<_, String>(name).map(serde_json::Value::String),
        // bool
        16 => row.try_get::<_, bool>(name).map(serde_json::Value::Bool),
        // timestamp/timestamptz (treating as string for simplicity)
        1114 | 1184 => row.try_get::<_, String>(name).map(serde_json::Value::String),
        // Other types - attempt to convert to string
        _ => row.try_get::<_, String>(name).map(serde_json::Value::String),
    }
}

// MySQL transaction executor implementation
#[cfg(feature = "mysql")]
pub struct MySQLTransactionExecutor<'a> {
    pub(crate) conn: &'a mut mysql::PooledConn,
}

#[cfg(feature = "mysql")]
impl<'a> TransactionExecutor for MySQLTransactionExecutor<'a> {
    fn execute(&mut self, sql: &str, params: &[&dyn Debug]) -> Result<u64, RustixError> {
        // TODO: Implement proper parameter binding for mysql-connector-rust
        self.conn
            .exec_drop(sql, ()) // Using () as placeholder parameters
            .map_err(|e| RustixError::QueryError(e.to_string()))?;

        // MySQL exec_drop doesn't reliably return affected rows for all statements.
        // Returning 1 as a placeholder; a more robust approach might be needed.
        Ok(1)
    }
}

#[cfg(feature = "mysql")]
impl<'a> QueryExecutor for MySQLTransactionExecutor<'a> {
    fn query_raw<T>(&mut self, sql: &str, _params: &[&dyn std::fmt::Debug]) -> Result<Vec<T>, RustixError>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        // TODO: Implement proper parameter binding for mysql-connector-rust
        let rows: Vec<Result<T, mysql::Error>> = self.conn.query_map(sql, |row: mysql::Row| {
            let mut json_obj = serde_json::Map::new();
            let columns = row.columns_ref();

            for (i, column) in columns.iter().enumerate() {
                let name = column.name_str().to_string();
                // Use the helper function to extract and convert the value
                let value = mysql_row_value_to_json(&row, i, column.column_type())
                    .unwrap_or(serde_json::Value::Null);
                json_obj.insert(name, value);
            }

            serde_json::from_value(serde_json::Value::Object(json_obj))
                .map_err(|e| mysql::Error::from(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))
        }).map_err(|e| RustixError::QueryError(e.to_string()))?;

        let result: Vec<T> = rows
            .into_iter()
            .collect::<Result<_, _>>()
            .map_err(|e| RustixError::QueryError(e.to_string()))?;

        Ok(result)
    }
}

// Helper function to extract value from MySQL row and convert to serde_json::Value
#[cfg(feature = "mysql")]
pub fn mysql_row_value_to_json(
    row: &mysql::Row,
    index: usize,
    column_type: mysql::consts::ColumnType,
) -> Result<serde_json::Value, mysql::Error> {
    match column_type {
        mysql::consts::ColumnType::MYSQL_TYPE_TINY
        | mysql::consts::ColumnType::MYSQL_TYPE_SHORT
        | mysql::consts::ColumnType::MYSQL_TYPE_LONG
        | mysql::consts::ColumnType::MYSQL_TYPE_INT24 => {
            row.get_opt::<i32, _>(index)
                .transpose()? // Transpose Option<Result<T, E>> to Result<Option<T>, E>
                .map(|v| serde_json::Value::Number(v.into()))
                .ok_or_else(|| { // Handle the Option<Value> to Result<Value, Error> conversion
                    mysql::Error::from(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to get INT or INT24 value at index {}", index),
                    ))
                })
        }
        mysql::consts::ColumnType::MYSQL_TYPE_LONGLONG => {
            row.get_opt::<i64, _>(index)
                .transpose()?
                .map(|v| serde_json::Value::Number(serde_json::Number::from(v)))
                .ok_or_else(|| {
                    mysql::Error::from(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to get LONGLONG value at index {}", index),
                    ))
                })
        }
        mysql::consts::ColumnType::MYSQL_TYPE_FLOAT | mysql::consts::ColumnType::MYSQL_TYPE_DOUBLE => {
            row.get_opt::<f64, _>(index)
                .transpose()?
                .map(|v| {
                    serde_json::Number::from_f64(v)
                        .map(serde_json::Value::Number)
                        .unwrap_or(serde_json::Value::Null) // Handle potential f64 to Number conversion failure
                })
                .ok_or_else(|| {
                    mysql::Error::from(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to get FLOAT or DOUBLE value at index {}", index),
                    ))
                })
        }
        mysql::consts::ColumnType::MYSQL_TYPE_STRING
        | mysql::consts::ColumnType::MYSQL_TYPE_VAR_STRING
        | mysql::consts::ColumnType::MYSQL_TYPE_VARCHAR
        | mysql::consts::ColumnType::MYSQL_TYPE_TINY_BLOB
        | mysql::consts::ColumnType::MYSQL_TYPE_MEDIUM_BLOB
        | mysql::consts::ColumnType::MYSQL_TYPE_LONG_BLOB
        | mysql::consts::ColumnType::MYSQL_TYPE_BLOB
        | mysql::consts::ColumnType::MYSQL_TYPE_DATE
        | mysql::consts::ColumnType::MYSQL_TYPE_DATETIME
        | mysql::consts::ColumnType::MYSQL_TYPE_TIMESTAMP => {
            row.get_opt::<String, _>(index)
                .transpose()?
                .map(serde_json::Value::String)
                .ok_or_else(|| {
                     mysql::Error::from(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to get STRING or related value at index {}", index),
                    ))
                })
        }
        _ => {
            // Handle other types by attempting to get them as a String
            row.get_opt::<String, _>(index)
                .transpose()?
                .map(serde_json::Value::String)
                .ok_or_else(|| {
                     mysql::Error::from(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("Failed to get value as String for unknown type at index {}", index),
                    ))
                })
        }
    }
}



// SQLite transaction executor implementation
#[cfg(feature = "rusqlite")]
pub struct SQLiteTransactionExecutor<'a> {
    pub(crate) tx: &'a rusqlite::Transaction<'a>,
}

#[cfg(feature = "rusqlite")]
impl<'a> TransactionExecutor for SQLiteTransactionExecutor<'a> {
    fn execute(&mut self, sql: &str, params: &[&dyn Debug]) -> Result<u64, RustixError> {
        // TODO: Implement proper parameter binding for rusqlite
        let result = self
            .tx
            .execute(sql, []) // Using [] as placeholder parameters
            .map_err(|e| RustixError::QueryError(e.to_string()))?;

        Ok(result as u64)
    }
}

#[cfg(feature = "rusqlite")]
impl<'a> QueryExecutor for SQLiteTransactionExecutor<'a> {
    fn query_raw<T>(&mut self, sql: &str, _params: &[&dyn Debug]) -> Result<Vec<T>, RustixError>
    where
        T: for<'de> serde::Deserialize<'de>,
    {
        let mut stmt = self
            .tx
            .prepare(sql)
            .map_err(|e| RustixError::QueryError(e.to_string()))?;

        let column_names: Vec<String> = stmt
            .column_names()
            .iter()
            .map(|name| name.to_string())
            .collect();

        let models = stmt
            .query_map([], |row| {
                let mut json_obj = serde_json::Map::new();

                for (i, name) in column_names.iter().enumerate() {
                    // Use the helper function to extract and convert the value
                    let value = sqlite_row_value_to_json(row, i)
                        .unwrap_or(serde_json::Value::Null);
                    json_obj.insert(name.clone(), value); // Clone name as it's a reference
                }

                let model = serde_json::from_value(serde_json::Value::Object(json_obj)).map_err(
                    |e| rusqlite::Error::FromSqlConversionFailure(
                        0,
                        rusqlite::types::Type::Text,
                        Box::new(e),
                    ),
                )?;

                Ok(model)
            })
            .map_err(|e| RustixError::QueryError(e.to_string()))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| RustixError::QueryError(e.to_string()))?;

        Ok(models)
    }
}

// Helper function to extract value from SQLite row and convert to serde_json::Value
#[cfg(feature = "rusqlite")]
pub fn sqlite_row_value_to_json(
    row: &rusqlite::Row<'_>,
    index: usize,
) -> Result<serde_json::Value, rusqlite::Error> {
    match row.get_ref(index)?.data_type() {
        rusqlite::types::Type::Integer => {
            row.get::<_, i64>(index).map(|v| serde_json::Value::Number(v.into()))
        }
        rusqlite::types::Type::Real => {
            row.get::<_, f64>(index)
                .ok()
                .and_then(serde_json::Number::from_f64)
                .map(serde_json::Value::Number)
                .ok_or_else(|| {
                    rusqlite::Error::FromSqlConversionFailure(
                        index,
                        rusqlite::types::Type::Real,
                        Box::new(std::io::Error::new(std::io::ErrorKind::Other, "Failed to convert f64 to serde_json::Number")),
                    )
                })
        }
        rusqlite::types::Type::Text => row.get::<_, String>(index).map(serde_json::Value::String),
        rusqlite::types::Type::Blob => {
            row.get::<_, Vec<u8>>(index).map(|v| {
                let b64 = base64::engine::general_purpose::STANDARD.encode(v);
                serde_json::Value::String(b64)
            })
        }
        rusqlite::types::Type::Null => Ok(serde_json::Value::Null),
    }
}

/// Helper function to run a transaction with PostgreSQL
#[cfg(feature = "postgres")]
pub(crate) async fn run_postgres_transaction<F, R>(
    client: &Arc<Mutex<tokio_postgres::Client>>, // Use &Client instead of &mut
    transaction_fn: F,
) -> Result<R, RustixError>
where
    F: FnOnce(&dyn TransactionExecutor) -> Result<R, RustixError>,
{
    // Create a transaction

    let mut guard = client.lock().map_err(|e| {
        RustixError::TransactionError(format!("Failed to acquire lock on connection: {}", e))
    })?;
    let tx = guard
        .transaction()
        .await
        .map_err(|e| RustixError::TransactionError(format!("Failed to start transaction: {}", e)))?;

    // Create a transaction executor
    let mut tx_executor = PostgresTransactionExecutor { tx: &tx };

    // Execute the user's function within the transaction
    let result = transaction_fn(&mut tx_executor); // Pass mutable reference

    // Commit or rollback based on the result
    match result {
        Ok(value) => {
            tx.commit()
                .await
                .map_err(|e| RustixError::TransactionError(format!("Failed to commit transaction: {}", e)))?;
            Ok(value)
        }
        Err(e) => {
            // Explicit rollback for clarity, though it happens automatically when tx is dropped
            if let Err(rollback_err) = tx.rollback().await {
                eprintln!("Error during transaction rollback: {}", rollback_err);
            }
            Err(e)
        }
    }
}

/// Helper function to run a transaction with MySQL
#[cfg(feature = "mysql")]
pub(crate) fn run_mysql_transaction<F, R>(
    pool: &Arc<mysql::Pool>,
    transaction_fn: F,
) -> Result<R, RustixError>
where
    F: FnOnce(&dyn TransactionExecutor) -> Result<R, RustixError>,
{
    // Get a connection from the pool
    let mut conn = pool
        .get_conn()
        .map_err(|e| RustixError::TransactionError(format!("Failed to get MySQL connection: {}", e)))?;

    // Start a transaction
    conn.exec_drop("START TRANSACTION", ())
        .map_err(|e| RustixError::TransactionError(format!("Failed to start transaction: {}", e)))?;

    // Create a transaction executor
    let mut tx_executor = MySQLTransactionExecutor { conn: &mut conn };

    // Execute the user's function
    let result = transaction_fn(&mut tx_executor); // Pass mutable reference

    // Commit or rollback
    match result {
        Ok(value) => {
            conn.exec_drop("COMMIT", ())
                .map_err(|e| RustixError::TransactionError(format!("Failed to commit transaction: {}", e)))?;
            Ok(value)
        }
        Err(e) => {
            if let Err(rollback_err) = conn.exec_drop("ROLLBACK", ()) {
                eprintln!("Error during transaction rollback: {}", rollback_err);
            }
            Err(e)
        }
    }
}

/// Helper function to run a transaction with SQLite
#[cfg(feature = "rusqlite")]
pub(crate) fn run_sqlite_transaction<F, R>(
    conn: &Arc<Mutex<rusqlite::Connection>>,
    transaction_fn: F,
) -> Result<R, RustixError>
where
    F: FnOnce(&dyn TransactionExecutor) -> Result<R, RustixError>,
{

    let mut guard = conn.lock().map_err(|e| {
        RustixError::TransactionError(format!("Failed to acquire lock on connection: {}", e))
    })?;
    // Start a transaction
    let tx = guard
        .transaction()
        .map_err(|e| RustixError::TransactionError(format!("Failed to start transaction: {}", e)))?;

    // Create a transaction executor
    let mut tx_executor = SQLiteTransactionExecutor { tx: &tx };

    // Execute the user's function
    let result = transaction_fn(&mut tx_executor); // Pass mutable reference

    // Commit or rollback
    match result {
        Ok(value) => {
            tx.commit()
                .map_err(|e| RustixError::TransactionError(format!("Failed to commit transaction: {}", e)))?;
            Ok(value)
        }
        Err(e) => {
            if let Err(rollback_err) = tx.rollback() {
                eprintln!("Error during transaction rollback: {}", rollback_err);
            }
            Err(e)
        }
    }
}
