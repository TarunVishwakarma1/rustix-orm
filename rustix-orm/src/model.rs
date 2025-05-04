use std::fmt::Debug;
use serde::{Deserialize, Serialize};
use crate::connection::Connection;
use crate::connection::DatabaseType;
use crate::error::RustixError;

pub trait SQLModel: Sized + Debug + Serialize + for<'de> Deserialize<'de> {
    fn table_name() -> String;
    fn primary_key_field() -> String;
    fn primary_key_value(&self) -> Option<i32>;
    fn set_primary_key(&mut self, id: i32);
    fn create_table_sql(db_type: &DatabaseType) -> String;
    
    fn save(&mut self, conn: &Connection) -> Result<(), RustixError> {
        if self.primary_key_value().is_some() {

            println!("Updating record in {}", Self::table_name());

        } else {

            println!("Inserting new record into {}", Self::table_name());
            self.set_primary_key(1); // Mock ID
        }
        Ok(())
    }
    
    fn find_by_id(conn: &Connection, id: i32) -> Result<Self, RustixError> {
        println!("Finding {} with id: {}", Self::table_name(), id);
        Err(RustixError::NotFound("Record not found".to_string()))
    }
    
    fn find_all(conn: &Connection) -> Result<Vec<Self>, RustixError> {
        println!("Finding all records in {}", Self::table_name());
        Ok(Vec::new())
    }
    
    fn delete(&self, conn: &Connection) -> Result<(), RustixError> {
        if let Some(id) = self.primary_key_value() {
            Self::delete_by_id(conn, id)
        } else {
            Err(RustixError::ValidationError("Cannot delete a record without a primary key".to_string()))
        }
    }
    
    fn delete_by_id(conn: &Connection, id: i32) -> Result<(), RustixError> {
        println!("Deleting {} with id: {}", Self::table_name(), id);
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum ModelAttribute {
    PrimaryKey,
    Column(String),
    Default(String),
    Nullable,
}