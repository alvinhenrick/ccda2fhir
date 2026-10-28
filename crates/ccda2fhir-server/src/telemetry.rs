//! Logging, and optional OpenTelemetry trace export.
//!
//! Logs always go to stderr. With the `otel` feature built in and `OTEL_EXPORTER_OTLP_ENDPOINT` (or
//! `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`) set, request spans are also exported over OTLP/HTTP. The
//! standard `OTEL_*` variables (`OTEL_SERVICE_NAME`, `OTEL_EXPORTER_OTLP_HEADERS`,
//! `OTEL_RESOURCE_ATTRIBUTES`, ...) are read by the SDK itself. Spans and events carry only the HTTP
//! method and path, the status word and counts. They never carry document content.

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Flushes and shuts the exporter down when dropped.
pub struct Guard {
    #[cfg(feature = "otel")]
    provider: Option<opentelemetry_sdk::trace::SdkTracerProvider>,
}

impl Drop for Guard {
    fn drop(&mut self) {
        #[cfg(feature = "otel")]
        if let Some(p) = self.provider.take() {
            let _ = p.shutdown();
        }
    }
}

fn filter() -> EnvFilter {
    EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))
}

fn configured() -> bool {
    [
        "OTEL_EXPORTER_OTLP_ENDPOINT",
        "OTEL_EXPORTER_OTLP_TRACES_ENDPOINT",
    ]
    .iter()
    .any(|k| std::env::var(k).is_ok_and(|v| !v.trim().is_empty()))
        && std::env::var("OTEL_SDK_DISABLED").map_or(true, |v| !v.eq_ignore_ascii_case("true"))
}

#[cfg(feature = "otel")]
pub fn init() -> Guard {
    use opentelemetry::trace::TracerProvider as _;

    let fmt = tracing_subscriber::fmt::layer();
    if configured() {
        match build_provider() {
            Ok(provider) => {
                let tracer = provider.tracer("ccda2fhir");
                tracing_subscriber::registry()
                    .with(filter())
                    .with(fmt)
                    .with(tracing_opentelemetry::layer().with_tracer(tracer))
                    .init();
                tracing::info!("exporting traces over OTLP");
                return Guard {
                    provider: Some(provider),
                };
            }
            Err(e) => {
                tracing_subscriber::registry()
                    .with(filter())
                    .with(fmt)
                    .init();
                tracing::warn!(error = %e, "cannot start the OTLP exporter; continuing without it");
                return Guard { provider: None };
            }
        }
    }
    tracing_subscriber::registry()
        .with(filter())
        .with(fmt)
        .init();
    Guard { provider: None }
}

#[cfg(feature = "otel")]
fn build_provider() -> Result<opentelemetry_sdk::trace::SdkTracerProvider, String> {
    use opentelemetry_sdk::{trace::SdkTracerProvider, Resource};

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_http()
        .build()
        .map_err(|e| e.to_string())?;
    let mut resource = Resource::builder();
    if std::env::var("OTEL_SERVICE_NAME").is_err() {
        resource = resource.with_service_name("ccda2fhir");
    }
    Ok(SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
        .with_resource(resource.build())
        .build())
}

#[cfg(not(feature = "otel"))]
pub fn init() -> Guard {
    tracing_subscriber::registry()
        .with(filter())
        .with(tracing_subscriber::fmt::layer())
        .init();
    if configured() {
        tracing::warn!(
            "OTEL_EXPORTER_OTLP_ENDPOINT is set but this build has no OpenTelemetry support; \
             rebuild with `--features otel`"
        );
    }
    Guard {}
}
