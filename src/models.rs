use std::ops::Add;
use std::time::{Duration, SystemTime};

use serde::Serialize;
use sqlx::{Decode, Encode, Sqlite};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};

#[derive(sqlx::FromRow)]
pub struct User {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub avatar: String,
    pub creation_time: TimeSinceEpoch
}

#[derive(sqlx::FromRow)]
pub struct ChallengeInstance {
    pub user_id: String,
    pub challenge_id: String,
    pub state: ChallengeInstanceState,
    pub details: Option<String>,
    pub start_time: TimeSinceEpoch
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "snake_case")]
pub enum ChallengeInstanceState {
    Stopped,
    Running,
    QueuedStart,
    QueuedRestart,
    QueuedStop
}

impl From<String> for ChallengeInstanceState {
    fn from(value: String) -> Self {
        value.as_str().into()
    }
}

impl From<&str> for ChallengeInstanceState {
    fn from(value: &str) -> Self {
        match value {
            "stopped" => ChallengeInstanceState::Stopped,
            "running" => ChallengeInstanceState::Running,
            "queued_start" => ChallengeInstanceState::QueuedStart,
            "queued_restart" => ChallengeInstanceState::QueuedRestart,
            "queued_stop" => ChallengeInstanceState::QueuedStop,
            v => panic!("unknown challenge instance state: {}", v)
        }
    }
}

impl From<&ChallengeInstanceState> for &str {
    fn from(value: &ChallengeInstanceState) -> Self {
        match value {
            ChallengeInstanceState::Stopped => "stopped",
            ChallengeInstanceState::Running => "running",
            ChallengeInstanceState::QueuedStart => "queued_start",
            ChallengeInstanceState::QueuedStop => "queued_stop",
            ChallengeInstanceState::QueuedRestart => "queued_restart"
        }
    }
}

impl sqlx::Type<Sqlite> for ChallengeInstanceState {
    fn type_info() -> SqliteTypeInfo {
        <&str as sqlx::Type<Sqlite>>::type_info()
    }
}

impl<'r> Decode<'r, Sqlite> for ChallengeInstanceState {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        let value = <&str as Decode<Sqlite>>::decode(value)?;
        Ok(value.into())
    }
}

impl<'q> Encode<'q, Sqlite> for ChallengeInstanceState {
    fn encode_by_ref(&self, buf: &mut Vec<SqliteArgumentValue<'q>>) -> Result<IsNull, BoxDynError> {
        let value: &str = self.into();
        <&str as Encode<Sqlite>>::encode(value, buf)
    }
}

pub struct TimeSinceEpoch(pub SystemTime);

impl TimeSinceEpoch {
    pub fn now() -> Self {
        TimeSinceEpoch(SystemTime::now())
    }
}

impl From<i64> for TimeSinceEpoch {
    fn from(value: i64) -> Self {
        TimeSinceEpoch(SystemTime::UNIX_EPOCH.add(Duration::from_millis(value as u64)))
    }
}

impl From<&TimeSinceEpoch> for i64 {
    fn from(value: &TimeSinceEpoch) -> Self {
        value.0.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() as i64
    }
}

impl sqlx::Type<Sqlite> for TimeSinceEpoch {
    fn type_info() -> SqliteTypeInfo {
        <i64 as sqlx::Type<Sqlite>>::type_info()
    }
}

impl<'r> Decode<'r, Sqlite> for TimeSinceEpoch {
    fn decode(value: SqliteValueRef<'r>) -> Result<Self, BoxDynError> {
        let value = <i64 as Decode<Sqlite>>::decode(value)?;
        Ok(value.into())
    }
}

impl<'q> Encode<'q, Sqlite> for TimeSinceEpoch {
    fn encode_by_ref(&self, buf: &mut Vec<SqliteArgumentValue<'q>>) -> Result<IsNull, BoxDynError> {
        let value: i64 = self.into();
        <i64 as Encode<Sqlite>>::encode(value, buf)
    }
}