use std::{env, fs, net::ToSocketAddrs, path::Path, sync::OnceLock};

use anyhow::Result;
use serde::Deserialize;

const CONF_PATH_ENV_VAR: &str = "XOCSP_CONF_PATH";
const DEF_CONF_PATH: &str = "/etc/xocsp/config.ron";
const MAX_CACHE_TTL_SEC: u32 = 2_462_400;
const DEF_BIND_HOST: &str = "127.0.0.1";
const DEF_BIND_PORT: u16 = 5396;
const DEF_CACHE_CAP: u64 = 1000;

static CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Debug, Deserialize, Clone)]
pub struct Config {
    #[serde(default = "def_bind_host")]
    pub bind_host: String,
    #[serde(default = "def_bind_port")]
    pub bind_port: u16,
    #[serde(alias = "database_path")]
    pub db_path: String,
    #[serde(default = "def_max_cache_ttl")]
    pub max_cache_ttl: u32,
    #[serde(default = "def_cache_cap")]
    pub cache_cap: u64,
    pub num_workers: Option<usize>,
}

impl Config {
    pub fn from_file(path: &str) -> Result<Config> {
        let content = fs::read_to_string(path)?;
        let config: Config = ron::from_str(&content)?;
        config.validate();
        Ok(config)
    }

    pub fn bind_addr(&self) -> impl ToSocketAddrs {
        (self.bind_host.as_str(), self.bind_port)
    }

    fn validate(&self) {
        if self.bind_host.is_empty() {
            panic!("Bind host cannot be empty")
        }
        if !(1000..=u16::MAX).contains(&self.bind_port) {
            panic!("Bind port must be between 1000 and {}", u16::MAX)
        }
        if self.db_path.is_empty() || !Path::new(&self.db_path).exists() {
            panic!("Database path doesn't exist")
        }
        if self.max_cache_ttl < 1 {
            panic!("Max cache TTL should be higher than 0")
        }
        if self.cache_cap < 1 {
            panic!("Cache capacity should be higher than 0")
        }
        if self.num_workers.is_some_and(|n| n < 1) {
            panic!("Number of workers should be higher than 0")
        }
    }
}

fn def_bind_host() -> String {
    DEF_BIND_HOST.to_string()
}

fn def_bind_port() -> u16 {
    DEF_BIND_PORT
}

fn def_max_cache_ttl() -> u32 {
    MAX_CACHE_TTL_SEC
}

fn def_cache_cap() -> u64 {
    DEF_CACHE_CAP
}

#[inline]
pub fn init_config() {
    let path = env::var(CONF_PATH_ENV_VAR).unwrap_or_else(|_| DEF_CONF_PATH.into());
    let config = Config::from_file(&path)
        .expect(format!("Could not parse config from path '{:?}'", path).as_str());
    _ = CONFIG.set(config).or_else(|_| {
        println!("Config already has been initialized");
        Ok::<(), Config>(())
    });
}

#[inline]
pub fn config() -> &'static Config {
    &CONFIG.get().expect("Config has not been initialized")
}
