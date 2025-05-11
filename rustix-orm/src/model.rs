use std::fmt::Debug;
use serde::{Deserialize, Serialize};
use crate::connection::{Connection, DatabaseType};
use crate::error::RustixError;


#[cfg(feature = "rusqlite")]
use rusqlite::types::ToSql as RusqliteToSql;
#[cfg(feature = "mysql")]
use mysql::prelude::ToValue as MysqlToSql;

use std::any::Any;

// Re-export the ToSql trait from the postgres crate if enabled.
// This trait is used in method signatures for database parameters.
#[cfg(feature = "postgres")]
pub use postgres::types::ToSql;

// Define a placeholder ToSql trait if postgres feature is not enabled.
// This allows the code to compile, but database interaction relying on
// this trait in signatures will only work with the postgres feature.
#[cfg(not(feature = "postgres"))]
pub trait ToSql {}


/// A trait for database models providing common CRUD operations.
///
/// This trait requires implementing several methods to define the model's
/// structure and how it interacts with the database.
pub trait SQLModel: Sized + Debug + Serialize + for<'de> Deserialize<'de> {
    /// Returns the name of the database table for this model.
    fn table_name() -> String;

    /// Returns the name of the primary key field.
    fn primary_key_field() -> String;

    /// Returns the value of the primary key for the current model instance.
    /// Returns `None` if the model has not been inserted yet.
    fn primary_key_value(&self) -> Option<i32>;

    /// Sets the primary key value for the current model instance.
    fn set_primary_key(&mut self, id: i32);

    /// Returns the SQL statement to create the table for this model
    /// for a given database type.
    fn create_table_sql(db_type: &DatabaseType) -> String;

    /// Returns a list of all field names in the model,
    /// typically corresponding to database columns.
    fn field_names() -> Vec<&'static str>;

    /// Returns a vector of boxed values for all fields, excluding the primary key,
    /// intended for use as SQL parameters. Each value must implement `ToSqlConvert`.
    fn to_sql_field_values(&self) -> Vec<Box<dyn ToSqlConvert>>;

    /// Converts a database row represented as a JSON Value (Map) into a model instance.
    fn from_row(row: &serde_json::Value) -> Result<Self, RustixError>;

    /// Inserts a new record into the database table based on the model instance.
    ///
    /// This method handles auto-increment primary keys by:
    /// 1. Including the primary key in the INSERT if the user provided a value
    /// 2. Excluding the primary key if it's None, letting the database generate it
    /// 3. Setting the generated primary key on the model instance after insertion
    fn insert(&mut self, conn: &Connection) -> Result<(), RustixError> {
        let fields = Self::field_names();
        let primary_key_field = Self::primary_key_field();
        let field_values = self.to_sql_field_values();
        
        // Find the primary key field index
        let pk_idx = fields.iter().position(|f| *f == primary_key_field);
        
        // Check if we should include the primary key in the INSERT
        let include_pk = if let Some(idx) = pk_idx {
            // Include PK if it has a value (not None)
            !field_values[idx].is_null()
        } else {
            // No PK field found, include all fields
            true
        };
        
        // Filter fields based on whether to include PK
        let insert_fields: Vec<&'static str> = if include_pk {
            fields.clone()
        } else {
            fields.iter()
                .filter(|&f| *f != primary_key_field)
                .copied()
                .collect()
        };
        
        // Skip the insert if there are no fields to insert
        if insert_fields.is_empty() {
            return Err(RustixError::QueryError("No fields to insert".to_string()));
        }
        
        // Generate SQL placeholders based on the database type
        let placeholders: Vec<String> = match conn.get_db_type() {
            DatabaseType::PostgreSQL => (1..=insert_fields.len()).map(|i| format!("${}", i)).collect(),
            _ => (0..insert_fields.len()).map(|_| "?".to_string()).collect()
        };
        
        let sql = format!(
            "INSERT INTO {} ({}) VALUES ({})",
            Self::table_name(),
            insert_fields.join(", "),
            placeholders.join(", ")
        );
        
        // Prepare parameters, filtering out the primary key if needed
        let mut params: Vec<&(dyn ToSql + Sync + 'static)> = Vec::new();
        
        for (idx, field_name) in fields.iter().enumerate() {
            if insert_fields.contains(field_name) {
                if let Some(sql_convert) = field_values[idx].as_ref_postgres() {
                    params.push(sql_convert);
                } else {
                    return Err(RustixError::QueryError(format!(
                        "Failed to convert field '{}' value to database-compatible type",
                        field_name
                    )));
                }
            }
        }
        
        // Execute the query
        conn.execute(&sql, &params)?;
        
        // If PK is not included in the insert, get the last inserted ID
        if !include_pk {
            if let Some(_) = pk_idx {
                let last_id_sql = match conn.get_db_type() {
                    DatabaseType::PostgreSQL => "SELECT lastval() as id".to_string(),
                    DatabaseType::MySQL => "SELECT LAST_INSERT_ID() as id".to_string(),
                    DatabaseType::SQLite => "SELECT last_insert_rowid() as id".to_string(),
                };
                
                #[derive(Deserialize, Debug)]
                struct IdRow {
                    id: i64,
                }
                
                let ids: Vec<IdRow> = conn.query_raw(&last_id_sql, &[])?;
                if let Some(id_row) = ids.first() {
                    self.set_primary_key(id_row.id as i32);
                } else {
                    return Err(RustixError::QueryError("Failed to retrieve last inserted ID".to_string()));
                }
            }
        }
        
        Ok(())
    }

    /// Updates an existing record in the database table based on the model instance's primary key.
    ///
    /// Requires the model instance to have a primary key value set.
    fn update(&self, conn: &Connection) -> Result<(), RustixError> {
        let id = self.primary_key_value().ok_or_else(|| {
            RustixError::QueryError("Cannot update a model without a primary key value".to_string())
        })?;

        let fields = Self::field_names();
        let primary_key_field = Self::primary_key_field();

        // Generate SET clause for the UPDATE statement, excluding the primary key
        let field_params: Vec<String> = fields.iter()
            .filter(|&f| *f != &primary_key_field)
            .enumerate()
            .map(|(i, f)| {
                match conn.get_db_type() {
                    // PostgreSQL parameters are 1-indexed
                    DatabaseType::PostgreSQL => format!("{} = ${}", f, i + 1),
                    // Other databases use ?
                    _ => format!("{} = ?", f)
                }
            })
            .collect();

        // Generate WHERE clause using the primary key
        let where_clause = match conn.get_db_type() {
            // PostgreSQL parameter for WHERE clause comes after all SET parameters
            DatabaseType::PostgreSQL => format!("{} = ${}", primary_key_field, field_params.len() + 1),
            _ => format!("{} = ?", primary_key_field)
        };

        let sql = format!(
            "UPDATE {} SET {} WHERE {}",
            Self::table_name(),
            field_params.join(", "),
            where_clause
        );

        // Prepare parameters: values for SET clause followed by the primary key value
        let mut params: Vec<&(dyn ToSql + Sync + 'static)> = Vec::new();
        let field_values = self.to_sql_field_values();

        for (i, field) in Self::field_names().iter().enumerate() {
            if *field != &primary_key_field {
                 // as_ref_postgres is expected to return a reference to dyn ToSql + Sync + 'static
                if let Some(sql_value) = field_values[i].as_ref_postgres() {
                    params.push(sql_value);
                } else {
                    // This error indicates a failure in the model's to_sql_field_values implementation
                    return Err(RustixError::QueryError(format!("Failed to convert field '{}' value to database-compatible type", field)));
                }
            }
        }

        // Add the primary key as the last parameter for the WHERE clause
        // Assumes i32 implements the necessary ToSql, Sync, and 'static bounds.
        // An explicit cast is used for clarity and safety.
        let id_param = &id;
        params.push(id_param as &(dyn ToSql + Sync + 'static));

        conn.execute(&sql, &params)?;

        Ok(())
    }

    /// Finds a single record by its primary key.
    /// Returns `Ok(model)` if found, `Err(RustixError::NotFound)` if not found.
    fn find_by_id(conn: &Connection, id: i32) -> Result<Self, RustixError> {
        let primary_key_field = Self::primary_key_field();
        // Use database-specific placeholder syntax
        #[cfg(feature = "postgres")]
        let sql = format!(
            "SELECT * FROM {} WHERE {} = $1",
            Self::table_name(),
            primary_key_field
        );

        #[cfg(not(feature = "postgres"))]
        let sql = format!(
            "SELECT * FROM {} WHERE {} = ?",
            Self::table_name(),
            primary_key_field
        );

        // Prepare parameters using dyn ToSql
        let params: Vec<&(dyn ToSql + Sync + 'static)> = vec![&id];

        // Attempt direct deserialization from the database result first
        // This is generally more efficient if supported by the underlying driver.
        let results: Result<Vec<Self>, _> = conn.query_raw(&sql, &params);

        match results {
            Ok(mut models) => {
                // Check if any rows were returned
                if let Some(model) = models.pop() { // Use pop to get the single model
                    Ok(model)
                } else {
                    // No rows returned, record not found
                    Err(RustixError::NotFound(format!("{} with id {} not found", Self::table_name(), id)))
                }
            },
            Err(e) => {
                // If direct deserialization failed, fallback to manual row processing
                // Log the original error for debugging purposes in a production environment
                eprintln!("Direct deserialization failed for find_by_id: {:?}", e);

                let rows: Vec<serde_json::Map<String, serde_json::Value>> = conn.query_raw(&sql, &params)?;

                // Process the fallback result
                if let Some(row) = rows.first() {
                    Self::from_row(&serde_json::Value::Object(row.clone()))
                } else {
                    // Still no rows in fallback, record not found
                    Err(RustixError::NotFound(format!("{} with id {} not found", Self::table_name(), id)))
                }
            }
        }
    }

    /// Finds all records in the table.
    fn find_all(conn: &Connection) -> Result<Vec<Self>, RustixError> {
        let sql = format!("SELECT * FROM {}", Self::table_name());
        // No parameters for SELECT all
        let params: &[&(dyn ToSql + Sync + 'static)] = &[];

        // Attempt direct deserialization from the database result first
        let direct_results: Result<Vec<Self>, _> = conn.query_raw(&sql, params);

        match direct_results {
            Ok(models) => {
                // Direct deserialization successful
                Ok(models)
            },
            Err(e) => {
                // If direct deserialization failed, fallback to manual row processing
                 // Log the original error for debugging purposes in a production environment
                eprintln!("Direct deserialization failed for find_all: {:?}", e);

                let rows: Vec<serde_json::Map<String, serde_json::Value>> = conn.query_raw(&sql, params)?;

                // Manually deserialize each row
                let mut models = Vec::with_capacity(rows.len());
                for row in rows {
                    models.push(Self::from_row(&serde_json::Value::Object(row))?);
                }

                Ok(models)
            }
        }
    }

    /// Deletes the current record from the database.
    ///
    /// Requires the model instance to have a primary key value set.
    fn delete(&self, conn: &Connection) -> Result<(), RustixError> {
        if let Some(id) = self.primary_key_value() {
            Self::delete_by_id(conn, id)
        } else {
            Err(RustixError::ValidationError("Cannot delete a record without a primary key value".to_string()))
        }
    }

    /// Deletes a record by its primary key.
    fn delete_by_id(conn: &Connection, id: i32) -> Result<(), RustixError> {
        let primary_key_field = Self::primary_key_field();
        // Use database-specific placeholder syntax
        #[cfg(feature = "postgres")]
        let sql = format!(
            "DELETE FROM {} WHERE {} = $1",
            Self::table_name(),
            primary_key_field
        );

        #[cfg(not(feature = "postgres"))]
         let sql = format!(
            "DELETE FROM {} WHERE {} = ?",
            Self::table_name(),
            primary_key_field
        );

        // Prepare parameters using dyn ToSql
        let params: Vec<&(dyn ToSql + Sync + 'static)> = vec![&id];

        conn.execute(&sql, &params)?;

        Ok(())
    }

    /// Finds records based on a single field's value.
    ///
    /// The value must implement `Debug`, `Any`, `Sync`, and `Send`.
    /// Note: This method uses `Any` downcasting, which can be less ergonomic
    /// than a dedicated query builder.
    fn find_by<T: Debug + Any + Sync + Send + 'static>(
        conn: &Connection,
        field: &str,
        value: &T,
    ) -> Result<Vec<Self>, RustixError> {
        // Basic validation for field name (could be more robust)
        if field.contains('"') || field.contains('\'') || field.contains(' ') {
             return Err(RustixError::QueryError(format!("Invalid characters in field name: {}", field)));
        }

        // Use database-specific placeholder syntax
        #[cfg(feature = "postgres")]
        let sql = format!(
            "SELECT * FROM {} WHERE {} = $1",
            Self::table_name(),
            field
        );

        #[cfg(not(feature = "postgres"))]
        let sql = format!(
            "SELECT * FROM {} WHERE {} = ?",
            Self::table_name(),
            field
        );

        let mut params: Vec<&(dyn ToSql + Sync + 'static)> = Vec::new();
        let any_value = value as &dyn Any;

        // Attempt to downcast the value to common SQL types and push as dyn ToSql
        if let Some(v) = any_value.downcast_ref::<i32>() {
             params.push(v as &(dyn ToSql + Sync + 'static));
        } else if let Some(v) = any_value.downcast_ref::<String>() {
             params.push(v as &(dyn ToSql + Sync + 'static));
        } else if let Some(v) = any_value.downcast_ref::<&str>() {
             params.push(v as &(dyn ToSql + Sync + 'static));
        } else if let Some(v) = any_value.downcast_ref::<i64>() {
             params.push(v as &(dyn ToSql + Sync + 'static));
        } else if let Some(v) = any_value.downcast_ref::<f64>() {
             params.push(v as &(dyn ToSql + Sync + 'static));
        } else if let Some(v) = any_value.downcast_ref::<bool>() {
             params.push(v as &(dyn ToSql + Sync + 'static));
        // Add more type checks for other supported types (e.g., dates, byte arrays)
        } else {
            return Err(RustixError::QueryError(format!("Unsupported parameter type for field '{}'", field)));
        }


        // Attempt direct deserialization first
        let direct_results: Result<Vec<Self>, _> = conn.query_raw(&sql, &params);

        match direct_results {
            Ok(models) => Ok(models),
            Err(e) => {
                 // Log the original error for debugging purposes in a production environment
                eprintln!("Direct deserialization failed for find_by: {:?}", e);
                // Fallback to manual row processing
                let rows: Vec<serde_json::Map<String, serde_json::Value>> = conn.query_raw(&sql, &params)?;

                let mut models = Vec::with_capacity(rows.len());
                for row in rows {
                    models.push(Self::from_row(&serde_json::Value::Object(row))?);
                }

                Ok(models)
            }
        }
    }

    /// Executes a raw SQL query and attempts to deserialize the results into models.
    ///
    /// Use with caution, as raw SQL can be less safe if not carefully constructed.
    /// Parameters should be provided as a slice of references to types implementing `ToSql + Sync + 'static`.
    fn find_with_sql(conn: &Connection, sql: &str, params: &[&(dyn ToSql + Sync + 'static)]) -> Result<Vec<Self>, RustixError> {
        // Attempt direct deserialization first
        let direct_results: Result<Vec<Self>, _> = conn.query_raw(sql, params);

        match direct_results {
            Ok(models) => Ok(models),
            Err(e) => {
                // If direct deserialization failed, fallback to manual row processing
                 // Log the original error for debugging purposes in a production environment
                eprintln!("Direct deserialization failed for find_with_sql: {:?}", e);
                let rows: Vec<serde_json::Map<String, serde_json::Value>> = conn.query_raw(sql, params)?;

                let mut models = Vec::with_capacity(rows.len());
                for row in rows {
                    models.push(Self::from_row(&serde_json::Value::Object(row))?);
                }

                Ok(models)
            }
        }
    }

    /// Counts the number of records in the table.
    fn count(conn: &Connection) -> Result<i64, RustixError> {
        let sql = format!("SELECT COUNT(*) as count FROM {}", Self::table_name());

        #[derive(Deserialize, Debug)]
        struct CountResult {
            count: i64,
        }

        // No parameters for count query
        let params: &[&(dyn ToSql + Sync + 'static)] = &[];
        let counts: Vec<CountResult> = conn.query_raw(&sql, params)?;

        if let Some(count_result) = counts.first() {
            Ok(count_result.count)
        } else {
            // Should ideally always return one row with count 0 if table is empty
            Ok(0)
        }
    }
}

/// Helper trait to bridge the gap between specific model field types and `dyn ToSql`.
///
/// Implementations for specific types should provide a reference to
/// `dyn ToSql + Sync + 'static` which is compatible with the `Connection`'s methods.
/// The name `as_ref_postgres` is retained from the original code but is intended
/// to provide a generic `dyn ToSql` reference compatible with the `postgres` crate's
/// `ToSql` trait when the feature is enabled, and potentially a compatible trait
/// for other databases if implemented.

pub trait ToSqlConvert: Debug + Sync + Send {
    /// Returns a reference to the value as `dyn ToSql + Sync + 'static` for PostgreSQL.
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)>;
    
    /// Checks if the value is null (for Option types).
    fn is_null(&self) -> bool {
        false
    }
}

// Add implementation for Option types
impl<T: ToSqlConvert + Clone> ToSqlConvert for Option<T> {
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)> {
        match self {
            Some(inner) => inner.as_ref_postgres(),
            None => None,
        }
    }
    
    fn is_null(&self) -> bool {
        self.is_none()
    }
}

impl<T: ToSqlConvert + ?Sized> ToSqlConvert for Box<T> {
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)> {
        (**self).as_ref_postgres()
    }
    
    fn is_null(&self) -> bool {
        (**self).is_null()
    }
}
impl ToSqlConvert for String {
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)> {
        Some(self)
    }
}

// Implementation for i32
impl ToSqlConvert for i32 {
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)> {
        Some(self)
    }
}

// Implementation for i64
impl ToSqlConvert for i64 {
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)> {
        Some(self)
    }
}

// Implementation for bool
impl ToSqlConvert for bool {
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)> {
        Some(self)
    }
}

// Implementation for f64
impl ToSqlConvert for f64 {
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)> {
        Some(self)
    }
}

// Implementation for NaiveDateTime
impl ToSqlConvert for chrono::NaiveDateTime {
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)> {
        Some(self)
    }
}

// Implementation for UUID if you use it
#[cfg(feature = "uuid")]
impl ToSqlConvert for uuid::Uuid {
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)> {
        Some(self)
    }
}

// Implementation for Vec<u8> (for blob data)
impl ToSqlConvert for Vec<u8> {
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)> {
        Some(self)
    }
}

// Add implementations for other types as needed
// For NaiveDate
impl ToSqlConvert for chrono::NaiveDate {
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)> {
        Some(self)
    }
}

// For NaiveTime
impl ToSqlConvert for chrono::NaiveTime {
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)> {
        Some(self)
    }
}

// TODO: Add implementations for other database drivers if needed.
// The current ToSqlConvert and as_ref_postgres design is heavily tied
// to the postgres crate's ToSql trait. For true multi-database support,
// a more generic approach or conditional compilation within ToSqlConvert
// implementations would be required to handle different database drivers'
// parameter traits (e.g., RusqliteToSql, MysqlToSql).

#[derive(Debug, Clone)]
pub enum ModelAttribute {
    PrimaryKey,
    Column(String),
    Default(String),
    Nullable,
    Index(bool), // true for unique index
    Foreign(String, String), // References table_name(column_name)
}