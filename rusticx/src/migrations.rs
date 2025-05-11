// use crate::connection::Connection;
// use crate::error::RustixError;

// pub trait Migration {
//     fn name(&self) -> &'static str; // Changed to &'static str
//     fn up(&self, conn: &Connection) -> Result<(), RustixError>;
//     fn down(&self, conn: &Connection) -> Result<(), RustixError>;
// }

// pub struct MigrationManager {
//     conn: Connection,
//     migrations: Vec<Box<dyn Migration>>,
// }

// impl MigrationManager {
//     pub fn new(conn: Connection) -> Self {
//         MigrationManager {
//             conn,
//             migrations: Vec::new(),
//         }
//     }

//     pub fn register(&mut self, migration: Box<dyn Migration>) {
//         self.migrations.push(migration);
//     }

//     pub fn migrate_up(&self) -> Result<(), RustixError> {
//         println!("Running {} migrations", self.migrations.len());

//         self.conn.execute(
//             "CREATE TABLE IF NOT EXISTS migrations (
//                 id SERIAL PRIMARY KEY,
//                 name VARCHAR(255) NOT NULL UNIQUE,
//                 applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
//             )",
//             &[],
//         )?;

//         for migration in &self.migrations {
//             let name = migration.name();
//             println!("Checking migration: {}", name);

//             let result = self.conn.query_raw(
//                 "SELECT name FROM migrations WHERE name = ?",
//                 &[&name],
//             )?;

//             let applied = !result.is_empty();

//             if !applied {
//                 println!("Applying migration: {}", name);
//                 migration.up(&self.conn)?;

//                 self.conn.execute(
//                     "INSERT INTO migrations (name) VALUES (?)",
//                     &[&name],
//                 )?;
//             } else {
//                 println!("Migration already applied: {}", name);
//             }
//         }

//         Ok(())
//     }

//     pub fn migrate_down(&self) -> Result<(), RustixError> {
//         println!("Rolling back migrations");

//         for migration in self.migrations.iter().rev() {
//             let name = migration.name();
//             println!("Rolling back migration: {}", name);

//             let result = self.conn.query_raw(
//                 "SELECT name FROM migrations WHERE name = ?",
//                 &[&name],
//             )?;

//             let applied = !result.is_empty();

//             if applied {
//                 migration.down(&self.conn)?;

//                 self.conn.execute(
//                     "DELETE FROM migrations WHERE name = ?",
//                     &[&name],
//                 )?;
//             }
//         }

//         Ok(())
//     }
// }