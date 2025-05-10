use std::fmt::Debug;
use serde::{Deserialize, Serialize};
use crate::connection::{Connection, DatabaseType};
use crate::error::RustixError;

pub trait SQLModel: Sized + Debug + Serialize + for<'de> Deserialize<'de> {
    fn table_name() -> String;
    fn primary_key_field() -> String;
    fn primary_key_value(&self) -> Option<i32>;
    fn set_primary_key(&mut self, id: i32);
    fn create_table_sql(db_type: &DatabaseType) -> String;
    
    // Generate field names for SQL queries
    fn field_names() -> Vec<&'static str>;
    
    // Generate values for SQL parameters
    fn field_values(&self) -> Vec<Box<dyn Debug>>;
    
    // Methods to convert from database rows to model instances
    fn from_row(row: &serde_json::Value) -> Result<Self, RustixError>;
    
    fn save(&mut self, conn: &Connection) -> Result<(), RustixError> {
        if let Some(id) = self.primary_key_value() {
            // Update existing record
            let fields = Self::field_names();
            let field_params: Vec<String> = fields.iter()
                .filter(|&f| *f != Self::primary_key_field())
                .map(|f| format!("{} = ?", f))
                .collect();
            
            let sql = format!(
                "UPDATE {} SET {} WHERE {} = ?",
                Self::table_name(),
                field_params.join(", "),
                Self::primary_key_field()
            );
            
            // Prepare parameters
            let mut params: Vec<&dyn Debug> = Vec::new();
            let field_values = self.field_values();
            for (i, field) in Self::field_names().iter().enumerate() {
                if *field != Self::primary_key_field() {
                    params.push(&*field_values[i]);
                }
            }
            // Add the primary key as the last parameter for the WHERE clause
            params.push(&id);
            
            conn.execute(&sql, &params)?;
        } else {
            // Insert new record
            let fields = Self::field_names();
            let placeholders: Vec<String> = (0..fields.len()).map(|_| "?".to_string()).collect();
            
            let sql = format!(
                "INSERT INTO {} ({}) VALUES ({})",
                Self::table_name(),
                fields.join(", "),
                placeholders.join(", ")
            );
            
            // Prepare parameters
            let field_values = self.field_values();
            let params: Vec<&dyn Debug> = field_values.iter().map(|v| &**v).collect();
            
            // Execute the query
            conn.execute(&sql, &params)?;
            
            // Get the last inserted ID
            // This is database-specific, so here's a generalized version
            let last_id_sql = match conn.get_db_type() {
                DatabaseType::PostgreSQL => format!("SELECT lastval() as id"),
                DatabaseType::MySQL => format!("SELECT LAST_INSERT_ID() as id"),
                DatabaseType::SQLite => format!("SELECT last_insert_rowid() as id"),
            };
            
            #[derive(serde::Deserialize)]
            struct IdRow {
                id: i64,
            }
            
            let ids: Vec<IdRow> = conn.query_raw(&last_id_sql, &[])?;
            if let Some(id_row) = ids.first() {
                self.set_primary_key(id_row.id as i32);
            }
        }
        
        Ok(())
    }
    
    fn find_by_id(conn: &Connection, id: i32) -> Result<Self, RustixError> {
        let sql = format!(
            "SELECT * FROM {} WHERE {} = ?",
            Self::table_name(),
            Self::primary_key_field()
        );
        
        // Try to directly deserialize from the database result
        let results: Result<Vec<Self>, _> = conn.query_raw(&sql, &[&id]);
        
        match results {
            Ok(models) => {
                if let Some(model) = models.into_iter().next() {
                    Ok(model)
                } else {
                    Err(RustixError::NotFound(format!("{} with id {} not found", Self::table_name(), id)))
                }
            },
            Err(_) => {
                // Fallback to manual deserialization
                let results: Vec<serde_json::Map<String, serde_json::Value>> = conn.query_raw(&sql, &[&id])?;
                
                if let Some(row) = results.first() {
                    Self::from_row(&serde_json::Value::Object(row.clone()))
                } else {
                    Err(RustixError::NotFound(format!("{} with id {} not found", Self::table_name(), id)))
                }
            }
        }
    }
    
    fn find_all(conn: &Connection) -> Result<Vec<Self>, RustixError> {
        let sql = format!("SELECT * FROM {}", Self::table_name());
        
        // Try to directly deserialize from the database result
        let direct_results: Result<Vec<Self>, _> = conn.query_raw(&sql, &[]);
        
        match direct_results {
            Ok(models) => Ok(models),
            Err(_) => {
                // Fallback to manual deserialization
                let rows: Vec<serde_json::Map<String, serde_json::Value>> = conn.query_raw(&sql, &[])?;
                
                let mut models = Vec::with_capacity(rows.len());
                for row in rows {
                    models.push(Self::from_row(&serde_json::Value::Object(row))?);
                }
                
                Ok(models)
            }
        }
    }
    
    fn delete(&self, conn: &Connection) -> Result<(), RustixError> {
        if let Some(id) = self.primary_key_value() {
            Self::delete_by_id(conn, id)
        } else {
            Err(RustixError::ValidationError("Cannot delete a record without a primary key".to_string()))
        }
    }
    
    fn delete_by_id(conn: &Connection, id: i32) -> Result<(), RustixError> {
        let sql = format!(
            "DELETE FROM {} WHERE {} = ?",
            Self::table_name(),
            Self::primary_key_field()
        );
        
        conn.execute(&sql, &[&id])?;
        
        Ok(())
    }
    
    // Add utility methods for finding by arbitrary conditions
    fn find_by(conn: &Connection, field: &str, value: &dyn Debug) -> Result<Vec<Self>, RustixError> {
        let sql = format!(
            "SELECT * FROM {} WHERE {} = ?",
            Self::table_name(),
            field
        );
        
        // Try direct deserialization first
        let direct_results: Result<Vec<Self>, _> = conn.query_raw(&sql, &[value]);
        
        match direct_results {
            Ok(models) => Ok(models),
            Err(_) => {
                // Fallback to manual row processing
                let rows: Vec<serde_json::Map<String, serde_json::Value>> = conn.query_raw(&sql, &[value])?;
                
                let mut models = Vec::with_capacity(rows.len());
                for row in rows {
                    models.push(Self::from_row(&serde_json::Value::Object(row))?);
                }
                
                Ok(models)
            }
        }
    }
    
    fn find_with_sql(conn: &Connection, sql: &str, params: &[&dyn Debug]) -> Result<Vec<Self>, RustixError> {
        // Try direct deserialization first
        let direct_results: Result<Vec<Self>, _> = conn.query_raw(sql, params);
        
        match direct_results {
            Ok(models) => Ok(models),
            Err(_) => {
                // Fallback to manual row processing
                let rows: Vec<serde_json::Map<String, serde_json::Value>> = conn.query_raw(sql, params)?;
                
                let mut models = Vec::with_capacity(rows.len());
                for row in rows {
                    models.push(Self::from_row(&serde_json::Value::Object(row))?);
                }
                
                Ok(models)
            }
        }
    }
    
    // Count records
    fn count(conn: &Connection) -> Result<i64, RustixError> {
        let sql = format!("SELECT COUNT(*) as count FROM {}", Self::table_name());
        
        #[derive(serde::Deserialize)]
        struct CountResult {
            count: i64,
        }
        
        let counts: Vec<CountResult> = conn.query_raw(&sql, &[])?;
        
        if let Some(count_result) = counts.first() {
            Ok(count_result.count)
        } else {
            Ok(0)
        }
    }
}

#[derive(Debug, Clone)]
pub enum ModelAttribute {
    PrimaryKey,
    Column(String),
    Default(String),
    Nullable,
    Index(bool), // true for unique index
    Foreign(String, String), // References table_name(column_name)
}