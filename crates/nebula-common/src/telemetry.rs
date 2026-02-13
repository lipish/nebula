use opentelemetry::trace::TracerProvider as TracerProviderTrait;
use opentelemetry::KeyValue;
use opentelemetry_otlp::{WithExportConfig, WithHttpConfig};
use opentelemetry_sdk::trace::TracerProvider;
use opentelemetry_sdk::Resource;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize tracing with optional OTLP export to xtrace.
///
/// - `service_name`: identifies this component (e.g. "nebula-gateway")
/// - `otlp_endpoint`: if `Some`, traces are exported via OTLP/HTTP to this base URL
///   (e.g. "http://10.21.11.92:8742/api/public/otel"). The exporter appends `/v1/traces`.
/// - `otlp_token`: bearer token for xtrace authentication
///
/// Returns an optional `SdkTracerProvider` that the caller should keep alive
/// and call `shutdown()` on before exit.
pub fn init_tracing(
    service_name: &str,
    otlp_endpoint: Option<&str>,
    otlp_token: Option<&str>,
) -> Option<TracerProvider> {
    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let fmt_layer = tracing_subscriber::fmt::layer();

    if let Some(endpoint) = otlp_endpoint {
        let mut headers = std::collections::HashMap::new();
        if let Some(token) = otlp_token {
            if !token.is_empty() {
                headers.insert("Authorization".to_string(), format!("Bearer {token}"));
            }
        }

        let exporter = match opentelemetry_otlp::SpanExporter::builder()
            .with_http()
            .with_endpoint(endpoint)
            .with_headers(headers)
            .build()
        {
            Ok(e) => e,
            Err(err) => {
                eprintln!("failed to create OTLP exporter: {err}, falling back to stdout only");
                tracing_subscriber::registry()
                    .with(env_filter)
                    .with(fmt_layer)
                    .init();
                return None;
            }
        };

        let provider = TracerProvider::builder()
            .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
            .with_resource(Resource::new([KeyValue::new("service.name", service_name.to_string())]))
            .build();

        let otel_layer = tracing_opentelemetry::layer()
            .with_tracer(provider.tracer(service_name.to_string()));

        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .with(otel_layer)
            .init();

        tracing::info!(endpoint, service_name, "OTLP tracing enabled");
        Some(provider)
    } else {
        tracing_subscriber::registry()
            .with(env_filter)
            .with(fmt_layer)
            .init();
        None
    }
}
