use rustix_orm::{Connection, SQLModel, SqlType};
use rustix_orm_derive::Model;
use serde::{Deserialize, Serialize};


#[derive(Debug, Deserialize, Serialize, Model)]
#[model(table_name="students")]
struct Student {
    #[model(primary_key, auto_increment)]
    id: Option::<i32>,
    name: String,
    age: i32,
    phone_no: String
}

fn main() {
    // Creating Students Table
    let conn = Connection::new("postgres://postgres:mypass@localhost:5432/postgres").unwrap();

    conn.create_table::<Student>().unwrap();
}