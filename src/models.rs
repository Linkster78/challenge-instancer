use std::ops::Add;
use std::time::{Duration, SystemTime};

pub struct User {
    pub id: String,
    pub username: String,
    pub display_name: String,
    pub avatar: String,
    pub creation_time: i64
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

impl From<TimeSinceEpoch> for i64 {
    fn from(value: TimeSinceEpoch) -> Self {
        value.0.duration_since(SystemTime::UNIX_EPOCH).unwrap().as_millis() as i64
    }
}