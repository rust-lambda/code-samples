use figment::providers::Env;
use figment::Figment;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct Config {
    pub table_name: String,
    pub stream_name: String,
}

impl Config {
    pub fn load() -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Env::raw().only(&["TABLE_NAME", "STREAM_NAME"]))
            .extract()
    }
}
