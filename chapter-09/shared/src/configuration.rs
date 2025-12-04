use aws_sdk_ssm::Client;
use figment::providers::{Env, Format, Json, Serialized};
use figment::Figment;
use serde::{Deserialize, Serialize};

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct Configuration {
    pub table_name: String,
}

impl Configuration {
    pub async fn load(ssm_client: &Client, secret_client: &aws_sdk_secretsmanager::Client) -> Self {
        let mut config = Figment::from(Serialized::defaults(Configuration::default()))
            // .merge fills in any missing values from the environment
            .merge(Env::prefixed("APP_"));

        let ssm_configuration = Configuration::load_from_ssm(ssm_client).await;
        config = match ssm_configuration {
            Ok(ssm_config) => config.merge(Json::string(&ssm_config)),
            Err(_) => config,
        };

        let secret_manager_configuration =
            Configuration::load_from_secret_manager(secret_client).await;
        config = match secret_manager_configuration {
            Ok(secret_config) => config
                // .join overrides any existing values with new values from this JSON
                .join(Json::string(&secret_config)),
            Err(_) => config,
        };

        let config = config.extract();

        match config {
            Ok(config) => {
                println!("{}", config);
                config
            }
            Err(e) => {
                eprintln!("Failed to load configuration: {:?}", e);
                Configuration::default()
            }
        }
    }

    async fn load_from_ssm(ssm_client: &Client) -> Result<String, ()> {
        let configuration_ssm_parameter_name = std::env::var("CONFIGURATION_PARAMETER_NAME");

        let configuration_ssm_parameter_name = match configuration_ssm_parameter_name {
            Ok(name) => name,
            Err(e) => {
                eprintln!("Failed to load configuration parameter name: {:?}", e);
                String::new()
            }
        };
        if configuration_ssm_parameter_name.len() > 0 {
            let ssm_configuration = ssm_client
                .get_parameter()
                .name(configuration_ssm_parameter_name)
                .with_decryption(true)
                .send()
                .await;

            return match ssm_configuration {
                Ok(config) => Ok(config.parameter.unwrap().value.unwrap()),
                Err(_) => Err(()),
            };
        }

        Err(())
    }

    async fn load_from_secret_manager(
        secret_client: &aws_sdk_secretsmanager::Client,
    ) -> Result<String, ()> {
        let configuration_secret_id = std::env::var("SECRET_MANAGER_SECRET_ID");

        let configuration_secret_id = match configuration_secret_id {
            Ok(name) => name,
            Err(_) => String::new(),
        };
        if configuration_secret_id.len() > 0 {
            let secret_value = secret_client
                .get_secret_value()
                .secret_id(configuration_secret_id)
                .send()
                .await;

            return match secret_value {
                Ok(value) => Ok(value.secret_string.unwrap()),
                Err(_) => Err(()),
            };
        }

        Err(())
    }
}

impl std::fmt::Display for Configuration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Configuration loaded successfully",)
    }
}

#[cfg(test)]
mod tests {
    use figment::{
        providers::{Env, Format, Json},
        Figment,
    };

    use crate::configuration::Configuration;

    #[tokio::test]
    async fn when_valid_configuration_should_load() {
        figment::Jail::expect_with(|jail| {
            jail.set_env("APP_TABLE_NAME", "james-test-table");

            let config: Configuration = Figment::new()
                .merge(Env::prefixed("APP_"))
                .merge(Json::string(stringify!({
                    "log_level": "INFO"
                })))
                .extract()
                .unwrap();

            assert_eq!(config.table_name, "james-test-table");

            Ok(())
        });
    }

    #[tokio::test]
    async fn when_valid_configuration_should_load_join_overrides() {
        figment::Jail::expect_with(|jail| {
            jail.set_env("APP_TABLE_NAME", "james-test-table");

            let config: Configuration = Figment::new()
                .merge(Env::prefixed("APP_"))
                .join(Json::string(stringify!({
                    "table_name": "james-test-table-override",
                    "log_level": "ERROR"
                })))
                .extract()
                .unwrap();

            assert_eq!(config.table_name, "james-test-table-override");

            Ok(())
        });
    }
}
