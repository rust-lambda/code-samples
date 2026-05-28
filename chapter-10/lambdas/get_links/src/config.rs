use figment::providers::Env;
use figment::Figment;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Config {
    pub table_name: String,
    pub rate_limit_table_name: String,
    pub rate_limit_max_requests: u32,
    pub rate_limit_window_secs: u64,
}

impl Config {
    pub fn load() -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Env::raw().only(&[
                "TABLE_NAME",
                "RATE_LIMIT_TABLE_NAME",
                "RATE_LIMIT_MAX_REQUESTS",
                "RATE_LIMIT_WINDOW_SECS",
            ]))
            .extract()
    }
}
