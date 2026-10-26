//! The HTTP service: a thin layer over the pure converter library.
//!
//! Document content is never logged. Error responses carry the library's content-free messages
//! and nothing else from the request.

use std::sync::Arc;
use std::time::Duration;

use axum::{
    body::Bytes,
    extract::{Query, State},
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
    /// LOINC classes for result categories (see `ccda2fhir --loinc-table`).
    pub loinc: Option<Arc<ccda_terminology::LoincTable>>,
    /// When set, `/convert` needs a valid bearer token. Health endpoints stay open.
    pub auth: Option<Arc<crate::auth::Authenticator>>,
    /// Serve the built-in web page at `/`.
    pub ui: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            max_body_bytes: 10 * 1024 * 1024,
            max_concurrency: 64,
            request_timeout: Duration::from_secs(30),
            loinc: None,
            auth: None,
            ui: true,
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

/// A built-in page asset. The policy lets the page load only its own script and style and talk
/// only to this server; it cannot be framed or send a referrer.
fn asset(content_type: &'static str, body: &'static str) -> Response {
    (
        [
            (header::CONTENT_TYPE, content_type),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; \
                 img-src 'self' data:; base-uri 'none'; form-action 'none'; frame-ancestors 'none'",
            ),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
            (header::REFERRER_POLICY, "no-referrer"),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        body,
    )
        .into_response()
}

/// Build the router.
pub fn router(config: &Config) -> Router {
    let mut convert = post(convert_handler);
    if let Some(auth) = &config.auth {
        convert = convert.route_layer(axum::middleware::from_fn_with_state(
            Arc::clone(auth),
            crate::auth::require,
        ));
    }
    let mut app = Router::new().route("/convert", convert);
    if config.ui {
        app = app
            .route(
                "/",
                get(|| async { asset("text/html; charset=utf-8", include_str!("ui/index.html")) }),
            )
            .route(
                "/ui/app.js",
                get(|| async {
                    asset("text/javascript; charset=utf-8", include_str!("ui/app.js"))
                }),
            )
            .route(
                "/ui/app.css",
                get(|| async { asset("text/css; charset=utf-8", include_str!("ui/app.css")) }),
            );
    }
    app.route("/healthz", get(|| async { "ok" }))
        .route("/readyz", get(|| async { "ok" }))
        .with_state(config.loinc.clone())
        .layer(RequestBodyLimitLayer::new(config.max_body_bytes))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            config.request_timeout,
        ))
        .layer(ConcurrencyLimitLayer::new(config.max_concurrency))
        .layer(TraceLayer::new_for_http())
}

pub(crate) fn outcome(status: StatusCode, code: &str, diagnostics: &str) -> Response {
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

/// The media type of the request, lower case, without parameters. Empty when absent.
fn media_type(headers: &HeaderMap) -> String {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .map(|m| m.trim().to_ascii_lowercase())
        .unwrap_or_default()
}

fn is_xml_type(media: &str) -> bool {
    media.is_empty() || media == "application/xml" || media == "text/xml" || media.ends_with("+xml")
}

fn is_json_type(media: &str) -> bool {
    media == "application/json" || media == "application/fhir+json"
}

/// Standard base64 (RFC 4648); whitespace is ignored, anything else invalid is an error.
fn base64_decode(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() / 4 * 3);
    let mut acc = 0u32;
    let mut bits = 0u32;
    let mut padding = 0usize;
    for c in input.chars().filter(|c| !c.is_whitespace()) {
        let v = match c {
            'A'..='Z' => c as u32 - 'A' as u32,
            'a'..='z' => c as u32 - 'a' as u32 + 26,
            '0'..='9' => c as u32 - '0' as u32 + 52,
            '+' => 62,
            '/' => 63,
            '=' => {
                padding += 1;
                continue;
            }
            _ => return None,
        };
        if padding > 0 {
            return None;
        }
        acc = (acc << 6) | v;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((acc >> bits) & 0xff).ok()?);
        }
    }
    (padding <= 2).then_some(out)
}

/// The C-CDA XML carried in a JSON request: `{ "document": "<base64>" }`, or a FHIR
/// `DocumentReference` whose attachment holds the XML as base64 `data`.
fn document_from_json(body: &[u8]) -> Result<Vec<u8>, &'static str> {
    let value: Value =
        serde_json::from_slice(body).map_err(|_| "the request body is not valid JSON")?;
    let data = if value["resourceType"] == "DocumentReference" {
        value["content"]
            .as_array()
            .into_iter()
            .flatten()
            .map(|c| &c["attachment"])
            .find(|a| {
                a["data"].is_string()
                    && a["contentType"].as_str().is_some_and(|t| {
                        is_xml_type(
                            &t.split(';')
                                .next()
                                .unwrap_or("")
                                .trim()
                                .to_ascii_lowercase(),
                        )
                    })
            })
            .and_then(|a| a["data"].as_str())
            .ok_or("the DocumentReference has no XML attachment with inline data")?
    } else {
        value["document"]
            .as_str()
            .ok_or("send { \"document\": \"<base64 C-CDA>\" } or a DocumentReference")?
    };
    base64_decode(data).ok_or("the document data is not valid base64")
}

async fn convert_handler(
    State(loinc): State<Option<Arc<ccda_terminology::LoincTable>>>,
    Query(query): Query<ConvertQuery>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let media = media_type(&headers);
    let body = if is_xml_type(&media) {
        body
    } else if is_json_type(&media) {
        match document_from_json(&body) {
            Ok(xml) => Bytes::from(xml),
            Err(message) => return outcome(StatusCode::BAD_REQUEST, "invalid", message),
        }
    } else {
        return outcome(
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "not-supported",
            "send the C-CDA document as application/xml, or as JSON (base64 or a DocumentReference)",
        );
    };
    let options = Options {
        profiles: if query.profiles.as_deref() == Some("none") {
            Profiles::None
        } else {
            Profiles::UsCore610
        },
        loinc,
        ..Options::default()
    };
    let with_report = query.report.unwrap_or(false);

    // Conversion is CPU work; keep it off the async threads.
    let result = tokio::task::spawn_blocking(move || {
        convert(&body, &options).map(|c| {
            let issues = c.report.issues.len();
            let warnings = c
                .report
                .issues
                .iter()
                .filter(|i| i.severity == ccda_map::Severity::Warning)
                .count();
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
            (c.bundle.to_json(), report, issues, warnings)
        })
    })
    .await;

    match result {
        Ok(Ok((bundle, report, issues, warnings))) => {
            // `complete`: nothing was dropped, defaulted or approximated. `partial`: at least one
            // warning says something was (see the report for which).
            let status = if warnings == 0 { "complete" } else { "partial" };
            let payload = if with_report {
                json!({
                    "status": status,
                    "issues": { "total": issues, "warnings": warnings },
                    "bundle": bundle,
                    "report": report,
                })
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
                    (
                        header::HeaderName::from_static("x-ccda2fhir-status"),
                        HeaderValue::from_static(status),
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

    fn b64(input: &str) -> String {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut out = String::new();
        for chunk in input.as_bytes().chunks(3) {
            let b = [
                chunk[0],
                *chunk.get(1).unwrap_or(&0),
                *chunk.get(2).unwrap_or(&0),
            ];
            let n = (u32::from(b[0]) << 16) | (u32::from(b[1]) << 8) | u32::from(b[2]);
            for i in 0..4 {
                if i <= chunk.len() {
                    out.push(char::from(T[((n >> (18 - 6 * i)) & 63) as usize]));
                } else {
                    out.push('=');
                }
            }
        }
        out
    }

    #[tokio::test]
    async fn base64_json_and_document_reference_are_accepted() {
        let wrapped = json!({ "document": b64(DOC) }).to_string();
        let (status, body) = call(
            post_doc(Some("application/json"), &wrapped, "/convert"),
            &Config::default(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["resourceType"], "Bundle");

        let docref = json!({
            "resourceType": "DocumentReference",
            "content": [
                { "attachment": { "contentType": "application/pdf", "data": "AAAA" } },
                { "attachment": { "contentType": "application/xml", "data": b64(DOC) } }
            ]
        })
        .to_string();
        let (status, body) = call(
            post_doc(Some("application/fhir+json"), &docref, "/convert"),
            &Config::default(),
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["resourceType"], "Bundle");
    }

    #[tokio::test]
    async fn bad_json_input_is_a_400_without_content() {
        for body in [
            r#"{"document":"not base64 !!"}"#,
            r#"{"nothing":1}"#,
            r#"{"resourceType":"DocumentReference","content":[]}"#,
            "{secret",
        ] {
            let (status, out) = call(
                post_doc(Some("application/json"), body, "/convert"),
                &Config::default(),
            )
            .await;
            assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
            assert!(!out.to_string().contains("secret"));
        }
    }

    #[tokio::test]
    async fn status_says_complete_or_partial() {
        let (_, body) = call(
            post_doc(None, DOC, "/convert?report=true"),
            &Config::default(),
        )
        .await;
        assert!(["complete", "partial"].contains(&body["status"].as_str().unwrap()));
        assert!(body["issues"]["total"].is_number());
    }

    #[tokio::test]
    async fn wrong_content_type_is_a_415() {
        let (status, _) = call(
            post_doc(Some("text/plain"), "{}", "/convert"),
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

    #[tokio::test]
    async fn the_web_page_is_served_with_a_strict_policy_and_can_be_turned_off() {
        let get_req = |uri: &str| Request::builder().uri(uri).body(Body::empty()).unwrap();
        for (uri, kind) in [
            ("/", "text/html"),
            ("/ui/app.js", "text/javascript"),
            ("/ui/app.css", "text/css"),
        ] {
            let response = router(&Config::default())
                .oneshot(get_req(uri))
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK, "{uri}");
            let headers = response.headers();
            assert!(headers[header::CONTENT_TYPE]
                .to_str()
                .unwrap()
                .starts_with(kind));
            let csp = headers[header::CONTENT_SECURITY_POLICY].to_str().unwrap();
            assert!(csp.contains("default-src 'none'") && !csp.contains("unsafe-inline"));
            assert_eq!(headers[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        }
        let off = Config {
            ui: false,
            ..Config::default()
        };
        let response = router(&off).oneshot(get_req("/")).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}
