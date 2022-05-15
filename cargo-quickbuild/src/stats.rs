use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

pub struct Stats {
    start: Instant,
    init_done: Option<Instant>,
    build_done: Option<Instant>,
    tar_done: Option<Instant>,
}
impl Stats {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            init_done: None,
            build_done: None,
            tar_done: None,
        }
    }
    pub fn init_done(&mut self) {
        self.init_done.replace(Instant::now());
    }
    pub fn build_done(&mut self) {
        self.build_done.replace(Instant::now());
    }
    pub fn tar_done(&mut self) {
        self.tar_done.replace(Instant::now());
    }
}

mod duration_as_float_seconds {
    use std::time::Duration;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    pub fn serialize<S: Serializer>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error> {
        duration.as_secs_f64().serialize(serializer)
    }
    pub fn deserialize<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Duration, D::Error> {
        Ok(Duration::from_secs_f64(f64::deserialize(deserializer)?))
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct ComputedStats {
    #[serde(with = "duration_as_float_seconds")]
    init_duration: Duration,
    #[serde(with = "duration_as_float_seconds")]
    build_duration: Duration,
    #[serde(with = "duration_as_float_seconds")]
    tar_duration: Duration,
}

impl From<Stats> for ComputedStats {
    fn from(stats: Stats) -> Self {
        Self {
            init_duration: stats.init_done.unwrap() - stats.start,
            build_duration: stats.build_done.unwrap() - stats.init_done.unwrap(),
            tar_duration: stats.tar_done.unwrap() - stats.build_done.unwrap(),
        }
    }
}
