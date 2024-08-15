use sqlx::{Error, SqlitePool};
use sqlx::sqlite::SqliteQueryResult;

use crate::models::User;

pub struct Database {
    pool: SqlitePool
}

impl Database {
    pub async fn new(pool: SqlitePool) -> sqlx::Result<Database> {
        sqlx::migrate!().run(&pool).await?;
        Ok(Database {
            pool
        })
    }

    pub async fn fetch_user(&self, id: &str) -> sqlx::Result<Option<User>> {
        sqlx::query_as!(User, "SELECT * FROM users WHERE id = ?", id)
            .fetch_optional(&self.pool).await
    }

    pub async fn insert_user(&self, user: &User) -> Result<SqliteQueryResult, Error> {
        sqlx::query!("INSERT INTO users VALUES (?, ?, ?, ?, ?)", user.id, user.username, user.display_name, user.avatar, user.creation_time)
            .execute(&self.pool).await
    }
}