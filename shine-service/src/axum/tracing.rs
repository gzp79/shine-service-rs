use axum::{
    extract::State,
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::put,
    Json, Router,
};
use opentelemetry::{
    sdk::{trace as otsdk, Resource},
    trace::{TraceError, Tracer},
};
use opentelemetry_semantic_conventions::resource as otconv;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error as ThisError;
use tracing::{log, subscriber::SetGlobalDefaultError, Dispatch, Level, Subscriber};
use tracing_opentelemetry::{OpenTelemetryLayer, PreSampledTracer};
use tracing_subscriber::{
    filter::EnvFilter,
    layer::SubscriberExt,
    registry::LookupSpan,
    reload::{self, Handle},
    Layer, Registry,
};

pub use axum_tracing_opentelemetry::opentelemetry_tracing_layer as tracing_layer;

#[derive(Debug, ThisError)]
pub enum TracingError {
    #[error(transparent)]
    SetGlobalTracing(#[from] SetGlobalDefaultError),
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

#[derive(Debug, Serialize, Deserialize)]
pub struct TraceConfigRequest {
    filter: String,
}

async fn reconfigure(State(data): State<Arc<Data>>, Json(format): Json<TraceConfigRequest>) -> Response {
    log::trace!("config: {:#?}", format);
    if let Some(reload_handle) = &data.reload_handle {
        match reload_handle.reconfigure(format.filter) {
            Err(err) => (StatusCode::BAD_REQUEST, err).into_response(),
            Ok(_) => StatusCode::OK.into_response(),
        }
    } else {
        (StatusCode::BAD_REQUEST, "Trace configure is disabled").into_response()
    }
}

struct EmptyLayer;

impl<S: Subscriber> Layer<S> for EmptyLayer {}

struct Data {
    reload_handle: Option<Box<dyn DynHandle>>,
}

pub struct TracingService {
    reload_handle: Option<Box<dyn DynHandle>>,
}

impl TracingService {
    /// Create a Service and initialize the global tracing logger
    pub async fn new(service_name: &str, config: &TracingConfig) -> Result<TracingService, TracingError> {
        let mut service = TracingService { reload_handle: None };
        service.install_telemetry(service_name, config)?;
        Ok(service)
    }

    fn set_global_logger<L>(&mut self, tracing_pipeline: L) -> Result<(), TracingError>
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

    fn install_filter<T>(&mut self, config: &TracingConfig, pipeline: T) -> Result<(), TracingError>
    where
        T: for<'a> LookupSpan<'a> + Subscriber + Send + Sync,
    {
        if config.allow_reconfigure {
            // enable filtering with reconfiguration capabilities
            let env_filter = EnvFilter::from_default_env().add_directive(Level::INFO.into());
            let (reload_env_filter, reload_handle) = reload::Layer::new(env_filter);
            let pipeline = pipeline.with(reload_env_filter);
            self.reload_handle = Some(Box::new(reload_handle));

            self.set_global_logger(pipeline)?;
            Ok(())
        } else {
            // enable filtering from the environment variables
            let env_filter = EnvFilter::from_default_env().add_directive(Level::INFO.into());
            let pipeline = pipeline.with(env_filter);

            self.set_global_logger(pipeline)?;
            Ok(())
        }
    }

    fn install_logger<T>(&mut self, config: &TracingConfig, pipeline: T) -> Result<(), TracingError>
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

    fn install_pipeline<L>(&mut self, config: &TracingConfig, layer: L) -> Result<(), TracingError>
    where
        L: Layer<Registry> + Send + Sync,
    {
        let pipeline = tracing_subscriber::registry().with(layer);
        self.install_logger(config, pipeline)
    }

    fn install_telemetry(&mut self, service_name: &str, config: &TracingConfig) -> Result<(), TracingError> {
        let resource = Resource::new(vec![otconv::SERVICE_NAME.string(service_name.to_string())]);

        match &config.telemetry {
            Telemetry::StdOut => {
                let tracer = opentelemetry::sdk::export::trace::stdout::PipelineBuilder::default()
                    .with_trace_config(
                        otsdk::config()
                            .with_resource(resource)
                            .with_sampler(otsdk::Sampler::AlwaysOn),
                    )
                    .install_simple();
                self.install_pipeline(config, Self::ot_layer(tracer))
            }
            #[cfg(feature = "ot_jaeger")]
            Telemetry::Jaeger => {
                let tracer = opentelemetry_jaeger::new_agent_pipeline()
                    .with_trace_config(otsdk::config().with_resource(resource))
                    .with_service_name(service_name.to_string())
                    .install_batch(opentelemetry::runtime::Tokio)?;
                self.install_pipeline(config, Self::ot_layer(tracer))
            }
            #[cfg(feature = "ot_zipkin")]
            Telemetry::Zipkin => {
                let tracer = opentelemetry_zipkin::new_pipeline()
                    .with_trace_config(otsdk::config().with_resource(resource))
                    .with_service_name(service_name.to_string())
                    .install_batch(opentelemetry::runtime::Tokio)?;
                self.install_pipeline(config, Self::ot_layer(tracer))
            }
            #[cfg(feature = "ot_app_insight")]
            Telemetry::AppInsight { instrumentation_key } => {
                let tracer = opentelemetry_application_insights::new_pipeline(instrumentation_key.clone())
                    .with_trace_config(otsdk::config().with_resource(resource))
                    .with_service_name(service_name.to_string())
                    .with_client(reqwest::Client::new())
                    .install_batch(opentelemetry::runtime::Tokio);
                self.install_pipeline(config, Self::ot_layer(tracer))
            }
            Telemetry::None => self.install_pipeline(config, EmptyLayer),
        }
    }

    pub fn into_router<S>(self) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        let mut router = Router::new();
        router = router.route("/config", put(reconfigure));

        router.with_state(Arc::new(Data {
            reload_handle: self.reload_handle,
        }))
    }
}
