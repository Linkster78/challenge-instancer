use crate::models::{ChallengeInstance, ChallengeInstanceState, TimeSinceEpoch, User};
use sqlx::{Error, SqlitePool};

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool
}

pub enum ChallengeInstanceInsertionResult {
    Inserted,
    Exists,
    LimitReached
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

    pub async fn insert_challenge_instance(&self, instance: &ChallengeInstance, max_instance_count: u32) -> Result<ChallengeInstanceInsertionResult, Error> {
        let mut tx = self.pool.begin().await?;

        let result = sqlx::query("UPDATE users SET instance_count = instance_count + 1 WHERE id = ? AND instance_count < ?")
            .bind(&instance.user_id)
            .bind(max_instance_count)
            .execute(&mut *tx).await?;

        if result.rows_affected() == 0 {
            return Ok(ChallengeInstanceInsertionResult::LimitReached);
        }

        let result = sqlx::query("INSERT INTO challenge_instances VALUES (?, ?, ?, ?, ?)")
            .bind(&instance.user_id)
            .bind(&instance.challenge_id)
            .bind(&instance.state)
            .bind(&instance.details)
            .bind(&instance.stop_time)
            .execute(&mut *tx).await;

        match result {
            Ok(_) => {
                tx.commit().await?;
                Ok(ChallengeInstanceInsertionResult::Inserted)
            }
            Err(Error::Database(err)) if err.is_unique_violation() => Ok(ChallengeInstanceInsertionResult::Exists),
            Err(err) => Err(err)
        }
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

    pub async fn populate_running_challenge_instance(&self, user_id: &str, challenge_id: &str, details: &str, stop_time: TimeSinceEpoch) -> Result<(), Error> {
        sqlx::query("UPDATE challenge_instances SET state = ?, details = ?, stop_time = ? WHERE user_id = ? AND challenge_id = ?")
            .bind(ChallengeInstanceState::Running)
            .bind(details)
            .bind(stop_time)
            .bind(user_id)
            .bind(challenge_id)
            .execute(&self.pool).await.map(|_| ())
    }

    pub async fn extend_challenge_instance(&self, user_id: &str, challenge_id: &str, stop_time: TimeSinceEpoch) -> Result<bool, Error> {
        let result = sqlx::query("UPDATE challenge_instances SET stop_time = ? WHERE state = ? AND user_id = ? AND challenge_id = ?")
            .bind(stop_time)
            .bind(ChallengeInstanceState::Running)
            .bind(user_id)
            .bind(challenge_id)
            .execute(&self.pool).await?;
        Ok(result.rows_affected() == 1)
    }

    pub async fn delete_challenge_instance(&self, user_id: &str, challenge_id: &str) -> Result<(), Error> {
        let mut tx = self.pool.begin().await?;

        sqlx::query("DELETE FROM challenge_instances WHERE user_id = ? AND challenge_id = ?")
            .bind(user_id)
            .bind(challenge_id)
            .execute(&mut *tx).await?;

        sqlx::query("UPDATE users SET instance_count = instance_count - 1 WHERE id = ?")
            .bind(user_id)
            .execute(&mut *tx).await?;

        tx.commit().await
    }

    pub async fn get_user_challenge_instances(&self, user_id: &str) -> Result<Vec<ChallengeInstance>, Error> {
        sqlx::query_as("SELECT * FROM challenge_instances WHERE user_id = ?")
            .bind(user_id)
            .fetch_all(&self.pool).await
    }

    pub async fn get_challenge_instances(&self) -> Result<Vec<ChallengeInstance>, Error> {
        sqlx::query_as("SELECT * FROM challenge_instances")
            .fetch_all(&self.pool).await
    }
}