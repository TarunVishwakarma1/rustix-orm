use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use crate::error::RusticxError;
use crate::model::SQLModel;
use crate::transaction_manager::TransactionExecutor;

// Conditional includes based on feature flags
#[cfg(feature = "mysql")]
use crate::transaction_manager::{run_mysql_transaction, mysql};
#[cfg(feature = "rusqlite")]
use crate::transaction_manager::{run_sqlite_transaction, rusqlite};
#[cfg(feature = "postgres")]
use crate::transaction_manager::{run_postgres_transaction, tokio_postgres};
#[cfg(feature = "postgres")]
use postgres::types::ToSql;
use tokio::runtime::Runtime;

#[cfg(feature = "mysql")]
use mysql::prelude::Queryable;

/// Represents the type of database being used.
#[derive(Debug, Clone)]
pub enum DatabaseType {
    /// PostgreSQL database type.
    PostgreSQL,
    /// MySQL database type.
    MySQL,
    /// SQLite database type.
    SQLite,
}

/// Represents a connection pool for different database types.
///
/// This enum holds the specific connection pool or client instance
/// depending on the enabled database feature. The `None` variant
/// indicates that no connection has been established yet.
#[derive(Clone)]
pub enum ConnectionPool {
    /// Connection pool/client for PostgreSQL.
    /// Holds an `Arc<Mutex<tokio_postgres::Client>>` for thread-safe access
    /// and an `Arc<Runtime>` for managing async operations.
    #[cfg(feature = "postgres")]
    PostgreSQL(Arc<Mutex<tokio_postgres::Client>>, Arc<Runtime>),
    /// Connection pool for MySQL.
    /// Holds an `Arc<mysql::Pool>`.
    #[cfg(feature = "mysql")]
    MySQL(Arc<mysql::Pool>),
    /// Connection for SQLite.
    /// Holds an `Arc<Mutex<rusqlite::Connection>>` for thread-safe access.
    #[cfg(feature = "rusqlite")]
    SQLite(Arc<Mutex<rusqlite::Connection>>),
    /// Represents an uninitialized or closed connection pool.
    None,
}

/// Represents a database connection with its URL, type, and connection pool.
///
/// This struct provides a unified interface for interacting with different
/// database systems supported by the `rusticx` library.
#[derive(Clone)]
pub struct Connection {
    /// The database connection URL.
    url: String,
    /// The type of the database.
    db_type: DatabaseType,
    /// The underlying connection pool or client.
    pool: ConnectionPool,
}

impl Connection {
    /// Creates a new `Connection` instance based on the provided database URL.
    ///
    /// This function determines the database type from the URL scheme and
    /// attempts to establish a connection using the appropriate driver.
    ///
    /// # Arguments
    ///
    /// * `url`: The database connection string (e.g., "postgres://...", "mysql://...", "sqlite://...").
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the initialized `Connection` on success,
    /// or a `RusticxError` if the URL is invalid or connection fails.
    pub fn new(url: &str) -> Result<Self, RusticxError> {
        let db_type = if url.starts_with("postgres://") {
            DatabaseType::PostgreSQL
        } else if url.starts_with("mysql://") {
            DatabaseType::MySQL
        } else if url.starts_with("sqlite://") {
            DatabaseType::SQLite
        } else {
            return Err(RusticxError::ConnectionError(
                "Invalid database URL scheme. Must start with postgres://, mysql://, or sqlite://"
                    .to_string(),
            ));
        };

        let connection = Connection {
            url: url.to_string(),
            db_type,
            pool: ConnectionPool::None, // Initialize with None, connect() will populate
        };

        // Immediately attempt to connect after determining the type
        connection.connect()
    }

    /// Establishes a connection to the database and returns the updated `Connection`.
    ///
    /// This internal helper function performs the actual database connection
    /// based on the determined `DatabaseType` and populates the `pool` field.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the `Connection` with an active pool on success,
    /// or a `RusticxError` if the connection fails or the database feature is not enabled.
    fn connect(self) -> Result<Self, RusticxError> {
        let pool = match self.db_type {
            #[cfg(feature = "postgres")]
            DatabaseType::PostgreSQL => {
                use tokio_postgres::NoTls;

                // Create a dedicated Tokio runtime for blocking async operations
                let rt = Runtime::new().map_err(|e| {
                    RusticxError::ConnectionError(format!("Failed to create Tokio runtime: {}", e))
                })?;

                let (client, connection) = rt
                    .block_on(async { tokio_postgres::connect(&self.url, NoTls).await })
                    .map_err(|e| {
                        RusticxError::ConnectionError(format!("Failed to connect to PostgreSQL: {}", e))
                    })?;

                // Spawn a task to handle the connection errors asynchronously
                rt.spawn(async move {
                    if let Err(e) = connection.await {
                        eprintln!("PostgreSQL connection error: {}", e);
                    }
                });

                ConnectionPool::PostgreSQL(Arc::new(Mutex::new(client)), Arc::new(rt))
            }

            #[cfg(feature = "mysql")]
            DatabaseType::MySQL => {
                let opts = mysql::OptsBuilder::from_opts(
                    mysql::Opts::from_url(&self.url)
                        .map_err(|e| RusticxError::ConnectionError(format!("Invalid MySQL URL: {}", e)))?,
                );
                let pool = mysql::Pool::new(opts)
                    .map_err(|e| RusticxError::ConnectionError(format!("Failed to connect to MySQL: {}", e)))?;
                ConnectionPool::MySQL(Arc::new(pool))
            }

            #[cfg(feature = "rusqlite")]
            DatabaseType::SQLite => {
                let path = self.url.trim_start_matches("sqlite://");
                let conn = rusqlite::Connection::open(path).map_err(|e| {
                    RusticxError::ConnectionError(format!("Failed to connect to SQLite: {}", e))
                })?;
                ConnectionPool::SQLite(Arc::new(Mutex::new(conn)))
            }

            // This pattern is marked unreachable because the initial URL check
            // should cover all supported types. However, it serves as a fallback
            // for completeness and handles cases where a feature is not enabled.
            #[allow(unreachable_patterns)]
            _ => {
                return Err(RusticxError::ConnectionError(format!(
                    "Database type {:?} is not supported or the corresponding feature is not enabled (check Cargo.toml)",
                    self.db_type
                )));
            }
        };

        Ok(Connection {
            url: self.url.clone(),
            db_type: self.db_type.clone(),
            pool,
        })
    }

    /// Creates a table in the database based on the provided SQL model definition.
    ///
    /// This function uses the `SQLModel` trait to generate the appropriate
    /// `CREATE TABLE` SQL statement for the current database type and
    /// executes it.
    ///
    /// # Type Parameters
    ///
    /// * `T`: The type representing the SQL model, which must implement `SQLModel`.
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` on successful table creation, or a `RusticxError`
    /// if the SQL generation or execution fails.
    pub fn create_table<T: SQLModel>(&self) -> Result<(), RusticxError> {
        // The table name is not directly used here, but could be for logging or validation
        let _table_name = T::table_name();
        let sql = T::create_table_sql(&self.db_type);
        self.execute(&sql, &[])?;
        Ok(())
    }

    /// Executes a SQL command (INSERT, UPDATE, DELETE, CREATE, DROP, etc.)
    /// with the provided parameters.
    ///
    /// This function is typically used for commands that do not return a result set.
    /// The number of affected rows (where applicable) is returned.
    ///
    /// # Arguments
    ///
    /// * `sql`: The SQL query string to execute.
    /// * `params`: A slice of references to values to be used as query parameters.
    ///             The specific type required depends on the database driver
    ///             (e.g., `&(dyn ToSql + Sync + 'static)` for postgres).
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the number of rows affected on success,
    /// or a `RusticxError` if the execution fails. Note that the meaning
    /// and availability of "rows affected" can vary between database drivers.
    ///
    /// # Errors
    ///
    /// Returns a `RusticxError::QueryError` on database query execution failure
    /// or `RusticxError::ConnectionError` if the connection pool is not initialized.
    pub fn execute(
        &self,
        sql: &str,
        params: &[&(dyn ToSql + Sync + 'static)],
    ) -> Result<u64, RusticxError> {
        match &self.pool {
            #[cfg(feature = "postgres")]
            ConnectionPool::PostgreSQL(client, rt) => {
                let client_guard = client.lock().map_err(|e| {
                    RusticxError::TransactionError(format!("Failed to acquire lock on connection: {}", e))
                })?;

                let result = rt
                    .block_on(async { client_guard.execute(sql, params).await })
                    .map_err(|e| RusticxError::QueryError(e.to_string()))?;
                Ok(result)
            }

            #[cfg(feature = "mysql")]
            ConnectionPool::MySQL(pool) => {
                let mut conn = pool
                    .get_conn()
                    .map_err(|e| RusticxError::QueryError(e.to_string()))?;
                // MySQL's `exec_drop` does not reliably return rows affected, returning 1 is a common workaround
                conn.exec_drop(sql, ())
                    .map_err(|e| RusticxError::QueryError(e.to_string()))?;
                Ok(1) // Indicate at least one operation was attempted
            }

            #[cfg(feature = "rusqlite")]
            ConnectionPool::SQLite(conn) => {
                let conn_guard = conn.lock().map_err(|e| {
                    RusticxError::ConnectionError(format!("Failed to acquire lock on SQLite connection: {}", e))
                })?;
                let result = conn_guard
                    .execute(sql, []) // rusqlite requires params as a slice of ToSql, converting &[&dyn ToSql] to &[&dyn ToSql] is complex. Assuming no params for simplicity in this example or adjust signature.
                    .map_err(|e| RusticxError::QueryError(e.to_string()))?;
                Ok(result as u64)
            }

            ConnectionPool::None => {
                Err(RusticxError::ConnectionError(
                    "No active database connection pool initialized".to_string(),
                ))
            }

            // Fallback for unsupported or disabled database types
            #[allow(unreachable_patterns)]
            _ => Err(RusticxError::ConnectionError(
                "Unsupported database type for execute operation".to_string(),
            )),
        }
    }

    /// Executes a raw SQL query (typically SELECT) and returns the results
    /// as a vector of deserialized objects.
    ///
    /// This function queries the database and attempts to map the rows
    /// from the result set into instances of the specified type `T`.
    ///
    /// # Type Parameters
    ///
    /// * `T`: The target type to deserialize the rows into. Must implement
    ///        `serde::Deserialize<'de>` and `Debug`.
    ///
    /// # Arguments
    ///
    /// * `sql`: The SQL query string (e.g., "SELECT id, name FROM users WHERE age > $1").
    /// * `params`: A slice of references to values to be used as query parameters.
    ///             The specific type required depends on the database driver.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing a `Vec<T>` on success, where each element
    /// corresponds to a row from the result set, or a `RusticxError` if the
    /// query or deserialization fails.
    ///
    /// # Errors
    ///
    /// Returns a `RusticxError::QueryError` on database query execution failure,
    /// `RusticxError::SerializationError` if deserialization fails, or
    /// `RusticxError::ConnectionError` if the connection pool is not initialized.
    pub fn query_raw<T>(&self, sql: &str, params: &[&(dyn ToSql + Sync + 'static)]) -> Result<Vec<T>, RusticxError>
    where
        T: for<'de> serde::Deserialize<'de> + Debug,
    {
        match &self.pool {
            #[cfg(feature = "postgres")]
            ConnectionPool::PostgreSQL(client, rt) => {
                let client_guard = client.lock().map_err(|e| {
                    RusticxError::TransactionError(format!("Failed to acquire lock on connection: {}", e))
                })?;
                let rows = rt
                    .block_on(async { client_guard.query(sql, params).await })
                    .map_err(|e| RusticxError::QueryError(e.to_string()))?;

                let mut models = Vec::with_capacity(rows.len());
                for row in rows {
                    let mut json_obj = serde_json::Map::new();
                    for column in row.columns() {
                        let name = column.name();
                        // Assuming a helper function exists to convert pg row value to JSON
                        let value = crate::transaction_manager::pg_row_value_to_json(&row, column)
                            .unwrap_or(serde_json::Value::Null); // Use Null for unconvertible values
                        json_obj.insert(name.to_string(), value);
                    }
                    let model = serde_json::from_value(serde_json::Value::Object(json_obj))
                        .map_err(|e| RusticxError::SerializationError(e.to_string()))?;
                    models.push(model);
                }
                Ok(models)
            }

            #[cfg(feature = "mysql")]
            ConnectionPool::MySQL(pool) => {
                let mut conn = pool
                    .get_conn()
                    .map_err(|e| RusticxError::QueryError(e.to_string()))?;

                // Use query_map to iterate over results and convert
                let rows: Vec<Result<T, mysql::Error>> = conn
                    .query_map(sql, |row: mysql::Row| {
                        let mut json_obj = serde_json::Map::new();
                        let columns = row.columns_ref();

                        for (i, column) in columns.iter().enumerate() {
                            let name = column.name_str().to_string();
                            // Assuming a helper function exists to convert mysql row value to JSON
                            let value = crate::transaction_manager::mysql_row_value_to_json(
                                &row,
                                i,
                                column.column_type(),
                            )
                            .unwrap_or(serde_json::Value::Null);
                            json_obj.insert(name, value);
                        }

                        // Deserialize the JSON object into the target struct T
                        serde_json::from_value(serde_json::Value::Object(json_obj)).map_err(|e| {
                            // Convert serde_json error to a mysql error for compatibility with query_map
                            mysql::Error::from(std::io::Error::new(
                                std::io::ErrorKind::Other,
                                e.to_string(),
                            ))
                        })
                    })
                    .map_err(|e| RusticxError::QueryError(e.to_string()))?;

                // Collect the results, converting the vector of Results into a single Result<Vec<T>>
                let result: Vec<T> = rows
                    .into_iter()
                    .collect::<Result<_, _>>()
                    .map_err(|e| RusticxError::QueryError(e.to_string()))?;

                Ok(result)
            }

            #[cfg(feature = "rusqlite")]
            ConnectionPool::SQLite(conn) => {
                let conn_guard = conn.lock().map_err(|e| {
                    RusticxError::ConnectionError(format!("Failed to acquire lock on SQLite connection: {}", e))
                })?;

                let mut stmt = conn_guard
                    .prepare(sql)
                    .map_err(|e| RusticxError::QueryError(e.to_string()))?;

                let column_names: Vec<String> = stmt
                    .column_names()
                    .iter()
                    .map(|name| name.to_string())
                    .collect();

                let models = stmt
                    .query_map([], |row| {
                        // Map each row to a JSON object
                        let mut json_obj = serde_json::Map::new();
                        for (i, name) in column_names.iter().enumerate() {
                            // Assuming a helper function exists to convert sqlite row value to JSON
                            let value = crate::transaction_manager::sqlite_row_value_to_json(row, i)
                                .unwrap_or(serde_json::Value::Null);
                            json_obj.insert(name.clone(), value);
                        }
                        // Deserialize the JSON object into the target struct T
                        serde_json::from_value(serde_json::Value::Object(json_obj)).map_err(
                            |e| {
                                // Convert serde_json error to a rusqlite error
                                rusqlite::Error::FromSqlConversionFailure(
                                    i, // Column index where error occurred
                                    rusqlite::types::Type::Text, // Assuming Text type for conversion
                                    Box::new(e),
                                )
                            },
                        )?;
                        Ok(model)
                    })
                    .map_err(|e| RusticxError::QueryError(e.to_string()))?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| RusticxError::QueryError(e.to_string()))?; // Collect results and handle potential errors

                Ok(models)
            }

            ConnectionPool::None => {
                Err(RusticxError::ConnectionError(
                    "No active database connection pool initialized".to_string(),
                ))
            }

            // Fallback for unsupported or disabled database types
            #[allow(unreachable_patterns)]
            _ => Err(RusticxError::ConnectionError(
                "Unsupported database type for query operation".to_string(),
            )),
        }
    }

    /// Executes a database transaction using the provided transaction function.
    ///
    /// This function manages the transaction lifecycle (begin, commit/rollback)
    /// and executes the code defined in the `transaction_fn` closure within
    /// the transaction's scope. The closure receives a `TransactionExecutor`
    /// which allows performing database operations within the transaction.
    ///
    /// # Type Parameters
    ///
    /// * `F`: The type of the closure that defines the transaction logic. Must
    ///        implement `FnOnce(&dyn TransactionExecutor) -> Result<R, RusticxError>`,
    ///        `Send`, and `'static`.
    /// * `R`: The return type of the transaction function. Must implement `Send`
    ///        and `'static`.
    ///
    /// # Arguments
    ///
    /// * `transaction_fn`: The closure containing the database operations to be
    ///                   executed within the transaction.
    ///
    /// # Returns
    ///
    /// Returns a `Result` containing the value `R` returned by the transaction
    /// function on successful commit, or a `RusticxError` if the transaction
    /// fails or is rolled back.
    ///
    /// # Errors
    ///
    /// Returns `RusticxError::TransactionError` on transaction management failures,
    /// or `RusticxError::ConnectionError` if the connection pool is not initialized.
    pub async fn transaction<F, R>(&self, transaction_fn: F) -> Result<R, RusticxError>
    where
        F: FnOnce(&dyn TransactionExecutor) -> Result<R, RusticxError> + Send + 'static,
        R: Send + 'static,
    {
        match &self.pool {
            #[cfg(feature = "postgres")]
            ConnectionPool::PostgreSQL(client, _) => {
                // Delegate to the PostgreSQL specific transaction runner
                run_postgres_transaction(&client.clone(), transaction_fn).await
            }

            #[cfg(feature = "mysql")]
            ConnectionPool::MySQL(pool) => {
                // Delegate to the MySQL specific transaction runner
                run_mysql_transaction(&pool.clone(), transaction_fn)
            }

            #[cfg(feature = "rusqlite")]
            ConnectionPool::SQLite(conn) => {
                // Delegate to the SQLite specific transaction runner
                run_sqlite_transaction(&conn.clone(), transaction_fn)
            }

            ConnectionPool::None => {
                Err(RusticxError::ConnectionError(
                    "No active database connection pool initialized for transaction".to_string(),
                ))
            }

            // Fallback for unsupported or disabled database types
            #[allow(unreachable_patterns)]
            _ => Err(RusticxError::ConnectionError(
                "Unsupported database type for transaction operation".to_string(),
            )),
        }
    }

    /// Returns a reference to the database type of this connection.
    ///
    /// # Returns
    ///
    /// A reference to the `DatabaseType` enum indicating the connected database.
    pub fn get_db_type(&self) -> &DatabaseType {
        &self.db_type
    }
}