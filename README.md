# Rustix ORM

Rustix ORM is a lightweight and intuitive Object-Relational Mapping (ORM) library for Rust, designed to simplify database interactions. This project supports multiple databases, including PostgreSQL, MySQL, and SQLite.

## Table of Contents

- [Features](#features)
- [Installation](#installation)
- [Usage](#usage)
- [Creating Models](#creating-models)
- [Testing](#testing)
- [Contributing](#contributing)
- [License](#license)

## Features

- **Multi-Database Support**: Works with PostgreSQL, MySQL, and SQLite.
- **Easy Model Creation**: Define your database models using Rust structs.
- **Automatic Table Creation**: Automatically generate SQL for creating tables.
- **CRUD Operations**: Simplified methods for creating, reading, updating, and deleting records.

## Installation

To use Rustix ORM in your project, add the following to your `Cargo.toml`:

```toml
[dependencies]
rustix-orm = { path = "path/to/rustix-orm" }
rustix-orm-derive = { path = "path/to/rustix-orm-derive" }
```

Make sure to replace `path/to/` with the actual path to the `rustix-orm` and `rustix-orm-derive` directories.

## Usage

To get started with Rustix ORM, follow these steps:

1. **Create a Connection**: Establish a connection to your database.

```rust
use rustix_orm::Connection;

let conn = Connection::new("postgres://username:password@localhost:5432/database_name").unwrap();
```

2. **Define Your Model**: Create a struct that represents your database table.

```rust
use rustix_orm_derive::Model;

#[derive(Debug, Model)]
#[model(table_name = "students")]
struct Student {
    #[model(primary_key, auto_increment)]
    id: Option<i32>,
    name: String,
    age: i32,
    phone_no: String,
}
```

3. **Create the Table**: Use the connection to create the table in the database.

```rust
conn.create_table::<Student>().unwrap();
```

4. **Perform CRUD Operations**: Use methods provided by the ORM to interact with your data.

## Creating Models

To create a model, define a struct and derive the `Model` trait. Use attributes to specify table names, primary keys, and other properties.

### Example

```rust
#[derive(Debug, Model)]
#[model(table_name = "users")]
struct User {
    #[model(primary_key, auto_increment)]
    id: Option<i32>,
    #[model(column = "full_name")]
    name: String,
    email: String,
    created_at: chrono::NaiveDateTime,
    #[model(sql_type = "VARCHAR(100)")]
    password_hash: String,
}
```

## Testing

To run tests, ensure you have a test database set up. You can use the following command:

```bash
cargo test
```

Make sure to set the `TEST_DB_URL` environment variable to point to your test database.

## Contributing

Contributions are welcome! Please fork the repository and submit a pull request with your changes.

## License

This project is licensed under the MIT License. See the [LICENSE](LICENSE) file for details.
