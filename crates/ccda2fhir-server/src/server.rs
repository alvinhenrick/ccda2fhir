//! The HTTP service: a thin layer over the pure converter library.
//!
//! Document content is never logged. Error responses carry the library's content-free messages
//! and nothing else from the request.

use std::time::Duration;

use axum::{
    body::Bytes,
    extract::Query,
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use ccda2fhir::{convert, Options};
use ccda_map::Profiles;
use serde::Deserialize;
use serde_json::{json, Value};
use tower::limit::ConcurrencyLimitLayer;
use tower_http::{limit::RequestBodyLimitLayer, timeout::TimeoutLayer, trace::TraceLayer};

/// Service settings.
#[derive(Debug, Clone)]
pub struct Config {
    /// Largest accepted request body, in bytes.
    pub max_body_bytes: usize,
    /// Conversions that may run at once; further requests wait.
    pub max_concurrency: usize,
    /// Time allowed for one request.
    pub request_timeout: Duration,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_body_bytes: 10 * 1024 * 1024,
            max_concurrency: 64,
            request_timeout: Duration::from_secs(30),
        }
    }
}

#[derive(Debug, Deserialize, Default)]
struct ConvertQuery {
    /// `none` declares no profiles; anything else (or nothing) is US Core 6.1.0.
    profiles: Option<String>,
    /// Return `{ "bundle": ..., "report": [...] }` instead of the bare Bundle.
    report: Option<bool>,
}

const FHIR_JSON: &str = "application/fhir+json";

/// Build the router.
pub fn router(config: &Config) -> Router {
    Router::new()
        .route("/convert", post(convert_handler))
        .route("/healthz", get(|| async { "ok" }))
        .route("/readyz", get(|| async { "ok" }))
        .layer(RequestBodyLimitLayer::new(config.max_body_bytes))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            config.request_timeout,
        ))
        .layer(ConcurrencyLimitLayer::new(config.max_concurrency))
        .layer(TraceLayer::new_for_http())
}

fn outcome(status: StatusCode, code: &str, diagnostics: &str) -> Response {
    let body = json!({
        "resourceType": "OperationOutcome",
        "issue": [{ "severity": "error", "code": code, "diagnostics": diagnostics }],
    });
    (
        status,
        [(header::CONTENT_TYPE, HeaderValue::from_static(FHIR_JSON))],
        Json(body),
    )
        .into_response()
}

fn is_xml(headers: &HeaderMap) -> bool {
    let Some(value) = headers.get(header::CONTENT_TYPE) else {
        return true;
    };
    let value = value.to_str().unwrap_or_default().to_ascii_lowercase();
    let media = value.split(';').next().unwrap_or_default().trim();
    media == "application/xml" || media == "text/xml" || media.ends_with("+xml")
}

async fn convert_handler(
    Query(query): Query<ConvertQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !is_xml(&headers) {
        return outcome(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "not-supported",
            "send the C-CDA document as application/xml",
        );
    }
    let options = Options {
        profiles: if query.profiles.as_deref() == Some("none") {
            Profiles::None
        } else {
            Profiles::UsCore610
        },
        ..Options::default()
    };
    let with_report = query.report.unwrap_or(false);

    // Conversion is CPU work; keep it off the async threads.
    let result = tokio::task::spawn_blocking(move || {
        convert(&body, &options).map(|c| {
            let issues = c.report.issues.len();
            let report: Vec<Value> = c
                .report
                .issues
                .iter()
                .map(|i| {
                    json!({
                        "severity": format!("{:?}", i.severity).to_lowercase(),
                        "code": i.code,
                        "message": i.message,
                    })
                })
                .collect();
            (c.bundle.to_json(), report, issues)
        })
    })
    .await;

    match result {
        Ok(Ok((bundle, report, issues))) => {
            let payload = if with_report {
                json!({ "bundle": bundle, "report": report })
            } else {
                bundle
            };
            (
                StatusCode::OK,
                [
                    (header::CONTENT_TYPE, HeaderValue::from_static(FHIR_JSON)),
                    (
                        header::HeaderName::from_static("x-ccda2fhir-issues"),
                        HeaderValue::from(issues),
                    ),
                ],
                Json(payload),
            )
                .into_response()
        }
        Ok(Err(e)) => outcome(StatusCode::BAD_REQUEST, "invalid", &e.to_string()),
        Err(_) => outcome(
            StatusCode::INTERNAL_SERVER_ERROR,
            "exception",
            "the conversion failed unexpectedly",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    const DOC: &str = r#"<ClinicalDocument xmlns="urn:hl7-org:v3"><id root="1.2.3" extension="X"/></ClinicalDocument>"#;

    async fn call(req: Request<Body>, config: &Config) -> (StatusCode, Value) {
        let response = router(config).oneshot(req).await.unwrap();
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (
            status,
            serde_json::from_slice(&bytes).unwrap_or(Value::Null),
        )
    }

    fn post_doc(content_type: Option<&str>, body: &str, uri: &str) -> Request<Body> {
        let mut b = Request::builder().method("POST").uri(uri);
        if let Some(ct) = content_type {
            b = b.header(header::CONTENT_TYPE, ct);
        }
        b.body(Body::from(body.to_owned())).unwrap()
    }

    #[tokio::test]
    async fn converts_a_document() {
        let (status, body) = call(
            post_doc(Some("application/xml"), DOC, "/convert"),
            &Config::default(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["resourceType"], "Bundle");
    }

    #[tokio::test]
    async fn report_is_returned_on_request() {
        let (status, body) = call(
            post_doc(None, DOC, "/convert?report=true&profiles=none"),
            &Config::default(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["bundle"]["resourceType"], "Bundle");
        assert!(body["report"].is_array());
    }

    #[tokio::test]
    async fn bad_xml_is_a_400_without_document_content() {
        let (status, body) = call(
            post_doc(Some("text/xml"), "<secret-name>", "/convert"),
            &Config::default(),
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(body["resourceType"], "OperationOutcome");
        assert!(!body.to_string().contains("secret-name"));
    }

    #[tokio::test]
    async fn wrong_content_type_is_a_415() {
        let (status, _) = call(
            post_doc(Some("application/json"), "{}", "/convert"),
            &Config::default(),
        )
        .await;
        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    }

    #[tokio::test]
    async fn oversized_body_is_a_413() {
        let config = Config {
            max_body_bytes: 16,
            ..Config::default()
        };
        let (status, _) = call(post_doc(Some("application/xml"), DOC, "/convert"), &config).await;
        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn health_endpoints_answer() {
        for uri in ["/healthz", "/readyz"] {
            let req = Request::builder().uri(uri).body(Body::empty()).unwrap();
            let response = router(&Config::default()).oneshot(req).await.unwrap();
            assert_eq!(response.status(), StatusCode::OK);
        }
    }
}
