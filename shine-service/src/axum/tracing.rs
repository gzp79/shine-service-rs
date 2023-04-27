use axum::{extract::State, routing::put, Json, Router};
use opentelemetry::{
    sdk::{trace as otsdk, Resource},
    trace::{TraceError, Tracer},
};
use opentelemetry_semantic_conventions::resource as otconv;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error as ThisError;
use tracing::{instrument::WithSubscriber, log, subscriber::SetGlobalDefaultError, Dispatch, Level, Subscriber};
use tracing_opentelemetry::PreSampledTracer;
use tracing_subscriber::{
    filter::EnvFilter,
    layer::SubscriberExt,
    registry::LookupSpan,
    reload::{self, Handle},
    Layer,
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

    /// Dump trace to the standard output
    StdOut,

    /// Enable Jaeger telemetry (https://www.jaegertracing.io)
    #[cfg(feature = "ot_jaeger")]
    Jaeger,

    /// Enable Zipkin telemetry (https://zipkin.io/)
    #[cfg(feature = "ot_zipkin")]
    Zipkin,

    /// AppInsight telemetry
    #[cfg(feature = "ot_app_insight")]
    AppInsight { instrumentation_key: String },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TracingConfig {
    allow_reconfigure: bool,
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

async fn reconfigure(State(data): State<Arc<Data>>, Json(format): Json<TraceConfigRequest>) -> Result<(), String> {
    log::trace!("config: {:#?}", format);
    if let Some(reload_handle) = &data.reload_handle {
        reload_handle.reconfigure(format.filter)
    } else {
        Err("Trace reconfigure is not enabled".into())
    }
}

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
        service.install_logger(service_name, config, tracing_subscriber::registry())?;
        Ok(service)
    }

    fn set_global_logger<L>(&mut self, tracing_pipeline: L) -> Result<(), TracingError>
    where
        L: Into<Dispatch>,
    {
        tracing::dispatcher::set_global_default(tracing_pipeline.into())?;
        Ok(())
    }

    fn install_telemetry_with_tracer<L, T>(
        &mut self,
        _config: &TracingConfig,
        tracing_pipeline: L,
        tracer: T,
    ) -> Result<(), TracingError>
    where
        L: for<'a> LookupSpan<'a> + Subscriber + WithSubscriber + Send + Sync,
        T: 'static + Tracer + PreSampledTracer + Send + Sync,
    {
        let telemetry = tracing_opentelemetry::layer()
            .with_tracked_inactivity(true)
            .with_tracer(tracer);
        let tracing_pipeline = tracing_pipeline.with(telemetry);
        self.set_global_logger(tracing_pipeline)?;
        Ok(())
    }

    fn install_telemetry<L>(
        &mut self,
        service_name: &str,
        config: &TracingConfig,
        tracing_pipeline: L,
    ) -> Result<(), TracingError>
    where
        L: for<'a> LookupSpan<'a> + Subscriber + WithSubscriber + Send + Sync,
    {
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
                self.install_telemetry_with_tracer(config, tracing_pipeline, tracer)
            }
            #[cfg(feature = "ot_jaeger")]
            Telemetry::Jaeger => {
                let tracer = opentelemetry_jaeger::new_agent_pipeline()
                    .with_trace_config(otsdk::config().with_resource(resource))
                    .with_service_name(service_name.to_string())
                    .install_batch(opentelemetry::runtime::Tokio)?;
                self.install_telemetry_with_tracer(config, tracing_pipeline, tracer)
            }
            #[cfg(feature = "ot_zipkin")]
            Telemetry::Zipkin => {
                let tracer = opentelemetry_zipkin::new_pipeline()
                    .with_trace_config(otsdk::config().with_resource(resource))
                    .with_service_name(service_name.to_string())
                    .install_batch(opentelemetry::runtime::Tokio)?;
                self.install_telemetry_with_tracer(config, tracing_pipeline, tracer)
            }
            #[cfg(feature = "ot_app_insight")]
            Telemetry::AppInsight { instrumentation_key } => {
                let tracer = opentelemetry_application_insights::new_pipeline(instrumentation_key.clone())
                    .with_trace_config(otsdk::config().with_resource(resource))
                    .with_service_name(service_name.to_string())
                    .with_client(reqwest::Client::new())
                    .install_batch(opentelemetry::runtime::Tokio);
                self.install_telemetry_with_tracer(config, tracing_pipeline, tracer)
            }
            Telemetry::None => self.set_global_logger(tracing_pipeline),
        }
    }

    fn install_logger<L>(
        &mut self,
        service_name: &str,
        config: &TracingConfig,
        tracing_pipeline: L,
    ) -> Result<(), TracingError>
    where
        L: for<'a> LookupSpan<'a> + Subscriber + WithSubscriber + Send + Sync,
    {
        if config.allow_reconfigure {
            let env_filter = EnvFilter::from_default_env().add_directive(Level::INFO.into());
            let (env_filter, reload_handle) = reload::Layer::new(env_filter);
            self.reload_handle = Some(Box::new(reload_handle));
            let tracing_pipeline = tracing_pipeline.with(env_filter);

            let fmt = tracing_subscriber::fmt::Layer::new();
            let tracing_pipeline = tracing_pipeline.with(fmt);

            self.install_telemetry(service_name, config, tracing_pipeline)?;
        } else {
            let env_filter = EnvFilter::from_default_env().add_directive(Level::INFO.into());
            let tracing_pipeline = tracing_pipeline.with(env_filter);

            let fmt = tracing_subscriber::fmt::Layer::new();
            let tracing_pipeline = tracing_pipeline.with(fmt);

            self.install_telemetry(service_name, config, tracing_pipeline)?;
        }

        Ok(())
    }

    pub fn into_router<S>(self) -> Router<S>
    where
        S: Clone + Send + Sync + 'static,
    {
        let mut router = Router::new();
        // todo: consider adding it conditionally 'if self.reload_handle.is_some()'
        router = router.route("/config", put(reconfigure));

        router.with_state(Arc::new(Data {
            reload_handle: self.reload_handle,
        }))
    }
}
