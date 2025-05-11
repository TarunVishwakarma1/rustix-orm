mod connection;
mod model;
mod query_builder;
// mod migrations;
mod error;
mod sql_types;
mod transaction_manager;


pub use connection::{Connection, DatabaseType}; // <-- Add this line
pub use model::{SQLModel, ModelAttribute, ToSqlConvert};
pub use query_builder::QueryBuilder;
pub use error::RustixError;
// pub use migrations::{Migration, MigrationManager};
pub use sql_types::SqlType;
#[cfg(feature = "mysql")]
pub use transaction_manager::MySQLTransactionExecutor;
#[cfg(feature = "rusqlite")]
pub use transaction_manager::SQLiteTransactionExecutor;
#[cfg(feature = "postgres")]
pub use transaction_manager::PostgresTransactionExecutor;
