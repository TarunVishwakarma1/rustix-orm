# rusticx

![rusticx](https://img.shields.io/badge/rust-1.45.0-orange.svg) ![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)

## Overview

`rusticx` is a lightweight, intuitive ORM (Object-Relational Mapping) library for Rust, designed to simplify database interactions across various SQL databases, including PostgreSQL, MySQL, and SQLite. It provides a unified interface for managing database connections, executing queries, and handling transactions.

## Features

- **Multi-Database Support**: Seamlessly switch between PostgreSQL, MySQL, and SQLite. (Tested for Postgres)
- **Asynchronous Operations**: Built on top of `tokio`, allowing for non-blocking database interactions. (in development)
- **Error Handling**: Comprehensive error management with custom error types.
- **Model Definition**: Define your database models with ease using traits.
- **Transactions**: Support for executing transactions with rollback capabilities.
- **Serialization**: Automatic serialization and deserialization of data using `serde`.

## Installation

To use `rusticx`, add it to your `Cargo.toml`:

```cmd
cargo add rusticx
```

### Optional Features

You can enable specific database support by adding features in your `Cargo.toml`:

```toml
[dependencies.rusticx]
version = "0.1.0"
features = ["postgres", "mysql", "rusqlite"]
```

## Getting Started

### Basic Usage

1. **Creating a Connection**:

```rust
use rusticx::{Connection, DatabaseType};

let conn = Connection::new("postgres://user:password@localhost/dbname")?;
```

2. **Defining a Model**:

```rust
use rusticx::model::SQLModel;

[derive(Debug, Serialize, Deserialize)]
struct User {
    id: Option<i32>,
    name: String,
    email: String,
}

impl SQLModel for User {
    fn table_name() -> String {
        "users".to_string()
    }

    fn primary_key_field() -> String {
        "id".to_string()
    }

    fn primary_key_value(&self) -> Option<i32> {
        self.id
    }

    fn set_primary_key(&mut self, id: i32) {
        self.id = Some(id);
    }

    // Implement other required methods...
}
```

**Usage with rustic_derive**

``` rust
use rusticx::model::SQLModel;
use rusticx_derive::Model;

#[derive(Debug, Serialize, Deserialize, Model)]
#[model(table = "users")]
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

    /// The password of the user.
    #[model(sql_type = "VARCHAR(100)")]
    pub password_hash: String,
}
```


3. **Inserting a Record**:

```rust
let mut user = User { id: None, name: "Alice".to_string(), email: "alice@example.com".to_string() };
user.insert(&conn)?;
```

4. **Querying Records**:

```rust
let users: Vec<User> = User::find_all(&conn)?;
```

5. **Updating a Record**:

```rust
user.name = "Alice Smith".to_string();
user.update(&conn)?;
```

6. **Deleting a Record**:

```rust
user.delete(&conn)?;
```

## Error Handling

`rusticx` provides a custom error type, `RusticxError`, which encapsulates various error scenarios, including connection errors, query errors, and serialization errors. You can handle these errors using Rust's standard error handling mechanisms.

## Contributing

Contributions are welcome! Please follow these steps:

1. Fork the repository.
2. Create a new branch (`git checkout -b feature-branch`).
3. Make your changes and commit them (`git commit -m 'Add new feature'`).
4. Push to the branch (`git push origin feature-branch`).
5. Create a pull request.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.

## Contact

For any inquiries or issues, please reach out to [Tarun Vishwakarma](mailto:vishwakarmatarun121@icloud.com).

## Acknowledgments

- [Rust](https://www.rust-lang.org/) - The programming language used.
- [Tokio](https://tokio.rs/) - The asynchronous runtime for Rust.
- [Serde](https://serde.rs/) - The serialization framework for Rust.
