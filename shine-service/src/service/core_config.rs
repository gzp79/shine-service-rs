use crate::azure::azure_keyvault_config::AzureKeyvaultConfigSource;
use azure_core::auth::TokenCredential;
use azure_identity::{AzureCliCredential, EnvironmentCredential};
use config::{builder::AsyncState, Config, ConfigBuilder, ConfigError, Environment, File};
use serde::{Deserialize, Serialize};
use std::{env, path::Path, sync::Arc};

pub const DEFAULT_CONFIG_FILE: &str = "server_config.json";
pub const DEFAULT_LOCAL_CONFIG_FILE: &str = "temp/server_config.json";

/// Partial configuration required for early setup. These parameters shall not be altered
/// in the other layers.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct CoreConfig {
    pub version: String,
    pub shared_keyvault: Option<String>,
    pub private_keyvault: Option<String>,
}

impl CoreConfig {
    pub fn new() -> Result<CoreConfig, ConfigError> {
        let builder = Config::builder()
            .add_source(File::from(Path::new(DEFAULT_CONFIG_FILE)))
            .add_source(Environment::default().separator("--"));

        let s = builder.build()?;
        let cfg: CoreConfig = s.try_deserialize()?;

        log::info!("pre-init configuration: {:#?}", cfg);
        Ok(cfg)
    }

    pub fn create_config_builder(&self) -> Result<ConfigBuilder<AsyncState>, ConfigError> {
        let mut builder = ConfigBuilder::<AsyncState>::default();
        builder = builder.add_source(File::from(Path::new(DEFAULT_CONFIG_FILE)));

        {
            let azure_credentials: Arc<dyn TokenCredential> = if env::var("AZURE_TENANT_ID").is_ok() {
                log::info!("Getting azure credentials through environment...");
                Arc::new(EnvironmentCredential::default())
            } else {
                log::info!("Getting azure credentials through azure cli...");
                Arc::new(AzureCliCredential::new())
            };

            log::info!("Checking shared keyvault...");
            let shared_keyvault = self
                .shared_keyvault
                .as_ref()
                .map(|uri| AzureKeyvaultConfigSource::new(azure_credentials.clone(), uri))
                .transpose()?;
            if let Some(shared_keyvault) = shared_keyvault {
                builder = builder.add_async_source(shared_keyvault)
            }

            log::info!("Checking private keyvault...");
            let private_keyvault = self
                .private_keyvault
                .as_ref()
                .map(|uri| AzureKeyvaultConfigSource::new(azure_credentials.clone(), uri))
                .transpose()?;
            if let Some(private_keyvault) = private_keyvault {
                builder = builder.add_async_source(private_keyvault)
            }
        }

        if Path::new(DEFAULT_LOCAL_CONFIG_FILE).exists() {
            builder = builder.add_source(File::from(Path::new(DEFAULT_LOCAL_CONFIG_FILE)));
        }
        builder = builder.add_source(Environment::default().separator("--"));

        Ok(builder)
    }
}
