use crate::azure::azure_keyvault_config::AzureKeyvaultConfigSource;
use azure_core::auth::TokenCredential;
use azure_identity::{AzureCliCredential, EnvironmentCredential};
use config::{builder::AsyncState, Config, ConfigBuilder, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::{env, path::Path, sync::Arc};

pub const DEFAULT_CONFIG_FILE: &str = "server_config.json";
pub const DEFAULT_DEV_CONFIG_FILE: &str = "server_config.dev.json";
pub const DEFAULT_LOCAL_CONFIG_FILE: &str = "temp/server_config.json";

/// Partial configuration required for early setup.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CoreConfig {
    pub stage: String,
    pub version: String,
    pub layers: Vec<String>,
}

impl CoreConfig {
    pub fn new(stage: &str) -> Result<Self, ConfigError> {
        let builder = Config::builder()
            .add_source(File::from(Path::new(&format!("server_config.{}.json", stage))))
            .add_source(File::from(Path::new("server_version.json")));

        let s = builder.build()?;
        let cfg: CoreConfig = s.try_deserialize()?;

        log::info!("pre-init configuration: {:#?}", cfg);
        Ok(cfg)
    }

    pub fn create_config_builder(&self) -> Result<ConfigBuilder<AsyncState>, ConfigError> {
        let mut builder = ConfigBuilder::<AsyncState>::default();

        // make sure self is added
        let mut layers = self.layers.clone();
        if !layers.iter().any(|x| x == "self") {
            layers.push("self".into());
        }

        for layer in layers {
            let mut tokens = layer.splitn(2, "://");
            let schema = tokens.next().ok_or(ConfigError::FileParse {
                uri: Some(layer.to_owned()),
                cause: "Invalid config layer url".into(),
            })?;

            let path = tokens.next();

            let mut azure_credentials: Option<Arc<dyn TokenCredential>> = None;

            match schema {
                "file" => {
                    let path = path.ok_or(ConfigError::FileParse {
                        uri: Some(layer.to_owned()),
                        cause: "Missing file path".into(),
                    })?;
                    builder = builder.add_source(File::from(Path::new(path)));
                }
                "file?" => {
                    let path = path.ok_or(ConfigError::FileParse {
                        uri: Some(layer.to_owned()),
                        cause: "Missing file path".into(),
                    })?;

                    if Path::new(path).exists() {
                        builder = builder.add_source(File::from(Path::new(path)));
                    }
                }
                "self" => {
                    if path.is_some() {
                        return Err(ConfigError::FileParse {
                            uri: Some(layer.to_owned()),
                            cause: "Missing file path".into(),
                        });
                    }
                    builder = builder.add_source(File::from(Path::new(&format!("server_config.{}.json", self.stage))));
                }
                "azk" => {
                    let path = path.ok_or(ConfigError::FileParse {
                        uri: Some(layer.to_owned()),
                        cause: "Missing azure keyvault location".into(),
                    })?;
                    if azure_credentials.is_none() {
                        azure_credentials = if env::var("AZURE_TENANT_ID").is_ok() {
                            log::info!("Getting azure credentials through environment...");
                            Some(Arc::new(EnvironmentCredential::default()))
                        } else {
                            log::info!("Getting azure credentials through azure cli...");
                            Some(Arc::new(AzureCliCredential::new()))
                        };
                    }
                    let azure_credentials = azure_credentials.clone().unwrap();
                    let keyvault_url = format!("https://{}", path);
                    let keyvault = AzureKeyvaultConfigSource::new(azure_credentials.clone(), &keyvault_url)?;
                    builder = builder.add_async_source(keyvault);
                }
                "environment" => {
                    builder = builder.add_source(Environment::default().separator("--"));
                }
                _ => {
                    return Err(ConfigError::FileParse {
                        uri: Some(layer.to_owned()),
                        cause: format!("Unsupported schema, {schema}").into(),
                    })
                }
            }
        }

        builder = builder
            //.set_override("stage", self.stage.clone())?
            .set_override("version", self.version.clone())?;

        Ok(builder)
    }
}
