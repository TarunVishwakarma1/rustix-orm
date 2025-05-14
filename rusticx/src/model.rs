use std::fmt::Debug;
use serde::{Deserialize, Serialize};
use crate::connection::{Connection, DatabaseType};
use crate::error::RusticxError;

// Required for find_by method using Any downcasting
use std::any::Any;

// Re-export the ToSql trait from the postgres crate if enabled.
// This trait is central to the parameter binding used in Connection and SQLModel.
#[cfg(feature = "postgres")]
pub use postgres::types::ToSql;

// Define a placeholder ToSql trait if postgres feature is not enabled.
// Code requiring `dyn ToSql` will compile but effectively only work
// when the postgres feature is enabled, due to the design of ToSqlConvert
// and the expected parameter types in Connection.
#[cfg(not(feature = "postgres"))]
pub trait ToSql {}


/// A trait for database models providing common CRUD operations.
///
/// Implement this trait for your structs to give them basic database
/// persistence capabilities. Requires models to be `Debug`, `Serialize`,
/// and `Deserialize` for handling data conversion.
pub trait SQLModel: Sized + Debug + Serialize + for<'de> Deserialize<'de> {
    /// Returns the name of the database table for this model.
    ///
    /// This should be a static string or derived from the model name.
    fn table_name() -> String;

    /// Returns the name of the primary key field.
    ///
    /// This field is used for `find_by_id`, `update`, and `delete`.
    fn primary_key_field() -> String;

    /// Returns the value of the primary key for the current model instance.
    ///
    /// Returns `None` if the model instance has not been inserted yet
    /// or its primary key was not set.
    fn primary_key_value(&self) -> Option<i32>;

    /// Sets the primary key value for the current model instance.
    ///
    /// This is typically called after a successful `insert` operation
    /// to populate the auto-generated ID.
    fn set_primary_key(&mut self, id: i32);

    /// Returns the SQL statement to create the table for this model
    /// for a given database type.
    ///
    /// This method is crucial for schema management or initial setup.
    fn create_table_sql(db_type: &DatabaseType) -> String;

    /// Returns a list of all field names in the model,
    /// typically corresponding to database columns.
    ///
    /// The order should match the order of values returned by
    /// `to_sql_field_values`.
    fn field_names() -> Vec<&'static str>;

    /// Returns a vector of boxed values for all fields.
    ///
    /// Each value must be boxed (`Box<dyn ToSqlConvert>`) and implement
    /// the `ToSqlConvert` trait, which bridges the model's native types
    /// to the database driver's parameter type (`dyn ToSql` in this case).
    /// The order of values *must* match the order of field names
    /// returned by `field_names`.
    fn to_sql_field_values(&self) -> Vec<Box<dyn ToSqlConvert>>;

    /// Converts a database row represented as a JSON Value (Map) into a model instance.
    ///
    /// This is used as a fallback deserialization mechanism if the `Connection`'s
    /// `query_raw` method doesn't directly deserialize into the model type `Self`.
    fn from_row(row: &serde_json::Value) -> Result<Self, RusticxError>;

    /// Inserts a new record into the database table based on the model instance.
    ///
    /// If the model instance's primary key value is `None`, it assumes the
    /// database handles auto-increment and attempts to retrieve the last
    /// inserted ID after the insert, setting it on the model instance.
    /// If the primary key value is `Some`, it includes the primary key
    /// in the INSERT statement.
    fn insert(&mut self, conn: &Connection) -> Result<(), RusticxError> {
        let fields = Self::field_names();
        let primary_key_field = Self::primary_key_field();
        let field_values = self.to_sql_field_values();

        // Find the primary key field index and check if PK should be included in INSERT
        let pk_idx = fields.iter().position(|f| *f == primary_key_field);
        let include_pk = if let Some(idx) = pk_idx {
            // Include PK if the corresponding value is NOT null (user provided it)
            !field_values.get(idx).map_or(true, |v| v.is_null()) // Handle case where pk_idx is found but field_values is shorter
        } else {
            // No PK field found in fields, include all (which is fields itself)
            true
        };

        // Filter fields and values based on whether to include PK
        let (insert_fields, insert_values): (Vec<&'static str>, Vec<Box<dyn ToSqlConvert>>) = fields.into_iter()
            .zip(field_values.into_iter())
            .filter(|(field_name, _)| include_pk || *field_name != primary_key_field)
            .unzip();

        // Skip the insert if there are no fields to insert
        if insert_fields.is_empty() {
            return Err(RusticxError::QueryError("No fields to insert".to_string()));
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

        // Prepare parameters as references to dyn ToSql + Sync + 'static
        let params: Vec<&(dyn ToSql + Sync + 'static)> = insert_values.iter()
             .filter_map(|v| v.as_ref_postgres()) // Use filter_map to handle Option values
            .collect();

         // Ensure the number of parameters matches the number of placeholders
        if params.len() != insert_fields.len() {
            // This indicates an issue in ToSqlConvert implementations not returning Some(_)
             return Err(RusticxError::QueryError(format!(
                "Parameter count mismatch: expected {} but got {}. Check ToSqlConvert implementations.",
                insert_fields.len(),
                params.len()
            )));
        }


        // Execute the query
        conn.execute(&sql, &params)?;

        // If PK was not included in the insert, get the last inserted ID and set it
        if !include_pk {
            if let Some(_) = pk_idx { // Check if PK field was defined at all
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
                    // This should not happen if the insert was successful and table has auto-increment
                    return Err(RusticxError::QueryError("Failed to retrieve last inserted ID".to_string()));
                }
            } else {
                 // This case implies PK field was defined but not found in field_names,
                 // or there's no auto-increment PK setup handled.
                 // Depending on the model structure, this might be an error.
                 // For now, we assume if !include_pk, it's an auto-increment scenario.
            }
        }

        Ok(())
    }

    /// Updates an existing record in the database table based on the model instance's primary key.
    ///
    /// Requires the model instance to have a primary key value set (`primary_key_value()`).
    fn update(&self, conn: &Connection) -> Result<(), RusticxError> {
        let id = self.primary_key_value().ok_or_else(|| {
            RusticxError::QueryError("Cannot update a model without a primary key value".to_string())
        })?;

        let fields = Self::field_names();
        let primary_key_field = Self::primary_key_field();
        let field_values = self.to_sql_field_values();

        // Collect fields and values, excluding the primary key field
        let update_fields_values: Vec<(&'static str, Box<dyn ToSqlConvert>)> = fields.into_iter()
            .zip(field_values.into_iter())
            .filter(|(field_name, _)| *field_name != primary_key_field)
            .collect();

         // Skip update if there are no non-PK fields to update
        if update_fields_values.is_empty() {
            return Ok(()); // No fields to update, return Ok
        }


        // Generate SET clause for the UPDATE statement
        let field_params: Vec<String> = update_fields_values.iter()
            .enumerate()
            .map(|(i, (field_name, _))| {
                match conn.get_db_type() {
                    // PostgreSQL parameters are 1-indexed
                    DatabaseType::PostgreSQL => format!("{} = ${}", field_name, i + 1),
                    // Other databases use ?
                    _ => format!("{} = ?", field_name)
                }
            })
            .collect();

        // Generate WHERE clause using the primary key.
        // The primary key parameter index depends on the number of SET parameters.
        let where_clause = match conn.get_db_type() {
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
        let mut params: Vec<&(dyn ToSql + Sync + 'static)> = update_fields_values.iter()
            .filter_map(|(_, value)| value.as_ref_postgres()) // Use filter_map for values
            .collect();

        // Add the primary key as the last parameter for the WHERE clause
        // Assumes i32 implements the necessary ToSql, Sync, and 'static bounds via a ToSqlConvert implementation.
        // An explicit cast is used for clarity and safety, assuming `&id` can be cast to `dyn ToSql`.
         let id_param = &id as &(dyn ToSql + Sync + 'static); // Cast &i32 to the required trait object
        params.push(id_param);

         // Ensure parameter count matches generated placeholders + PK
         if params.len() != update_fields_values.len() + 1 {
             return Err(RusticxError::QueryError(format!(
                "Parameter count mismatch for update: expected {} but got {}. Check ToSqlConvert implementations.",
                update_fields_values.len() + 1,
                params.len()
            )));
         }


        conn.execute(&sql, &params)?;

        Ok(())
    }

    /// Finds a single record by its primary key.
    ///
    /// Returns `Ok(model)` if a record with the given ID is found.
    /// Returns `Err(RusticxError::NotFound)` if no record is found.
    /// Returns `Err(RusticxError::QueryError)` or other errors on database issues.
    fn find_by_id(conn: &Connection, id: i32) -> Result<Self, RusticxError> {
        let primary_key_field = Self::primary_key_field();
        // Use database-specific placeholder syntax
        #[cfg(feature = "postgres")]
        let sql = format!(
            "SELECT * FROM {} WHERE {} = $1 LIMIT 1", // Added LIMIT 1 for efficiency
            Self::table_name(),
            primary_key_field
        );

        #[cfg(not(feature = "postgres"))]
        let sql = format!(
            "SELECT * FROM {} WHERE {} = ? LIMIT 1", // Added LIMIT 1 for efficiency
            Self::table_name(),
            primary_key_field
        );

        // Prepare parameters using dyn ToSql. &id needs to be cast to the trait object.
        let id_param = &id as &(dyn ToSql + Sync + 'static); // Cast &i32 to the required trait object
        let params: &[&(dyn ToSql + Sync + 'static)] = &[id_param];

        // Attempt direct deserialization from the database result first
        let results: Result<Vec<Self>, RusticxError> = conn.query_raw(&sql, params);

        match results {
            Ok(mut models) => {
                // Check if any rows were returned
                if let Some(model) = models.pop() { // Use pop to get the single model
                    Ok(model)
                } else {
                    // No rows returned, record not found
                    Err(RusticxError::NotFound(format!("{} with id {} not found", Self::table_name(), id)))
                }
            },
            Err(e) => {
                // If direct deserialization failed, fallback to manual row processing
                // In a production environment, you might log 'e' here for debugging
                eprintln!("Warning: Direct deserialization failed for find_by_id, falling back to manual row processing: {:?}", e);

                let rows: Vec<serde_json::Map<String, serde_json::Value>> = conn.query_raw(&sql, params)?;

                // Process the fallback result
                if let Some(row) = rows.first() {
                    Self::from_row(&serde_json::Value::Object(row.clone()))
                } else {
                    // Still no rows in fallback, record not found
                    Err(RusticxError::NotFound(format!("{} with id {} not found", Self::table_name(), id)))
                }
            }
        }
    }

    /// Finds all records in the table.
    ///
    /// Returns a vector of all model instances found in the table.
    fn find_all(conn: &Connection) -> Result<Vec<Self>, RusticxError> {
        let sql = format!("SELECT * FROM {}", Self::table_name());
        // No parameters for SELECT all
        let params: &[&(dyn ToSql + Sync + 'static)] = &[];

        // Attempt direct deserialization from the database result first
        let direct_results: Result<Vec<Self>, RusticxError> = conn.query_raw(&sql, params);

        match direct_results {
            Ok(models) => {
                // Direct deserialization successful
                Ok(models)
            },
            Err(e) => {
                // If direct deserialization failed, fallback to manual row processing
                // In a production environment, you might log 'e' here for debugging
                eprintln!("Warning: Direct deserialization failed for find_all, falling back to manual row processing: {:?}", e);

                let rows: Vec<serde_json::Map<String, serde_json::Value>> = conn.query_raw(&sql, params)?;

                // Manually deserialize each row using from_row
                rows.into_iter().map(|row| Self::from_row(&serde_json::Value::Object(row))).collect()
            }
        }
    }

    /// Deletes the current record from the database using its primary key.
    ///
    /// Requires the model instance to have a primary key value set (`primary_key_value()`).
    /// Returns `Err(RusticxError::ValidationError)` if the primary key is not set.
    fn delete(&self, conn: &Connection) -> Result<(), RusticxError> {
        if let Some(id) = self.primary_key_value() {
            Self::delete_by_id(conn, id)
        } else {
            Err(RusticxError::ValidationError("Cannot delete a record without a primary key value".to_string()))
        }
    }

    /// Deletes a record by its primary key.
    ///
    /// Returns `Ok(())` on success.
    fn delete_by_id(conn: &Connection, id: i32) -> Result<(), RusticxError> {
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

        // Prepare parameters using dyn ToSql. &id needs to be cast.
        let id_param = &id as &(dyn ToSql + Sync + 'static); // Cast &i32 to the required trait object
        let params: &[&(dyn ToSql + Sync + 'static)] = &[id_param];

        conn.execute(&sql, &params)?;

        Ok(())
    }

    /// Finds records based on a single field's value.
    ///
    /// This method uses `std::any::Any` downcasting to handle parameter
    /// types. This approach is less type-safe and less ergonomic than
    /// a dedicated query builder. Only a limited set of types are
    /// supported for the `value` parameter (i32, String, &str, i64, f64, bool, etc.,
    /// as implemented in `ToSqlConvert`).
    ///
    /// Basic validation is performed on the `field` name.
    fn find_by<T: Debug + Any + Sync + Send + 'static>(
        conn: &Connection,
        field: &str,
        value: &T,
    ) -> Result<Vec<Self>, RusticxError> {
        // Basic validation for field name to prevent SQL injection via field name
        // A more robust solution might involve checking against expected field names
        if field.contains('"') || field.contains('\'') || field.contains(' ') || field.contains('-') {
             return Err(RusticxError::QueryError(format!("Invalid characters in field name: {}", field)));
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

        // Attempt to downcast the value to common SQL types and create the dyn ToSql reference
        let any_value = value as &dyn Any;
        let param: &(dyn ToSql + Sync + 'static) = if let Some(v) = any_value.downcast_ref::<i32>() {
             v as &(dyn ToSql + Sync + 'static)
        } else if let Some(v) = any_value.downcast_ref::<String>() {
             v as &(dyn ToSql + Sync + 'static)
        } else if let Some(v) = any_value.downcast_ref::<&str>() {
             v as &(dyn ToSql + Sync + 'static)
        } else if let Some(v) = any_value.downcast_ref::<i64>() {
             v as &(dyn ToSql + Sync + 'static)
        } else if let Some(v) = any_value.downcast_ref::<f64>() {
             v as &(dyn ToSql + Sync + 'static)
        } else if let Some(v) = any_value.downcast_ref::<bool>() {
             v as &(dyn ToSql + Sync + 'static)
        // Add more type checks and casts for other supported types as needed (e.g., dates, byte arrays)
        // Ensure a ToSqlConvert implementation exists for the type being downcasted.
        } else {
            return Err(RusticxError::QueryError(format!("Unsupported parameter type for field '{}'", field)));
        };

        let params: &[&(dyn ToSql + Sync + 'static)] = &[param];


        // Attempt direct deserialization first
        let direct_results: Result<Vec<Self>, RusticxError> = conn.query_raw(&sql, params);

        match direct_results {
            Ok(models) => Ok(models),
            Err(e) => {
                 // In a production environment, you might log 'e' here for debugging
                eprintln!("Warning: Direct deserialization failed for find_by, falling back to manual row processing: {:?}", e);
                // Fallback to manual row processing
                let rows: Vec<serde_json::Map<String, serde_json::Value>> = conn.query_raw(&sql, params)?;

                rows.into_iter().map(|row| Self::from_row(&serde_json::Value::Object(row))).collect()
            }
        }
    }

    /// Executes a raw SQL query and attempts to deserialize the results into models.
    ///
    /// Use with caution, as raw SQL can be less safe if not carefully constructed,
    /// although parameter binding helps mitigate injection risks for values.
    ///
    /// Parameters should be provided as a slice of references to types implementing
    /// `ToSql + Sync + 'static` (effectively types supported by `ToSqlConvert`
    /// and cast to the trait object).
    fn find_with_sql(conn: &Connection, sql: &str, params: &[&(dyn ToSql + Sync + 'static)]) -> Result<Vec<Self>, RusticxError> {
        // Attempt direct deserialization first
        let direct_results: Result<Vec<Self>, RusticxError> = conn.query_raw(sql, params);

        match direct_results {
            Ok(models) => Ok(models),
            Err(e) => {
                // If direct deserialization failed, fallback to manual row processing
                 // In a production environment, you might log 'e' here for debugging
                eprintln!("Warning: Direct deserialization failed for find_with_sql, falling back to manual row processing: {:?}", e);
                let rows: Vec<serde_json::Map<String, serde_json::Value>> = conn.query_raw(sql, params)?;

                 rows.into_iter().map(|row| Self::from_row(&serde_json::Value::Object(row))).collect()
            }
        }
    }

    /// Counts the number of records in the table.
    ///
    /// Returns the total count as an `i64`.
    fn count(conn: &Connection) -> Result<i64, RusticxError> {
        let sql = format!("SELECT COUNT(*) as count FROM {}", Self::table_name());

        // Helper struct for deserializing the count result
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
/// Implementations for specific types provide a reference to `dyn ToSql + Sync + 'static`,
/// which is compatible with the `Connection`'s methods (assuming `Connection`
/// methods expect this trait object, as is common with the `postgres` crate's `ToSql`).
///
/// The name `as_ref_postgres` highlights that this is currently tied to the
/// `postgres` crate's `ToSql` signature. For true multi-database support, this
/// trait or the `Connection` trait's signatures would need a more generic approach.
pub trait ToSqlConvert: Debug + Sync + Send {
    /// Returns a reference to the value as `dyn ToSql + Sync + 'static`.
    ///
    /// This is required by the parameter binding mechanism used by the `Connection`
    /// methods when the `postgres` feature is enabled.
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)>;

    /// Checks if the underlying value is logically null (e.g., for `Option` types).
    fn is_null(&self) -> bool {
        false
    }
}

// --- Implementations of ToSqlConvert for common types ---

// Implementation for Option<T> where T itself implements ToSqlConvert
impl<T: ToSqlConvert + Clone + Debug + Sync + Send + 'static> ToSqlConvert for Option<T> {
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)> {
        match self {
            Some(inner) => inner.as_ref_postgres(),
            None => {
                 // For postgres, None options are bound as NULL
                 // Need a way to return a reference representing NULL
                 // The current Connection::execute/query_raw likely handles None in &[&dyn ToSql]
                 // Returning None here means filter_map will skip it, which is intended for Option.
                 // However, the parameter list must still match the placeholder count.
                 // The model must ensure its to_sql_field_values correctly handles Options
                 // and that the Connection implementation supports Option<&dyn ToSql>.
                 // Based on typical postgres/rust-postgres usage, None options are bound as NULL.
                 // This implementation relies on the Connection's handling of `None` within the slice.
                 None // Indicate that this specific Option value is NULL/None
            }
        }
    }

    fn is_null(&self) -> bool {
        self.is_none()
    }
}

// Implementation for Box<T> where T itself implements ToSqlConvert
impl<T: ToSqlConvert + ?Sized + Debug + Sync + Send + 'static> ToSqlConvert for Box<T> {
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)> {
        (**self).as_ref_postgres()
    }

    fn is_null(&self) -> bool {
        (**self).is_null()
    }
}

// Implementation for String
impl ToSqlConvert for String {
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)> {
        Some(self)
    }
}

// // Implementation for &str
// impl ToSqlConvert for &str {
//      fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)> {
//          Some(self)
//      }
// }


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

// Implementation for NaiveDateTime (requires chrono)
impl ToSqlConvert for chrono::NaiveDateTime {
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)> {
        Some(self)
    }
}

// Implementation for UUID if feature is enabled (requires uuid)
#[cfg(feature = "uuid")]
impl ToSqlConvert for uuid::Uuid {
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)> {
        Some(self)
    }
}

// Implementation for Vec<u8> (for blob/bytea data)
impl ToSqlConvert for Vec<u8> {
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)> {
        Some(self)
    }
}

// Implementation for chrono::NaiveDate (requires chrono)
impl ToSqlConvert for chrono::NaiveDate {
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)> {
        Some(self)
    }
}

// Implementation for chrono::NaiveTime (requires chrono)
impl ToSqlConvert for chrono::NaiveTime {
    fn as_ref_postgres(&self) -> Option<&(dyn ToSql + Sync + 'static)> {
        Some(self)
    }
}

// TODO: For true multi-database support using this trait structure,
// the ToSqlConvert trait would need to provide references compatible
// with *each* enabled database driver's parameter trait (e.g.,
// rusqlite::types::ToSql, mysql::prelude::ToValue). This could potentially
// be done with conditional compilation within the implementations or
// by having the Connection trait abstract parameter binding differently.
// As currently designed, parameter binding via ToSqlConvert is strongly
// coupled to the `postgres` crate's `ToSql` trait signature.

#[derive(Debug, Clone)]
pub enum ModelAttribute {
    PrimaryKey,
    Column(String),
    Default(String),
    Nullable,
    Index(bool), // true for unique index
    Foreign(String, String), // References table_name(column_name)
}