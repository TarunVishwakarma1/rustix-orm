use serde::{Deserialize, Serialize};
use rusticx::{Connection, SQLModel}; // Import RusticxError and DatabaseType
use rusticx_derive::Model;
use chrono::{Local, NaiveDateTime}; // Assuming created_at uses this type
use uuid::Uuid;

#[derive(Debug, Serialize, Deserialize, Model)]
// #[model(table = "users")]
pub struct Users {
    #[model(primary_key, auto_increment)]
    pub id: Option<i32>,

    #[model(column = "full_name")] 
    #[serde(rename = "full_name")]
    pub name: String,
    pub email: String,
    pub created_at: NaiveDateTime,
    #[model(sql_type = "VARCHAR(100)")]
    pub password_hash: String,
    #[model(uuid)]
    pub uuid: Uuid,
}

fn main(){
    let conn = Connection::new("postgresql://postgres:mypass@localhost:5432/postgres");

    let conn  = match conn {
        Ok(con) => con,
        Err(e) => {println!("{}", e); return;},
    };

    conn.create_table::<Users>().unwrap();


    let mut users = Users{
        id:None,
        name:String::from(""),
        email:String::from(""),
        created_at:NaiveDateTime::new(Local::now().date_naive(), Local::now().time()),
        password_hash: String::from(""),
        uuid:Uuid::new_v4()
    };

    users.insert(&conn).unwrap();

}

