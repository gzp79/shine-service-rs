use azure_core::auth::TokenCredential;
use azure_security_keyvault::SecretClient;
use config::{ConfigError, Map as ConfigMap, Source as ConfigSource, Value as ConfigValue};
use futures::StreamExt;
use std::sync::Arc;
use thiserror::Error as ThisError;
use tokio::runtime::Handle as RtHandle;

#[derive(Debug, ThisError)]
#[error("Azure core error: {0}")]
pub struct AzureKeyvaultConfigError(#[source] azure_core::Error);

impl From<AzureKeyvaultConfigError> for ConfigError {
    fn from(err: AzureKeyvaultConfigError) -> Self {
        ConfigError::Foreign(Box::new(err))
    }
}

#[derive(Clone, Debug)]
pub struct AzureKeyvaultConfigSource {
    rt_handle: RtHandle,
    keyvault_url: String,
    client: SecretClient,
}

impl AzureKeyvaultConfigSource {
    pub fn new(
        rt_handle: &RtHandle,
        azure_credentials: Arc<dyn TokenCredential>,
        keyvault_url: &str,
    ) -> Result<AzureKeyvaultConfigSource, ConfigError> {
        let client = SecretClient::new(keyvault_url, azure_credentials).map_err(AzureKeyvaultConfigError)?;
        Ok(Self {
            rt_handle: rt_handle.clone(),
            keyvault_url: keyvault_url.to_owned(),
            client,
        })
    }
}

impl ConfigSource for AzureKeyvaultConfigSource {
    fn clone_into_box(&self) -> Box<dyn ConfigSource + Send + Sync> {
        Box::new(self.clone())
    }

    fn collect(&self) -> Result<ConfigMap<String, ConfigValue>, ConfigError> {
        tokio::task::block_in_place(|| {
            self.rt_handle.block_on(async {
                let mut config = ConfigMap::new();

                log::info!("Loading secrets from {} ...", self.keyvault_url);
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
                                config.insert(path, secret.value.into());
                            }
                        }
                    }
                }

                log::info!("{:#?}", config);
                Ok(config)
            })
        })
    }
}
