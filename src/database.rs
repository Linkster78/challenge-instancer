use sqlx::{Error, SqlitePool};
use sqlx::sqlite::{SqliteConnectOptions, SqliteQueryResult};

use crate::config::DatabaseConfig;
use crate::models::User;

pub struct InstancerDatabase {
    pool: SqlitePool
}

impl InstancerDatabase {
    pub async fn new(config: &DatabaseConfig) -> sqlx::Result<InstancerDatabase> {
        let pool = SqlitePool::connect_with(SqliteConnectOptions::new()
            .create_if_missing(true)
            .filename(config.file_path.clone()))
            .await?;

        sqlx::migrate!().run(&pool).await?;

        Ok(InstancerDatabase {
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