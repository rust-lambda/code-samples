use figment::providers::Env;
use figment::Figment;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub(crate) struct EnvVars {
    pub table_name: String,
    pub queue_url: String,
}

impl EnvVars {
    pub fn load() -> Result<Self, figment::Error> {
        Figment::new()
            .merge(Env::raw().only(&["TABLE_NAME", "QUEUE_URL"]))
            .extract()
    }
}
