use chrono::NaiveDateTime;
use rusticx::{Connection, RusticxError, SQLModel,SqlType};
use serde::{Deserialize, Serialize};
use rusticx_derive::Model;

#[derive(Debug, Serialize, Deserialize, Model)]
#[model(table = "users")]
pub struct User {
    #[model(primary_key, auto_increment)]
    pub id: Option<i32>,

    #[model(column = "full_name")] 
    #[serde(rename = "full_name")]
    pub name: String,

    pub email: String,

    pub created_at: NaiveDateTime,

    #[model(sql_type = "VARCHAR(100)")]
    pub password_hash: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    // Helper function to create a test database connection
    fn create_connection() -> Result<Connection, RusticxError> {
        // Use an environment variable to override the connection string for CI/CD
        let conn_string = std::env::var("TEST_DB_URL")
            .unwrap_or_else(|_| "postgres://postgres:mypass@localhost:5432/postgres".to_string());
        
        Connection::new(&conn_string)
    }

    // Helper function to set up the test database
    fn setup_database(conn: &Connection) -> Result<(), Box<dyn Error>> {
        // Create users table if it doesn't exist
        let create_sql = User::create_table_sql(&conn.get_db_type());
        match conn.execute(&create_sql, &[]) {
            Ok(_) => (),
            Err(e) => eprintln!("Table may already exist: {}", e),
        }

        // Clean up any existing test data to ensure tests start fresh
        let _ = conn.execute("DELETE FROM users WHERE email LIKE '%test.com'", &[]);
        
        Ok(())
    }

    // Helper function to create a test user
    fn create_test_user(name: &str, email: &str) -> User {
        let created_at = NaiveDateTime::parse_from_str(
            "2023-01-01 00:00:00", 
            "%Y-%m-%d %H:%M:%S"
        ).expect("Failed to parse timestamp");
        
        User {
            id: None,
            name: name.to_string(),
            email: email.to_string(),
            created_at,
            password_hash: format!("hashed_{}_pw", name.to_lowercase()),
        }
    }

    #[test]
    fn test_create_and_find_user() -> Result<(), Box<dyn Error>> {
        let conn = create_connection()?;
        setup_database(&conn)?;

        // Create a test user
        let mut user = create_test_user("Test User", "test@test.com");
        
        // Test insert
        user.insert(&conn)?;
        assert!(user.id.is_some(), "User ID should be populated after insert");
        
        // Test find_by_id
        let found_user = User::find_by_id(&conn, user.id.unwrap())?;
        assert_eq!(found_user.name, "Test User");
        assert_eq!(found_user.email, "test@test.com");
        
        // Clean up
        user.delete(&conn)?;
        
        Ok(())
    }

    #[test]
    fn test_update_user() -> Result<(), Box<dyn Error>> {
        let conn = create_connection()?;
        setup_database(&conn)?;

        // Create and insert test user
        let mut user = create_test_user("Update Test", "update@test.com");
        user.insert(&conn)?;
        
        // Update user
        user.name = "Updated Name".to_string();
        user.email = "updated@test.com".to_string();
        user.update(&conn)?;
        
        // Verify update
        let updated_user = User::find_by_id(&conn, user.id.unwrap())?;
        assert_eq!(updated_user.name, "Updated Name");
        assert_eq!(updated_user.email, "updated@test.com");
        
        // Clean up
        user.delete(&conn)?;
        
        Ok(())
    }

    #[test]
    fn test_find_by() -> Result<(), Box<dyn Error>> {
        let conn = create_connection()?;
        setup_database(&conn)?;

        // Create and insert test users
        let mut user1 = create_test_user("Find Test", "find@test.com");
        let mut user2 = create_test_user("Find Test", "find2@test.com");
        user1.insert(&conn)?;
        user2.insert(&conn)?;
        
        // Test find_by email
        let users_by_email = User::find_by(&conn, "email", &"find@test.com".to_string())?;
        assert_eq!(users_by_email.len(), 1);
        assert_eq!(users_by_email[0].email, "find@test.com");
        
        // Test find_by name (using the full_name column)
        let users_by_name = User::find_by(&conn, "full_name", &"Find Test".to_string())?;
        assert_eq!(users_by_name.len(), 2);
        
        // Test find_by with non-existent value
        let not_found = User::find_by(&conn, "email", &"nonexistent@test.com".to_string())?;
        assert_eq!(not_found.len(), 0);
        
        // Clean up
        user1.delete(&conn)?;
        user2.delete(&conn)?;
        
        Ok(())
    }

    #[test]
    fn test_find_all_and_count() -> Result<(), Box<dyn Error>> {
        let conn = create_connection()?;
        setup_database(&conn)?;

        // Delete any existing test users to ensure clean state
        let _ = conn.execute("DELETE FROM users WHERE email LIKE '%findall%'", &[]);
        
        // Create and insert multiple test users
        let mut user1 = create_test_user("Find All 1", "findall1@test.com");
        let mut user2 = create_test_user("Find All 2", "findall2@test.com");
        let mut user3 = create_test_user("Find All 3", "findall3@test.com");
        
        user1.insert(&conn)?;
        user2.insert(&conn)?;
        user3.insert(&conn)?;
        
        // Test count
        let users_with_findall = User::find_with_sql(
            &conn, 
            "SELECT * FROM users WHERE email LIKE $1", 
            &[&"%findall%".to_string()]
        )?;
        assert_eq!(users_with_findall.len(), 3);
        
        // Test find_all (note: this will find ALL users in the table)
        let all_users = User::find_all(&conn)?;
        assert!(all_users.len() >= 3); // At least our 3 test users
        
        // Clean up
        user1.delete(&conn)?;
        user2.delete(&conn)?;
        user3.delete(&conn)?;
        
        Ok(())
    }

    #[test]
    fn test_delete_operations() -> Result<(), Box<dyn Error>> {
        let conn = create_connection()?;
        setup_database(&conn)?;

        // Create and insert test users
        let mut user1 = create_test_user("Delete Test 1", "delete1@test.com");
        let mut user2 = create_test_user("Delete Test 2", "delete2@test.com");
        
        user1.insert(&conn)?;
        user2.insert(&conn)?;
        
        // Test delete by instance
        user1.delete(&conn)?;
        
        // Test delete by id
        let id = user2.id.unwrap();
        User::delete_by_id(&conn, id)?;
        
        // Verify deletions
        match User::find_by_id(&conn, user1.id.unwrap()) {
            Ok(_) => panic!("User 1 was not deleted"),
            Err(RusticxError::NotFound(_)) => (), // Expected
            Err(e) => return Err(Box::new(e)),
        }
        
        match User::find_by_id(&conn, id) {
            Ok(_) => panic!("User 2 was not deleted"),
            Err(RusticxError::NotFound(_)) => (), // Expected
            Err(e) => return Err(Box::new(e)),
        }
        
        Ok(())
    }

    #[test]
    fn test_find_with_sql() -> Result<(), Box<dyn Error>> {
        let conn = create_connection()?;
        setup_database(&conn)?;

        // Create and insert test user
        let mut user = create_test_user("SQL Test", "sql@test.com");
        user.insert(&conn)?;
        
        // Test find_with_sql with parameters
        let sql_param = "sql@test.com".to_string();
        let users = User::find_with_sql(
            &conn,
            "SELECT id, full_name, email, created_at, password_hash FROM users WHERE email = $1",
            &[&sql_param]
        )?;
        
        assert_eq!(users.len(), 1);
        assert_eq!(users[0].email, "sql@test.com");
        
        // Clean up
        user.delete(&conn)?;
        
        Ok(())
    }

    #[test]
    fn test_not_found_error() -> Result<(), Box<dyn Error>> {
        let conn = create_connection()?;
        
        // Test finding a non-existent ID
        match User::find_by_id(&conn, 99999) {
            Ok(_) => panic!("Should not find user with ID 99999"),
            Err(RusticxError::NotFound(_)) => (), // Expected
            Err(e) => return Err(Box::new(e)),
        }
        
        Ok(())
    }
}