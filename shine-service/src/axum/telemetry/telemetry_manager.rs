use crate::axum::telemetry::OtelLayer;
use opentelemetry::{
    global,
    metrics::{Meter, MeterProvider as _},
    trace::{TraceError, Tracer, TracerProvider as _},
};
use opentelemetry_sdk::{
    metrics::MeterProvider,
    runtime::Tokio,
    trace::config as otConfig,
    trace::{Sampler, TracerProvider},
    Resource,
};
use opentelemetry_semantic_conventions as otconv;
use prometheus::{Encoder, Registry as PromRegistry, TextEncoder};
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
pub enum TelemetryBuildError {
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
pub enum Tracing {
    /// Disable tracing
    None,

    /// Enable tracing to the standard output
    StdOut,

    /// Enable Jaeger tracing (https://www.jaegertracing.io)
    #[cfg(feature = "ot_jaeger")]
    Jaeger,

    /// Enable Zipkin tracing (https://zipkin.io/)
    #[cfg(feature = "ot_zipkin")]
    Zipkin,

    /// Enable AppInsight tracing
    #[cfg(feature = "ot_app_insight")]
    AppInsight { instrumentation_key: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TelemetryConfig {
    allow_reconfigure: bool,
    enable_console_log: bool,
    metrics: bool,
    tracing: Tracing,
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
pub struct TelemetryManager {
    reconfigure: Option<Arc<dyn DynHandle>>,
    meter: Option<Meter>,
    prom_registry: Option<PromRegistry>,
}

impl TelemetryManager {
    /// Create a Service and initialize the global tracing logger
    pub async fn new(service_name: &str, config: &TelemetryConfig) -> Result<Self, TelemetryBuildError> {
        let mut service = TelemetryManager {
            reconfigure: None,
            meter: None,
            prom_registry: None,
        };
        service.install_telemetry(service_name, config)?;
        Ok(service)
    }

    fn set_global_tracing<L>(&mut self, tracing_pipeline: L) -> Result<(), TelemetryBuildError>
    where
        L: Into<Dispatch>,
    {
        tracing::dispatcher::set_global_default(tracing_pipeline.into())?;
        Ok(())
    }

    fn install_tracing_with_filter<T>(
        &mut self,
        config: &TelemetryConfig,
        pipeline: T,
    ) -> Result<(), TelemetryBuildError>
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

            self.set_global_tracing(pipeline)?;
            Ok(())
        } else {
            // enable filtering from the environment variables
            let pipeline = pipeline.with(env_filter);

            self.set_global_tracing(pipeline)?;
            Ok(())
        }
    }

    fn install_tracing_layer<L>(&mut self, config: &TelemetryConfig, layer: L) -> Result<(), TelemetryBuildError>
    where
        L: Layer<Registry> + Send + Sync,
    {
        let pipeline = tracing_subscriber::registry().with(layer);
        if config.enable_console_log {
            let console_layer = tracing_subscriber::fmt::Layer::new().pretty();
            let pipeline = pipeline.with(console_layer);
            self.install_tracing_with_filter(config, pipeline)
        } else {
            self.install_tracing_with_filter(config, pipeline)
        }
    }

    fn ot_layer<T>(tracer: T) -> OpenTelemetryLayer<Registry, T>
    where
        T: 'static + Tracer + PreSampledTracer + Send + Sync,
    {
        tracing_opentelemetry::layer()
            .with_tracked_inactivity(true)
            .with_tracer(tracer)
    }

    fn install_telemetry(&mut self, service_name: &str, config: &TelemetryConfig) -> Result<(), TelemetryBuildError> {
        let resource = Resource::new(vec![otconv::resource::SERVICE_NAME.string(service_name.to_string())]);

        // Install meter provider for opentelemetry
        if config.metrics {
            let prom_registry = prometheus::Registry::new();
            let exporter = opentelemetry_prometheus::exporter()
                .with_registry(prom_registry.clone())
                .build()
                .unwrap();
            let provider = MeterProvider::builder().with_reader(exporter).build();
            self.meter = Some(provider.meter(service_name.to_string()));
            self.prom_registry = Some(prom_registry);
        }

        // Install tracer provider for opentelemetry
        match &config.tracing {
            Tracing::StdOut => {
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
                self.install_tracing_layer(config, Self::ot_layer(tracer))?;
            }
            #[cfg(feature = "ot_jaeger")]
            Tracing::Jaeger => {
                let tracer = opentelemetry_jaeger::new_agent_pipeline()
                    .with_trace_config(otConfig().with_resource(resource))
                    .with_service_name(service_name.to_string())
                    .install_batch(Tokio)?;
                self.install_tracing_layer(config, Self::ot_layer(tracer))?;
            }
            #[cfg(feature = "ot_zipkin")]
            Tracing::Zipkin => {
                let tracer = opentelemetry_zipkin::new_pipeline()
                    .with_trace_config(otConfig().with_resource(resource))
                    .with_service_name(service_name.to_string())
                    .install_batch(Tokio)?;
                self.install_tracing_layer(config, Self::ot_layer(tracer))?;
            }
            #[cfg(feature = "ot_app_insight")]
            Tracing::AppInsight { instrumentation_key } => {
                let tracer = opentelemetry_application_insights::new_pipeline_from_connection_string(
                    instrumentation_key.clone(),
                )
                .map_err(TelemetryBuildError::AppInsightConfigError)?
                .with_trace_config(otConfig().with_resource(resource))
                .with_service_name(service_name.to_string())
                .with_client(reqwest::Client::new())
                .install_batch(Tokio);
                self.install_tracing_layer(config, Self::ot_layer(tracer))?;
            }
            Tracing::None => self.install_tracing_layer(config, EmptyLayer)?,
        };

        Ok(())
    }

    pub fn reconfigure(&self, filter: String) -> Result<(), TraceReconfigureError> {
        if let Some(reconfigure) = &self.reconfigure {
            reconfigure.reconfigure(filter).map_err(TraceReconfigureError)?
        }
        Ok(())
    }

    pub fn meter(&self) -> Option<&Meter> {
        self.meter.as_ref()
    }

    pub fn metrics(&self) -> String {
        if let Some(registry) = &self.prom_registry {
            let mut buffer = vec![];
            let encoder = TextEncoder::new();
            let metric_families = registry.gather();
            encoder.encode(&metric_families, &mut buffer).unwrap();
            String::from_utf8(buffer).unwrap()
        } else {
            String::new()
        }
    }

    pub fn to_layer(&self) -> OtelLayer {
        //todo: read route filtering from config
        OtelLayer::default()
    }
}

struct EmptyLayer;
impl<S: Subscriber> Layer<S> for EmptyLayer {}
