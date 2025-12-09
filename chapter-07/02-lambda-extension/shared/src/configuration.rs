use async_trait::async_trait;
use figment::providers::{Env, Format, Json, Serialized};
use figment::Figment;
use serde::{Deserialize, Serialize};

#[cfg(any(test, feature = "mocks"))]
use mockall::{automock, predicate::*};

#[cfg_attr(any(test, feature = "mocks"), automock)]
#[async_trait]
pub trait Config {
    async fn refresh(&self) -> Configuration;
}

// The LogLevel enum is here to illustrate how to parse an enum from configuration, in the `real world`
// you would typically use a LogLevel that comes from a logging crate.
// you'll see that in the chapter on observability
#[derive(Default, Debug, Serialize, Deserialize)]
pub enum LogLevel {
    Trace,
    #[default]
    Info,
    Warn,
    Error,
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Configuration {
    pub(crate) table_name: String,
    pub(crate) log_level: LogLevel,
    pub(crate) api_key: String,
}

pub struct ConfigurationManager {
    http_client: reqwest::Client,
}

impl ConfigurationManager {
    pub fn new() -> ConfigurationManager {
        ConfigurationManager {
            http_client: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(2))
                .build()
                .unwrap(),
        }
    }

    async fn load(&self) -> Configuration {
        let mut config = Figment::from(Serialized::defaults(Configuration::default()))
            // .merge overrides any existing values with new values the environment
            .merge(Env::prefixed("APP_"));

        let ssm_configuration = &self.load_from_ssm().await;
        config = match ssm_configuration {
            Ok(ssm_config) => config.merge(Json::string(&ssm_config)),
            Err(_) => config,
        };

        let secret_manager_configuration = &self.load_from_secret_manager().await;
        config = match secret_manager_configuration {
            Ok(secret_config) => config
                // .merge overrides any existing values with new values from this JSON
                .merge(Json::string(&secret_config)),
            Err(_) => config,
        };

        let config = config.extract();

        match config {
            Ok(config) => {
                println!("{:?}", config);
                config
            }
            Err(e) => {
                eprintln!("Failed to load configuration: {:?}", e);
                Configuration::default()
            }
        }
    }

    async fn load_from_ssm(&self) -> Result<String, ()> {
        let ssm_parameter_name = std::env::var("CONFIGURATION_PARAMETER_NAME");

        if ssm_parameter_name.is_err() {
            println!("'SSM_PARAMETER_NAME' not set");
            return Err(());
        }

        let url = format!(
            "systemsmanager/parameters/get/?name={}",
            ssm_parameter_name.unwrap()
        );

        let extension = &self.load_from_extension(url).await;

        match extension {
            Ok(extension_config) => {
                let json_value: serde_json::Value =
                    serde_json::from_str(extension_config).map_err(|_| ())?;
                let parameter_value = json_value["Parameter"]["Value"]
                    .as_str()
                    .ok_or(())?
                    .to_string();
                Ok(parameter_value.clone())
            }
            Err(e) => {
                println!("Failed to load configuration from SSM: {:?}", e);
                Err(())
            }
        }
    }

    async fn load_from_secret_manager(&self) -> Result<String, ()> {
        let configuration_secret_id = std::env::var("SECRET_MANAGER_SECRET_ID");

        if configuration_secret_id.is_err() {
            println!("'SECRET_MANAGER_SECRET_ID' not set");
            return Err(());
        }

        let url = format!(
            "secretsmanager/get/?secretId={}",
            configuration_secret_id.unwrap()
        );

        let extension = &self.load_from_extension(url).await;

        match extension {
            Ok(extension_config) => {
                let json_value: serde_json::Value =
                    serde_json::from_str(extension_config).map_err(|_| ())?;
                let parameter_value = json_value["SecretString"].as_str().ok_or(())?.to_string();
                Ok(parameter_value.clone())
            }
            Err(e) => {
                println!("Failed to load configuration from secrets manager: {:?}", e);
                Err(())
            }
        }
    }

    async fn load_from_extension(&self, url: String) -> Result<String, ()> {
        let extension_port = std::env::var("PARAMETERS_SECRETS_EXTENSION_HTTP_PORT");
        let session_token = std::env::var("AWS_SESSION_TOKEN");

        if extension_port.is_err() || session_token.is_err() {
            println!("'PARAMETERS_SECRETS_EXTENSION_HTTP_PORT' or 'AWS_SESSION_TOKEN' not set");
            return Err(());
        }

        let ssm_extension_response = &self
            .http_client
            .get(format!(
                "http://localhost:{}/{}",
                extension_port.unwrap(),
                url
            ))
            .header("X-Aws-Parameters-Secrets-Token", session_token.unwrap())
            .send()
            .await
            .map_err(|e| {
                println!("Failed to make HTTP request to extension: {:?}", e);
                ()
            })?
            .text()
            .await
            .map_err(|e| {
                println!("Failed to parse response text: {:?}", e);
                ()
            })?;

        Ok(ssm_extension_response.clone())
    }
}

#[async_trait]
impl Config for ConfigurationManager {
    async fn refresh(&self) -> Configuration {
        self.load().await
    }
}

impl std::fmt::Display for Configuration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.log_level {
            LogLevel::Trace | LogLevel::Info => write!(
                f,
                "Configuration {{ table_name: {}, log_level: {:?}, api_key: {:?} }}",
                self.table_name, self.log_level, self.api_key
            ),
            _ => write!(f, "Configuration loaded successfully",),
        }
    }
}

#[cfg(test)]
mod tests {
    use figment::{
        providers::{Env, Format, Json, Serialized},
        Figment,
    };

    use crate::configuration::{Configuration, LogLevel};

    #[tokio::test]
    async fn when_valid_configuration_should_load() {
        figment::Jail::expect_with(|jail| {
            jail.set_env("APP_TABLE_NAME", "james-test-table");

            let config: Configuration = Figment::new()
                .merge(Env::prefixed("APP_"))
                .merge(Json::string(stringify!({
                    "log_level": "Info"
                })))
                .merge(Json::string(stringify!({
                    "api_key": "my-test-api-key"
                })))
                .extract()
                .unwrap();

            assert_eq!(config.table_name, "james-test-table");
            assert!(matches!(config.log_level, LogLevel::Info));

            Ok(())
        });
    }

    #[tokio::test]
    async fn when_valid_configuration_should_load_join_overrides() {
        figment::Jail::expect_with(|jail| {
            jail.set_env("APP_TABLE_NAME", "james-test-table");

            let config: Configuration = Figment::new()
                .merge(Env::prefixed("APP_"))
                .merge(Json::string(stringify!({
                    "table_name": "james-test-table-override",
                    "log_level": "Error",
                    "api_key": "my-test"
                })))
                .extract()
                .unwrap();

            assert_eq!(config.table_name, "james-test-table-override");
            assert!(matches!(config.log_level, LogLevel::Error));

            Ok(())
        });
    }

    #[tokio::test]
    async fn when_using_defaults_values_should_be_set() {
        figment::Jail::expect_with(|jail| {
            jail.set_env("APP_TABLE_NAME", "james-test-table");

            let config: Configuration =
                Figment::from(Serialized::defaults(Configuration::default()))
                    .extract()
                    .unwrap();

            assert_eq!(config.table_name, "");
            assert_eq!(config.api_key, "");
            assert!(matches!(config.log_level, LogLevel::Info));

            Ok(())
        });
    }
}
