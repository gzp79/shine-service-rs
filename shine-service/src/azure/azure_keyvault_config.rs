use async_trait::async_trait;
use azure_core::auth::TokenCredential;
use azure_security_keyvault::SecretClient;
use config::{
    AsyncSource as ConfigAsyncSource, ConfigError, Map as ConfigMap, Value as ConfigValue, ValueKind as ConfigValueKind,
};
use futures::StreamExt;
use std::sync::Arc;
use thiserror::Error as ThisError;

#[derive(Debug, ThisError)]
#[error("Azure core error: {0}")]
pub struct AzureKeyvaultConfigError(#[source] azure_core::Error);

impl From<AzureKeyvaultConfigError> for ConfigError {
    fn from(err: AzureKeyvaultConfigError) -> Self {
        log::error!("{:?}", err);
        ConfigError::Foreign(Box::new(err))
    }
}

#[derive(Clone, Debug)]
pub struct AzureKeyvaultConfigSource {
    keyvault_url: String,
    client: SecretClient,
}

impl AzureKeyvaultConfigSource {
    pub fn new(
        azure_credentials: Arc<dyn TokenCredential>,
        keyvault_url: &str,
    ) -> Result<AzureKeyvaultConfigSource, ConfigError> {
        let client = SecretClient::new(keyvault_url, azure_credentials).map_err(AzureKeyvaultConfigError)?;
        Ok(Self {
            keyvault_url: keyvault_url.to_owned(),
            client,
        })
    }
}

#[async_trait]
impl ConfigAsyncSource for AzureKeyvaultConfigSource {
    async fn collect(&self) -> Result<ConfigMap<String, ConfigValue>, ConfigError> {
        let mut config = ConfigMap::new();

        log::info!("Loading secrets from {} ...", self.keyvault_url);
        let origin = self.keyvault_url.to_string();
        let mut stream = self.client.list_secrets().into_stream();
        while let Some(response) = stream.next().await {
            let response = response.map_err(AzureKeyvaultConfigError)?;
            for raw in &response.value {
                let key = raw.id.split('/').last();
                if let Some(key) = key {
                    let path = key.replace('-', ".");
                    log::info!("Reading secret {:?}", key);
                    let secret = self
                        .client
                        .get(key)
                        .into_future()
                        .await
                        .map_err(AzureKeyvaultConfigError)?;
                    if secret.attributes.enabled {
                        let value = secret.value;

                        // try to parse value, as conversion from string to a concrete type is not automatic.
                        let value = if let Ok(parsed) = value.parse::<i64>() {
                            ConfigValueKind::I64(parsed)
                        } else {
                            ConfigValueKind::String(value)
                        };

                        config.insert(path, ConfigValue::new(Some(&origin), value));
                    }
                }
            }
        }

        log::info!("keyvault config: {:#?}", config);
        Ok(config)
    }
}
