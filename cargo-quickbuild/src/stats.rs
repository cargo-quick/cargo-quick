use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};

pub struct Stats {
    start: Instant,
    init_done: Option<Instant>,
    untar_done: Option<Instant>,
    build_done: Option<Instant>,
    tar_done: Option<Instant>,
}
impl Stats {
    pub fn new() -> Self {
        Self {
            start: Instant::now(),
            init_done: None,
            untar_done: None,
            build_done: None,
            tar_done: None,
        }
    }
    pub fn init_done(&mut self) {
        self.init_done.replace(Instant::now());
        log::info!("init_duration: {:?}s", self.init_duration().as_secs_f64());
    }
    fn init_duration(&self) -> Duration {
        self.init_done.unwrap() - self.start
    }
    pub fn untar_done(&mut self) {
        self.untar_done.replace(Instant::now());
        log::info!("untar_duration: {:?}s", self.untar_duration().as_secs_f64());
    }
    fn untar_duration(&self) -> Duration {
        self.untar_done.unwrap() - self.init_done.unwrap()
    }
    pub fn build_done(&mut self) {
        self.build_done.replace(Instant::now());
        log::info!("build_duration: {:?}s", self.build_duration().as_secs_f64());
    }
    fn build_duration(&self) -> Duration {
        self.build_done.unwrap() - self.untar_done.unwrap()
    }
    pub fn tar_done(&mut self) {
        self.tar_done.replace(Instant::now());
        log::info!("tar_duration: {:?}s", self.tar_duration().as_secs_f64());
    }
    fn tar_duration(&self) -> Duration {
        self.tar_done.unwrap() - self.build_done.unwrap()
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
    untar_duration: Duration,
    #[serde(with = "duration_as_float_seconds")]
    build_duration: Duration,
    #[serde(with = "duration_as_float_seconds")]
    tar_duration: Duration,
}

impl From<Stats> for ComputedStats {
    fn from(stats: Stats) -> Self {
        Self {
            init_duration: stats.init_duration(),
            untar_duration: stats.untar_duration(),
            build_duration: stats.build_duration(),
            tar_duration: stats.tar_duration(),
        }
    }
}
