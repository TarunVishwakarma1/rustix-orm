use serde::{Deserialize, Serialize};
use rusticx::{Connection, SQLModel, RusticxError}; // Import RusticxError and DatabaseType
use rusticx_derive::Model;
use chrono::NaiveDateTime; // Assuming created_at uses this type

/// Represents a user in the database.
///
/// This struct is used to model the `users` table in the database, with fields
/// corresponding to the columns in the table. It derives the `Model` trait to
/// enable database operations such as insertion, updating, and querying.
#[derive(Debug, Serialize, Deserialize, Model)]
#[model(table = "demoooo")]
pub struct User {
    /// The unique identifier for the user, which is the primary key.
    #[model(primary_key, auto_increment)]
    pub id: Option<i32>,

    /// The full name of the user.
    #[model(column = "full_name")] 
    #[serde(rename = "full_name")]
    pub name: String,

    /// The email address of the user.
    pub email: String,

    /// The timestamp when the user was created.
    pub created_at: NaiveDateTime,

    /// The hashed password of the user.
    #[model(sql_type = "VARCHAR(100)")]
    pub password_hash: String,
}

// You might need to implement the ToSqlConvert trait for NaiveDateTime
// in your model.rs or a separate file if it's not already handled
// by a generic implementation for PostgresToSql types.
// If your model.rs only has #[cfg(feature="postgres")] impl<T: PostgresToSql + ...> ToSqlConvert for T,
// and NaiveDateTime implements PostgresToSql, it should work.

fn main() -> Result<(), rusticx::RusticxError> {
    println!("Attempting to connect to database...");
    // Make sure your PostgreSQL container is running and accessible
    let conn = Connection::new("postgresql://postgres:mypass@localhost:5432/postgres")?;
    println!("Connected successfully.");

    // --- Demonstrate create_table_sql ---
    // This part is often run separately or conditionally to set up the database.
    // Uncomment and run once if you need to create the table.
    println!("\n--- Demonstrating create_table_sql ---");
    let create_sql = User::create_table_sql(&conn.get_db_type());
    println!("Generated CREATE TABLE SQL:\n{}", create_sql);
    // Execute the create table SQL (use with caution - it will error if table exists)
    let create_result = conn.execute(&create_sql, &[]);
    match create_result {
        Ok(_) => println!("CREATE TABLE command executed (table might already exist)."),
        Err(e) => eprintln!("Error executing CREATE TABLE: {}", e),
    }


    // --- Demonstrate INSERT ---
    println!("\n--- Demonstrating INSERT ---");
    let created_at_str = "2023-01-01 00:00:00";
    let created_at_time = NaiveDateTime::parse_from_str(created_at_str, "%Y-%m-%d %H:%M:%S")
        .expect("Failed to parse timestamp");

    let mut new_user = User {
        id: None, // ID should be None for new records
        name: "Alice Smith".to_string(),
        email: String::from("alice@example.com"),
        created_at: created_at_time,
        password_hash: "hashed_alice_pw".to_string(),
    };
    println!("User instance before insert: {:?}", new_user);
    new_user.insert(&conn)?; // Insert the user
    // The ID should now be populated by the database and set on the instance
    println!("User inserted. Instance after insert (ID should be populated): {:?}", new_user);
    let alice_id = new_user.id.expect("ID should be populated after insert"); // Store ID for later

    // Insert another user for find_all and count examples
     let mut another_user = User {
         id: None,
         name: "Bob Johnson".to_string(),
         email: String::from("bob@example.com"),
         created_at: NaiveDateTime::parse_from_str("2024-02-15 12:00:00", "%Y-%m-%d %H:%M:%S").expect("Failed to parse timestamp"),
         password_hash: "hashed_bob_pw".to_string(),
     };
     another_user.insert(&conn)?;
     println!("Another user inserted: {:?}", another_user);
     let bob_id = another_user.id.expect("ID should be populated after insert");


    // --- Demonstrate find_by_id ---
    println!("\n--- Demonstrating find_by_id ---");
    match User::find_by_id(&conn, alice_id) {
        Ok(found_user) => println!("Found user by ID {}: {:?}", alice_id, found_user),
        Err(RusticxError::NotFound(msg)) => println!("find_by_id error: {}", msg),
        Err(e) => println!("find_by_id query error: {}", e),
    }
     // Try finding a non-existent ID to show NotFound error
    match User::find_by_id(&conn, 9999) {
        Ok(found_user) => println!("Found user by ID 9999 (unexpected): {:?}", found_user),
        Err(RusticxError::NotFound(msg)) => println!("find_by_id 9999 error (expected NotFound): {}", msg),
        Err(e) => println!("find_by_id 9999 query error: {}", e),
    }


    // --- Demonstrate find_all ---
    println!("\n--- Demonstrating find_all ---");
    let all_users = User::find_all(&conn)?;
    println!("Found all users ({}) : {:#?}", all_users.len(), all_users);

    // --- Demonstrate UPDATE ---
    println!("\n--- Demonstrating UPDATE ---");
    // We need an instance with an ID to update. Use the 'new_user' variable which has the ID from insert.
    println!("User before update: {:?}", new_user);
    new_user.name = "Alicia Smith".to_string(); // Change the name
    new_user.email = "alicia.s@example.com".to_string(); // Change the email
    new_user.update(&conn)?; // Update the user in the database
    println!("User updated. Instance after update: {:?}", new_user);

    // Verify the update by fetching the user again
    println!("Verifying update with find_by_id...");
    match User::find_by_id(&conn, alice_id) {
        Ok(updated_user) => println!("Verified updated user: {:?}", updated_user),
        Err(e) => println!("Verification failed: {}", e),
    }


    // --- Demonstrate find_by ---
    println!("\n--- Demonstrating find_by ---");
    // Find by email (using the new email)
    // Note: find_by uses &dyn Any, so we pass a reference to the value.
    let users_by_email = User::find_by(&conn, "email", &"alicia.s@example.com".to_string())?; // Pass &String
    println!("Found users by email 'alicia.s@example.com': {:#?}", users_by_email);

    // Find by the renamed column (full_name) using the updated name
    let users_by_name = User::find_by(&conn, "full_name", &"Alicia Smith".to_string())?; // Pass &String
    println!("Found users by name 'Alicia Smith' (using 'full_name' column): {:#?}", users_by_name);

    // Find by a non-existent value
    let users_not_found = User::find_by(&conn, "email", &"nonexistent@example.com".to_string())?;
    println!("Found users by non-existent email: {:#?}", users_not_found); // Should be an empty vector

    // --- Demonstrate COUNT ---
    println!("\n--- Demonstrating COUNT ---");
    let count_before_delete = User::count(&conn)?;
    println!("User count before delete: {}", count_before_delete);


    // --- Demonstrate DELETE ---
    // We can delete using the instance or by ID. Let's delete Bob by ID and Alice using the instance.
    println!("\n--- Demonstrating DELETE ---");
    println!("Deleting user with ID {} (Bob) using delete_by_id...", bob_id);
    User::delete_by_id(&conn, bob_id)?;
    println!("User with ID {} (Bob) deleted.", bob_id);

    println!("Deleting user with ID {} (Alice) using delete method on instance...", alice_id);
    new_user.delete(&conn)?; // Use the 'new_user' instance which still holds Alice's ID
    println!("User with ID {} (Alice) deleted.", alice_id);


    // Verify deletions
     println!("\nVerifying deletions...");
    match User::find_by_id(&conn, alice_id) {
        Ok(found_user) => println!("User with ID {} still found (unexpected): {:?}", alice_id, found_user),
        Err(RusticxError::NotFound(msg)) => println!("User with ID {} not found (expected NotFound): {}", alice_id, msg),
        Err(e) => println!("Verification find_by_id query error: {}", e),
    }
     match User::find_by_id(&conn, bob_id) {
        Ok(found_user) => println!("User with ID {} still found (unexpected): {:?}", bob_id, found_user),
        Err(RusticxError::NotFound(msg)) => println!("User with ID {} not found (expected NotFound): {}", bob_id, msg),
        Err(e) => println!("Verification find_by_id query error: {}", e),
    }


     // Demonstrate count after deletes
    println!("\n--- Demonstrating COUNT after deletes ---");
    let count_after_delete = User::count(&conn)?;
    println!("User count after deletes: {}", count_after_delete); // Should be 0


     // --- Demonstrate find_with_sql ---
     // Use with caution as this is raw SQL
    println!("\n--- Demonstrating find_with_sql ---");
    // Select all from the now empty table
    let raw_sql_all = User::find_with_sql(&conn, "SELECT * FROM demoooo", &[])?;
    println!("Found users using raw SQL 'SELECT * FROM demoooo': {:#?}", raw_sql_all); // Should be empty vector

    // Insert one user back to demonstrate find_with_sql with parameters
    let mut temp_user = User {
         id: None,
         name: "Charlie Brown".to_string(),
         email: "charlie@example.com".to_string(),
         created_at: NaiveDateTime::parse_from_str("2025-05-10 08:00:00", "%Y-%m-%d %H:%M:%S").expect("Failed to parse timestamp"),
         password_hash: "hashed_charlie_pw".to_string(),
    };
    temp_user.insert(&conn)?;
    println!("Inserted a temporary user for raw SQL demo: {:?}", temp_user);

    // Find Charlie using raw SQL with a parameter
    let charlie_email = "charlie@example.com".to_string();
    // Note: The SELECT column list in raw SQL should match the struct fields you are deserializing into (User)
    // or you need a different struct for partial results with its own Deserialize impl.
    let raw_sql_charlie = User::find_with_sql(&conn, "SELECT id, full_name, email, created_at, password_hash FROM demoooo WHERE email = $1", &[&charlie_email])?;
    println!("Found Charlie using raw SQL with parameter: {:#?}", raw_sql_charlie);


    // Clean up the temporary user
    println!("\nCleaning up temporary user...");
    temp_user.delete(&conn)?;
    println!("Temporary user deleted.");
     let final_count = User::count(&conn)?;
    println!("Final user count: {}", final_count); // Should be 0 again


    println!("\nAll examples finished successfully.");

    Ok(())
}