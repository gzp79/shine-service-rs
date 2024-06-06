use crate::azure::azure_keyvault_config::AzureKeyvaultConfigSource;
use azure_core::auth::TokenCredential;
use azure_identity::{AzureCliCredential, EnvironmentCredential, TokenCredentialOptions};
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
    pub before_layers: Vec<String>,
    pub after_layers: Vec<String>,
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

        enum Layer<'a> {
            Base,
            Environment,
            Config(&'a str, &'a str, Option<&'a str>),
        }
        impl<'a> Layer<'a> {
            fn from_layer(layer: &'a str) -> Result<Self, ConfigError> {
                if layer == "environment" {
                    Ok(Layer::Environment)
                } else {
                    let mut tokens = layer.splitn(2, "://");
                    let schema = tokens.next().ok_or(ConfigError::FileParse {
                        uri: Some(layer.to_owned()),
                        cause: "Invalid config layer".into(),
                    })?;
                    Ok(Self::Config(schema, layer, tokens.next()))
                }
            }
        }

        let mut layers = Vec::with_capacity(self.before_layers.len() + self.after_layers.len() + 1);
        for l in self.before_layers.iter().map(|x| Layer::from_layer(x.as_str())) {
            layers.push(l?);
        }
        layers.push(Layer::Base);
        for l in self.after_layers.iter().map(|x| Layer::from_layer(x.as_str())) {
            layers.push(l?);
        }

        let mut azure_credentials: Option<Arc<dyn TokenCredential>> = None;
        for layer in layers {
            match layer {
                Layer::Base => {
                    builder = builder.add_source(File::from(Path::new(&format!("server_config.{}.json", self.stage))));
                }
                Layer::Environment => {
                    builder = builder.add_source(Environment::default().separator("--"));
                }
                Layer::Config("file", url, path) => {
                    let path = path.ok_or(ConfigError::FileParse {
                        uri: Some(url.to_owned()),
                        cause: "Missing file path".into(),
                    })?;
                    builder = builder.add_source(File::from(Path::new(path)));
                }
                Layer::Config("file?", url, path) => {
                    let path = path.ok_or(ConfigError::FileParse {
                        uri: Some(url.to_owned()),
                        cause: "Missing file path".into(),
                    })?;

                    if Path::new(path).exists() {
                        log::info!("Adding optional config file {}...", path);
                        builder = builder.add_source(File::from(Path::new(path)));
                    }
                }
                Layer::Config("azk", url, path) => {
                    let path = path.ok_or(ConfigError::FileParse {
                        uri: Some(url.to_owned()),
                        cause: "Missing azure keyvault location".into(),
                    })?;
                    if azure_credentials.is_none() {
                        azure_credentials = if env::var("AZURE_TENANT_ID").is_ok() {
                            let credentials = EnvironmentCredential::create(TokenCredentialOptions::default())
                                .map_err(|err| ConfigError::FileParse {
                                    uri: Some(url.to_owned()),
                                    cause: err.into(),
                                })?;
                            log::info!("Getting azure credentials through environment...");
                            Some(Arc::new(credentials))
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
                Layer::Config(schema, url, _) => {
                    return Err(ConfigError::FileParse {
                        uri: Some(url.to_owned()),
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
