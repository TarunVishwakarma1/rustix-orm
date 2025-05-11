use std::fmt::Debug;
use std::sync::{Arc, Mutex};
use crate::error::RustixError;
use crate::model::SQLModel;
use crate::transaction_manager::TransactionExecutor;
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
    PostgreSQL,
    MySQL,
    SQLite,
}

/// Represents a connection pool for different database types.
#[derive(Clone)]
pub enum ConnectionPool {
    #[cfg(feature = "postgres")]
    PostgreSQL(Arc<Mutex<tokio_postgres::Client>>, Arc<Runtime>),
    #[cfg(feature = "mysql")]
    MySQL(Arc<mysql::Pool>),
    #[cfg(feature = "rusqlite")]
    SQLite(Arc<Mutex<rusqlite::Connection>>),
    None,
}

/// Represents a database connection with its URL, type, and connection pool.
#[derive(Clone)]
pub struct Connection {
    url: String,
    db_type: DatabaseType,
    pool: ConnectionPool,
}

impl Connection {
    /// Creates a new `Connection` instance based on the provided database URL.
    /// Returns an error if the URL is invalid.
    pub fn new(url: &str) -> Result<Self, RustixError> {
        let db_type = if url.starts_with("postgres://") {
            DatabaseType::PostgreSQL
        } else if url.starts_with("mysql://") {
            DatabaseType::MySQL
        } else if url.starts_with("sqlite://") {
            DatabaseType::SQLite
        } else {
            return Err(RustixError::ConnectionError("Invalid database URL".to_string()));
        };

        let connection = Connection {
            url: url.to_string(),
            db_type,
            pool: ConnectionPool::None,
        };

        connection.connect()
    }

    /// Establishes a connection to the database and returns the updated `Connection`.
    fn connect(self) -> Result<Self, RustixError> {
        let pool = match self.db_type {
            #[cfg(feature = "postgres")]
            DatabaseType::PostgreSQL => {
                use tokio_postgres::NoTls;

                let rt = Runtime::new().map_err(|e| {
                    RustixError::ConnectionError(format!("Failed to create Tokio runtime: {}", e))
                })?;

                let (client, connection) = rt.block_on(async {
                    tokio_postgres::connect(&self.url, NoTls).await
                }).map_err(|e| {
                    RustixError::ConnectionError(format!("Failed to connect to PostgreSQL: {}", e))
                })?;

                rt.spawn(async move {
                    if let Err(e) = connection.await {
                        eprintln!("Database connection error: {}", e);
                    }
                });

                ConnectionPool::PostgreSQL(Arc::new(Mutex::new(client)), Arc::new(rt))
            }

            #[cfg(feature = "mysql")]
            DatabaseType::MySQL => {
                let opts = mysql::OptsBuilder::from_opts(
                    mysql::Opts::from_url(&self.url)
                        .map_err(|e| RustixError::ConnectionError(format!("Invalid MySQL URL: {}", e)))?,
                );
                let pool = mysql::Pool::new(opts)
                    .map_err(|e| RustixError::ConnectionError(format!("Failed to connect to MySQL: {}", e)))?;
                ConnectionPool::MySQL(Arc::new(pool))
            }

            #[cfg(feature = "rusqlite")]
            DatabaseType::SQLite => {
                let path = self.url.trim_start_matches("sqlite://");
                let conn = rusqlite::Connection::open(path)
                    .map_err(|e| RustixError::ConnectionError(format!("Failed to connect to SQLite: {}", e)))?;
                ConnectionPool::SQLite(Arc::new(Mutex::new(conn)))
            }

            #[allow(unreachable_patterns)]
            _ => {
                return Err(RustixError::ConnectionError(format!(
                    "Database type {:?} is not supported or the corresponding feature is not enabled",
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

    /// Creates a table in the database based on the provided SQL model.
    pub fn create_table<T: SQLModel>(&self) -> Result<(), RustixError> {
        let _table_name = T::table_name();
        let sql = T::create_table_sql(&self.db_type);
        self.execute(&sql, &[])?;
        Ok(())
    }

    /// Executes a SQL command with the provided parameters.
    pub fn execute(&self, sql: &str, params: &[&(dyn ToSql + Sync + 'static)]) -> Result<u64, RustixError> {
        match &self.pool {
            #[cfg(feature = "postgres")]
            ConnectionPool::PostgreSQL(client, rt) => {
                let client_guard = client.lock().map_err(|e| {
                    RustixError::TransactionError(format!("Failed to acquire lock on connection: {}", e))
                })?;
                
                let result = rt.block_on(async {
                    client_guard.execute(sql, params).await
                }).map_err(|e| RustixError::QueryError(e.to_string()))?;
                Ok(result)
            }

            #[cfg(feature = "mysql")]
            ConnectionPool::MySQL(pool) => {
                let mut conn = pool
                    .get_conn()
                    .map_err(|e| RustixError::QueryError(e.to_string()))?;
                let _result = conn
                    .exec_drop(sql, ())
                    .map_err(|e| RustixError::QueryError(e.to_string()))?;
                Ok(1) // MySQL doesn't return rows affected reliably for exec_drop
            }

            #[cfg(feature = "rusqlite")]
            ConnectionPool::SQLite(conn) => {
                let conn_guard = conn.lock().map_err(|e| {
                    RustixError::ConnectionError(format!("Failed to acquire lock on SQLite connection: {}", e))
                })?;
                let result = conn_guard
                    .execute(sql, [])
                    .map_err(|e| RustixError::QueryError(e.to_string()))?;
                Ok(result as u64)
            }

            ConnectionPool::None => {
                Err(RustixError::ConnectionError("No active database connection".to_string()))
            }

            #[allow(unreachable_patterns)]
            _ => Err(RustixError::ConnectionError("Unsupported database type".to_string())),
        }
    }

    /// Executes a raw SQL query and returns the results as a vector of deserialized objects.
    pub fn query_raw<T>(&self, sql: &str, params: &[&(dyn ToSql + Sync + 'static)]) -> Result<Vec<T>, RustixError>
    where
        T: for<'de> serde::Deserialize<'de> + Debug,
    {
        match &self.pool {
            #[cfg(feature = "postgres")]
            ConnectionPool::PostgreSQL(client, rt) => {
                let client_guard = client.lock().map_err(|e| {
                    RustixError::TransactionError(format!("Failed to acquire lock on connection: {}", e))
                })?;
                let rows = rt.block_on(async {
                    client_guard.query(sql, params).await
                }).map_err(|e| RustixError::QueryError(e.to_string()))?;

                let mut models = Vec::with_capacity(rows.len());
                for row in rows {
                    let mut json_obj = serde_json::Map::new();
                    for column in row.columns() {
                        let name = column.name();
                        let value = crate::transaction_manager::pg_row_value_to_json(&row, column).unwrap_or(serde_json::Value::String(String::from("Null")));
                        json_obj.insert(name.to_string(), value);
                    }
                    let model = serde_json::from_value(serde_json::Value::Object(json_obj))
                        .map_err(|e| RustixError::SerializationError(e.to_string()))?;
                    models.push(model);
                }
                Ok(models)
            }

            #[cfg(feature = "mysql")]
            ConnectionPool::MySQL(pool) => {
                let mut conn = pool
                    .get_conn()
                    .map_err(|e| RustixError::QueryError(e.to_string()))?;

                let rows: Vec<Result<T, mysql::Error>> = conn
                    .query_map(sql, |row: mysql::Row| {
                        let mut json_obj = serde_json::Map::new();
                        let columns = row.columns_ref();

                        for (i, column) in columns.iter().enumerate() {
                            let name = column.name_str().to_string();
                            let value = crate::transaction_manager::mysql_row_value_to_json(&row, i, column.column_type())
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

            #[cfg(feature = "rusqlite")]
            ConnectionPool::SQLite(conn) => {
                let conn_guard = conn.lock().map_err(|e| {
                    RustixError::ConnectionError(format!("Failed to acquire lock on SQLite connection: {}", e))
                })?;

                let mut stmt = conn_guard
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
                            let value = crate::transaction_manager::sqlite_row_value_to_json(row, i)
                                .unwrap_or(serde_json::Value::Null);
                            json_obj.insert(name.clone(), value);
                        }
                        let model = serde_json::from_value(serde_json::Value::Object(json_obj)).map_err(
                            |e| rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(e)),
                        )?;
                        Ok(model)
                    })
                    .map_err(|e| RustixError::QueryError(e.to_string()))?
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|e| RustixError::QueryError(e.to_string()))?;

                Ok(models)
            }

            ConnectionPool::None => {
                Err(RustixError::ConnectionError("No active database connection".to_string()))
            }

            #[allow(unreachable_patterns)]
            _ => Err(RustixError::ConnectionError("Unsupported database type".to_string())),
        }
    }

    /// Executes a transaction using the provided transaction function.
    pub async fn transaction<F, R>(&self, transaction_fn: F) -> Result<R, RustixError>
    where
        F: FnOnce(&dyn TransactionExecutor) -> Result<R, RustixError> + Send + 'static,
        R: Send + 'static,
    {
        match &self.pool {
            #[cfg(feature = "postgres")]
            ConnectionPool::PostgreSQL(client, _) => {
                run_postgres_transaction(&client.clone(), transaction_fn).await
            }

            #[cfg(feature = "mysql")]
            ConnectionPool::MySQL(pool) => {
                run_mysql_transaction(&pool.clone(), transaction_fn)
            }

            #[cfg(feature = "rusqlite")]
            ConnectionPool::SQLite(conn) => {
                run_sqlite_transaction(&conn.clone(), transaction_fn)
            }

            ConnectionPool::None => {
                Err(RustixError::ConnectionError("No active database connection".to_string()))
            }

            #[allow(unreachable_patterns)]
            _ => Err(RustixError::ConnectionError("Unsupported database type".to_string())),
        }
    }

    /// Returns a reference to the database type.
    pub fn get_db_type(&self) -> &DatabaseType {
        &self.db_type
    }
}