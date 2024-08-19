use sqlx::{Error, SqlitePool};
use crate::models::{ChallengeInstance, ChallengeInstanceState, User};

#[derive(Clone)]
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
        sqlx::query_as("SELECT * FROM users WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool).await
    }

    pub async fn insert_user(&self, user: &User) -> Result<(), Error> {
        sqlx::query("INSERT INTO users VALUES (?, ?, ?, ?, ?)")
            .bind(&user.id)
            .bind(&user.username)
            .bind(&user.display_name)
            .bind(&user.avatar)
            .bind(&user.creation_time)
            .execute(&self.pool).await.map(|_| ())
    }

    pub async fn insert_challenge_instance(&self, instance: &ChallengeInstance) -> Result<(), Error> {
        sqlx::query("INSERT INTO challenge_instances VALUES (?, ?, ?, ?, ?)")
            .bind(&instance.user_id)
            .bind(&instance.challenge_id)
            .bind(&instance.state)
            .bind(&instance.details)
            .bind(&instance.start_time)
            .execute(&self.pool).await.map(|_| ())
    }

    pub async fn transition_challenge_instance_state(&self, user_id: &str, challenge_id: &str, old_state: ChallengeInstanceState, new_state: ChallengeInstanceState) -> Result<bool, Error> {
        let result = sqlx::query("UPDATE challenge_instances SET state = ? WHERE user_id = ? AND challenge_id = ? AND state = ?")
            .bind(new_state)
            .bind(user_id)
            .bind(challenge_id)
            .bind(old_state)
            .execute(&self.pool).await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn update_challenge_instance_state(&self, user_id: &str, challenge_id: &str, new_state: ChallengeInstanceState) -> Result<(), Error> {
        sqlx::query("UPDATE challenge_instances SET state = ? WHERE user_id = ? AND challenge_id = ?")
            .bind(new_state)
            .bind(user_id)
            .bind(challenge_id)
            .execute(&self.pool).await.map(|_| ())
    }

    pub async fn populate_running_challenge_instance(&self, user_id: &str, challenge_id: &str, details: &str) -> Result<(), Error> {
        sqlx::query("UPDATE challenge_instances SET state = ?, details = ? WHERE user_id = ? AND challenge_id = ?")
            .bind(ChallengeInstanceState::Running)
            .bind(details)
            .bind(user_id)
            .bind(challenge_id)
            .execute(&self.pool).await.map(|_| ())
    }

    pub async fn delete_challenge_instance(&self, user_id: &str, challenge_id: &str) -> Result<(), Error> {
        sqlx::query("DELETE FROM challenge_instances WHERE user_id = ? AND challenge_id = ?")
            .bind(user_id)
            .bind(challenge_id)
            .execute(&self.pool).await.map(|_| ())
    }

    pub async fn get_user_challenge_instances(&self, user_id: &str) -> Result<Vec<ChallengeInstance>, Error> {
        sqlx::query_as("SELECT * FROM challenge_instances WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(&self.pool).await
    }

    pub async fn get_queued_challenge_instances(&self) -> Result<Vec<ChallengeInstance>, Error> {
        sqlx::query_as("SELECT * FROM challenge_instances WHERE state LIKE 'queued_%'")
            .fetch_all(&self.pool).await
    }
}