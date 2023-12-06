use crate::axum::tracing::OtelLayer;
use opentelemetry::{
    global,
    trace::{TraceError, Tracer, TracerProvider as _},
};
use opentelemetry_sdk::{
    runtime::Tokio,
    trace::config as otConfig,
    trace::{Sampler, TracerProvider},
    Resource,
};
use opentelemetry_semantic_conventions as otconv;
use serde::{Deserialize, Serialize};
use std::{error::Error as StdError, sync::Arc};
use thiserror::Error as ThisError;
use tracing::{subscriber::SetGlobalDefaultError, Dispatch, Subscriber};
use tracing_opentelemetry::{OpenTelemetryLayer, PreSampledTracer};
use tracing_subscriber::{
    filter::{EnvFilter, ParseError},
    layer::SubscriberExt,
    registry::LookupSpan,
    reload::{self, Handle},
    Layer, Registry,
};

#[derive(Debug, ThisError)]
pub enum TracingBuildError {
    #[error(transparent)]
    SetGlobalTracing(#[from] SetGlobalDefaultError),
    #[error("Default log format could not be parsed")]
    DefaultLogError(#[from] ParseError),
    #[cfg(feature = "ot_app_insight")]
    #[error(transparent)]
    AppInsightConfigError(Box<dyn StdError + Send + Sync + 'static>),
    #[error(transparent)]
    TraceError(#[from] TraceError),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
#[serde(tag = "type")]
pub enum Telemetry {
    /// Disable telemetry
    None,

    /// Enable telemetry to the standard output
    StdOut,

    /// Enable Jaeger telemetry (https://www.jaegertracing.io)
    #[cfg(feature = "ot_jaeger")]
    Jaeger,

    /// Enable Zipkin telemetry (https://zipkin.io/)
    #[cfg(feature = "ot_zipkin")]
    Zipkin,

    /// Enable AppInsight telemetry
    #[cfg(feature = "ot_app_insight")]
    AppInsight { instrumentation_key: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TracingConfig {
    allow_reconfigure: bool,
    enable_console_log: bool,
    telemetry: Telemetry,
    default_level: Option<String>,
}

trait DynHandle: Send + Sync {
    fn reconfigure(&self, config: String) -> Result<(), String>;
}

impl<L, S> DynHandle for Handle<L, S>
where
    L: 'static + Layer<S> + From<EnvFilter> + Send + Sync,
    S: Subscriber,
{
    fn reconfigure(&self, mut new_config: String) -> Result<(), String> {
        new_config.retain(|c| !c.is_whitespace());
        let new_filter = new_config.parse::<EnvFilter>().map_err(|e| format!("{}", e))?;
        self.reload(new_filter).map_err(|e| format!("{}", e))
    }
}

#[derive(Debug, ThisError)]
#[error("Failed to update trace: {0}")]
pub struct TraceReconfigureError(String);

#[derive(Clone)]
pub struct TracingManager {
    reconfigure: Option<Arc<dyn DynHandle>>,
}

impl TracingManager {
    /// Create a Service and initialize the global tracing logger
    pub async fn new(service_name: &str, config: &TracingConfig) -> Result<Self, TracingBuildError> {
        let mut service = TracingManager { reconfigure: None };
        service.install_telemetry(service_name, config)?;
        Ok(service)
    }

    fn set_global_logger<L>(&mut self, tracing_pipeline: L) -> Result<(), TracingBuildError>
    where
        L: Into<Dispatch>,
    {
        tracing::dispatcher::set_global_default(tracing_pipeline.into())?;
        Ok(())
    }

    fn ot_layer<T>(tracer: T) -> OpenTelemetryLayer<Registry, T>
    where
        T: 'static + Tracer + PreSampledTracer + Send + Sync,
    {
        tracing_opentelemetry::layer()
            .with_tracked_inactivity(true)
            .with_tracer(tracer)
    }

    fn install_filter<T>(&mut self, config: &TracingConfig, pipeline: T) -> Result<(), TracingBuildError>
    where
        T: for<'a> LookupSpan<'a> + Subscriber + Send + Sync,
    {
        let env_filter = if let Some(default_level) = &config.default_level {
            EnvFilter::builder().parse(default_level)?
        } else {
            EnvFilter::from_default_env()
        };

        if config.allow_reconfigure {
            // enable filtering with reconfiguration capabilities
            let (reload_env_filter, reload_handle) = reload::Layer::new(env_filter);
            let pipeline = pipeline.with(reload_env_filter);
            self.reconfigure = Some(Arc::new(reload_handle));

            self.set_global_logger(pipeline)?;
            Ok(())
        } else {
            // enable filtering from the environment variables
            let pipeline = pipeline.with(env_filter);

            self.set_global_logger(pipeline)?;
            Ok(())
        }
    }

    fn install_logger<T>(&mut self, config: &TracingConfig, pipeline: T) -> Result<(), TracingBuildError>
    where
        T: for<'a> LookupSpan<'a> + Subscriber + Send + Sync,
    {
        if config.enable_console_log {
            let console_layer = tracing_subscriber::fmt::Layer::new().pretty();
            let pipeline = pipeline.with(console_layer);
            self.install_filter(config, pipeline)
        } else {
            self.install_filter(config, pipeline)
        }
    }

    fn install_pipeline<L>(&mut self, config: &TracingConfig, layer: L) -> Result<(), TracingBuildError>
    where
        L: Layer<Registry> + Send + Sync,
    {
        let pipeline = tracing_subscriber::registry().with(layer);
        self.install_logger(config, pipeline)
    }

    fn install_telemetry(&mut self, service_name: &str, config: &TracingConfig) -> Result<(), TracingBuildError> {
        let resource = Resource::new(vec![otconv::resource::SERVICE_NAME.string(service_name.to_string())]);

        match &config.telemetry {
            Telemetry::StdOut => {
                let exporter = opentelemetry_stdout::SpanExporter::default();
                let provider = TracerProvider::builder()
                    .with_simple_exporter(exporter)
                    .with_config(otConfig().with_resource(resource).with_sampler(Sampler::AlwaysOn))
                    .build();
                let tracer = provider.versioned_tracer(
                    "opentelemetry-stdout",
                    Some(env!("CARGO_PKG_VERSION")),
                    Some(otconv::SCHEMA_URL),
                    None,
                );
                let _ = global::set_tracer_provider(provider);
                self.install_pipeline(config, Self::ot_layer(tracer))
            }
            #[cfg(feature = "ot_jaeger")]
            Telemetry::Jaeger => {
                let tracer = opentelemetry_jaeger::new_agent_pipeline()
                    .with_trace_config(otConfig().with_resource(resource))
                    .with_service_name(service_name.to_string())
                    .install_batch(Tokio)?;
                self.install_pipeline(config, Self::ot_layer(tracer))
            }
            #[cfg(feature = "ot_zipkin")]
            Telemetry::Zipkin => {
                let tracer = opentelemetry_zipkin::new_pipeline()
                    .with_trace_config(otConfig().with_resource(resource))
                    .with_service_name(service_name.to_string())
                    .install_batch(Tokio)?;
                self.install_pipeline(config, Self::ot_layer(tracer))
            }
            #[cfg(feature = "ot_app_insight")]
            Telemetry::AppInsight { instrumentation_key } => {
                let tracer = opentelemetry_application_insights::new_pipeline_from_connection_string(
                    instrumentation_key.clone(),
                )
                .map_err(TracingBuildError::AppInsightConfigError)?
                .with_trace_config(otConfig().with_resource(resource))
                .with_service_name(service_name.to_string())
                .with_client(reqwest::Client::new())
                .install_batch(Tokio);
                self.install_pipeline(config, Self::ot_layer(tracer))
            }
            Telemetry::None => self.install_pipeline(config, EmptyLayer),
        }
    }

    pub fn reconfigure(&self, filter: String) -> Result<(), TraceReconfigureError> {
        if let Some(reconfigure) = &self.reconfigure {
            reconfigure.reconfigure(filter).map_err(TraceReconfigureError)?
        }
        Ok(())
    }

    pub fn to_layer(&self) -> OtelLayer {
        //todo: read route filtering from config
        OtelLayer::default()
    }
}

struct EmptyLayer;
impl<S: Subscriber> Layer<S> for EmptyLayer {}
