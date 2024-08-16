use sqlx::{Error, SqlitePool};

use crate::models::{ChallengeInstance, User};

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

    pub async fn insert_user(&self, user: &User) -> Result<(), Error> {
        sqlx::query!("INSERT INTO users VALUES (?, ?, ?, ?, ?)", user.id, user.username, user.display_name, user.avatar, user.creation_time)
            .execute(&self.pool).await.map(|_| ())
    }

    pub async fn insert_challenge_instance(&self, instance: &ChallengeInstance) -> Result<(), Error> {
        sqlx::query!("INSERT INTO challenge_instances VALUES (?, ?, ?, ?)", instance.user_id, instance.challenge_id, instance.state, instance.start_time)
            .execute(&self.pool).await.map(|_| ())
    }
}