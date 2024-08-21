use std::ops::{Add, Sub};
use std::time::{Duration, SystemTime};

use serde::{Serialize, Serializer};
use sqlx::encode::IsNull;
use sqlx::error::BoxDynError;
use sqlx::sqlite::{SqliteArgumentValue, SqliteTypeInfo, SqliteValueRef};
use sqlx::{Decode, Encode, Sqlite};

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
    pub stop_time: Option<TimeSinceEpoch>
}

#[derive(Debug, Serialize, Clone, Ord, PartialOrd, Eq, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum ChallengeInstanceState {
    Stopped,
    Running,
    QueuedStart,
    QueuedRestart,
    QueuedStop,
}

impl ChallengeInstanceState {
    pub fn is_queued(&self) -> bool {
        match self {
            ChallengeInstanceState::QueuedStop | ChallengeInstanceState::QueuedStart | ChallengeInstanceState::QueuedRestart => true,
            _ => false
        }
    }
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

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct TimeSinceEpoch(pub SystemTime);

impl TimeSinceEpoch {
    pub fn now() -> Self {
        TimeSinceEpoch(SystemTime::now())
    }
    pub fn zero() -> Self { TimeSinceEpoch(SystemTime::UNIX_EPOCH) }
    pub fn from_now(duration: Duration) -> Self { TimeSinceEpoch(SystemTime::now().add(duration)) }
}

impl Sub for &TimeSinceEpoch {
    type Output = Duration;

    fn sub(self, rhs: Self) -> Self::Output {
        self.0.duration_since(rhs.0).unwrap()
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

impl Serialize for TimeSinceEpoch {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer
    {
        let since_epoch: i64 = self.into();
        serializer.serialize_i64(since_epoch)
    }
}