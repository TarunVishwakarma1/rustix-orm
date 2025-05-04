use serde::{Deserialize, Serialize};
use rustix_orm::{Connection, QueryBuilder, SQLModel, DatabaseType, SqlType};
use rustix_orm_derive::Model;

#[derive(Debug, Serialize, Deserialize, Model)]
#[model(table = "users")]
struct User {
    #[model(primary_key)]
    id: Option<i32>,
    name: String,
    email: String,
    age: i32,
}

#[derive(Debug, Serialize, Deserialize, Model)]
#[model(table = "posts")]
struct Post {
    #[model(primary_key)]
    id: Option<i32>,
    title: String,
    content: String,
    user_id: i32,
    #[model(default = "false")]
    published: bool,
}

impl User {
    fn posts(&self, conn: &Connection) -> Result<Vec<Post>, rustix_orm::RustixError> {
        QueryBuilder::new()
            .filter("user_id = ?", &[self.id.unwrap()])
            .order_by("id", false)
            .find_all::<Post>(conn)
    }
}

impl Post {
    fn author(&self, conn: &Connection) -> Result<User, rustix_orm::RustixError> {
        User::find_by_id(conn, self.user_id) // will give error for now
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Connect to database
    let conn = Connection::new("postgres://postgres:mypass@localhost:5432/postgres")?;

    conn.create_table::<User>()?;
    conn.create_table::<Post>()?;

    conn.transaction(|tx| {

        let mut user = User {
            id: None,
            name: "John Doe".to_string(),
            email: "john@example.com".to_string(),
            age: 30,
        };
        user.save(tx)?;
        println!("User created with ID: {:?}", user.id);

        let mut post1 = Post {
            id: None,
            title: "First Post".to_string(),
            content: "Hello, world!".to_string(),
            user_id: user.id.unwrap(),
            published: true,
        };
        post1.save(tx)?;

        let mut post2 = Post {
            id: None,
            title: "Draft Post".to_string(),
            content: "Work in progress...".to_string(),
            user_id: user.id.unwrap(),
            published: false,
        };
        post2.save(tx)?;

        post2.title = "Updated Draft Post".to_string();
        post2.save(tx)?;

        let published_posts = QueryBuilder::new()
            .filter("published = ?", &[&true])
            .find_all::<Post>(tx)?;

        println!("Published posts: {:?}", published_posts);

        let adult_users = QueryBuilder::new()
            .filter("age > ?", &[&25])
            .order_by("name", true)
            .find_all::<User>(tx)?;

        println!("Adult users: {:?}", adult_users);

        let user_posts = user.posts(tx)?;
        println!("User posts: {:?}", user_posts);

        post1.delete(tx)?;


        Ok(())
    })?;

    println!("All operations completed successfully!");
    Ok(())
}