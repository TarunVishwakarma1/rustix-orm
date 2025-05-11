/// The main library module for the Rusticx ORM.
///
/// This module provides the core functionality for interacting with various databases
/// through a unified interface. It includes connection management, error handling,
/// and transaction management.
mod connection;
mod model;
// mod query_builder;
// mod migrations;
mod error;
mod sql_types;
mod transaction_manager;

/// Re-exporting types for easier access by users of the library.
pub use connection::{Connection, DatabaseType}; // Re-exporting connection-related types
pub use model::{SQLModel, ModelAttribute, ToSqlConvert}; // Re-exporting model-related types
// pub use query_builder::QueryBuilder;
pub use error::RusticxError; // Re-exporting the RusticxError type for error handling
// pub use migrations::{Migration, MigrationManager};
pub use sql_types::SqlType; // Re-exporting SQL type definitions
#[cfg(feature = "mysql")]
pub use transaction_manager::MySQLTransactionExecutor; // Re-exporting MySQL transaction executor
#[cfg(feature = "rusqlite")]
pub use transaction_manager::SQLiteTransactionExecutor; // Re-exporting SQLite transaction executor
#[cfg(feature = "postgres")]
pub use transaction_manager::PostgresTransactionExecutor; // Re-exporting PostgreSQL transaction executor
#[cfg(feature = "postgres")]
pub use postgres::types::ToSql as PostgresToSql; // Re-exporting PostgreSQL ToSql trait
