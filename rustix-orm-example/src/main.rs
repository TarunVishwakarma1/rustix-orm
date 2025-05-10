use serde::{Deserialize, Serialize};
use rustix_orm::{Connection, SQLModel, SqlType};
use rustix_orm_derive::Model;

#[derive(Debug, Serialize, Deserialize, Model)]
#[model(table = "users")]
pub struct User{
    #[model(primary_key)]
    pub id: Option<i32>,
    
    #[model(column = "full_name")]
    pub name: String,
    
    #[model(nullable)]
    pub email: Option<String>,
    
    #[model(default = "CURRENT_TIMESTAMP")]
    pub created_at: String,
    
    #[model(sql_type = "VARCHAR(100)")]
    pub password_hash: String,
}

// Example of using the generated methods
fn main() -> Result<(), rustix_orm::RustixError> {
    // Create a database connection
    let conn = Connection::new("postgres://postgres:mypass@localhost:5432/postgres")?;
    // Create the users table
    conn.execute(&User::create_table_sql(&conn.get_db_type()), &[])?; //This works
    // Create a new user
    let mut user = User {
        id: None,
        name: "John Doe".to_string(),
        email: Some("john@example.com".to_string()),
        created_at: "2023-01-01 00:00:00".to_string(),
        password_hash: "hashed_password".to_string(),
    };
    // Save the user to the database
    // user.save(&conn)?;

    // println!("User saved with ID: {:?}", user.primary_key_value());
    // // Find the user by ID
    // let found_user = User::find_by_id(&conn, user.primary_key_value().unwrap())?;
    // println!("Found user: {:?}", found_user);
    
    // // Find all users
    // let all_users = User::find_all(&conn)?;
    // println!("All users: {:?}", all_users);
    
    // // Find users by email
    // let users_by_email = User::find_by(&conn, "email", &"john@example.com")?;
    // println!("Users with email john@example.com: {:?}", users_by_email);
    
    // // Update the user
    // user.name = "Jane Doe".to_string();
    // user.save(&conn)?;
    
    // // Delete the user
    // user.delete(&conn)?;
    
    // // Count users
    // let count = User::count(&conn)?;
    // println!("User count: {}", count);
    
    Ok(())
}
