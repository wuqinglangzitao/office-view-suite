//! REST/HTTP API for ppt-rs (optional `http` feature).
//!
//! Exposes generation, validation, export, and diff over JSON + raw `.pptx` bodies.
//! Run via the `ppt_http` binary: `cargo run --features http --bin ppt_http`.

use axum::{
    Json, Router,
    body::Bytes,
    extract::Query,
    http::{StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use serde::{Deserialize, Serialize};

use crate::api::Presentation;
use crate::core::{PackageValidationReport, validate_package_bytes};
use crate::diff::{PresentationDiff, compare_presentations};
use crate::generator::{SlideContent, create_pptx_with_content};
use crate::import::import_pptx;

const PPTX_CONTENT_TYPE: &str =
    "application/vnd.openxmlformats-officedocument.presentationml.presentation";

#[derive(Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

#[derive(Debug, Deserialize, Serialize)]
struct GenerateRequest {
    title: String,
    slides: Vec<SlideRequest>,
}

#[derive(Debug, Deserialize, Serialize)]
struct SlideRequest {
    title: String,
    #[serde(default)]
    bullets: Vec<String>,
}

#[derive(Deserialize)]
struct MarkdownRequest {
    markdown: String,
    #[serde(default)]
    title: Option<String>,
}

#[derive(Deserialize)]
struct DiffRequest {
    /// Base64-encoded baseline `.pptx`.
    old_pptx: String,
    /// Base64-encoded updated `.pptx`.
    new_pptx: String,
}

#[derive(Deserialize, Default)]
struct ExportQuery {
    #[serde(default)]
    group_by_sections: bool,
}

#[derive(Serialize)]
struct ErrorBody {
    error: String,
}

#[derive(Serialize)]
struct ValidationResponse {
    valid: bool,
    error_count: usize,
    warning_count: usize,
    issues: Vec<ValidationIssueResponse>,
}

#[derive(Serialize)]
struct ValidationIssueResponse {
    severity: &'static str,
    category: String,
    path: Option<String>,
    message: String,
}

#[derive(Serialize)]
struct DiffResponse {
    equal: bool,
    summary: String,
    title_changed: Option<TitleChangeResponse>,
    slide_changes: Vec<SlideChangeResponse>,
}

#[derive(Serialize)]
struct TitleChangeResponse {
    before: String,
    after: String,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SlideChangeResponse {
    Added { index: usize, title: String },
    Removed { index: usize, title: String },
    Modified { index: usize, fields: String },
}

struct ApiError {
    status: StatusCode,
    message: String,
}

impl ApiError {
    fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }

    fn internal(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        (
            self.status,
            Json(ErrorBody {
                error: self.message,
            }),
        )
            .into_response()
    }
}

/// Build the HTTP router.
pub fn router() -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/v1/generate", post(generate))
        .route("/v1/markdown", post(markdown_to_pptx))
        .route("/v1/validate", post(validate))
        .route("/v1/export/markdown", post(export_markdown))
        .route("/v1/export/html", post(export_html))
        .route("/v1/diff", post(diff))
}

/// Bind and serve the REST API.
pub async fn run_server(addr: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    eprintln!("ppt-rs HTTP listening on http://{addr}");
    axum::serve(listener, router()).await?;
    Ok(())
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: crate::VERSION,
    })
}

async fn generate(Json(req): Json<GenerateRequest>) -> Result<Response, ApiError> {
    let bytes = build_pptx(&req.title, &req.slides).map_err(ApiError::internal)?;
    Ok(pptx_response(bytes))
}

async fn markdown_to_pptx(Json(req): Json<MarkdownRequest>) -> Result<Response, ApiError> {
    let slides = crate::cli::parse_markdown(&req.markdown).map_err(ApiError::bad_request)?;
    if slides.is_empty() {
        return Err(ApiError::bad_request("Markdown produced no slides"));
    }
    let title = req
        .title
        .unwrap_or_else(|| "Presentation from Markdown".to_string());
    let bytes =
        create_pptx_with_content(&title, slides).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(pptx_response(bytes))
}

async fn validate(body: Bytes) -> Result<Json<ValidationResponse>, ApiError> {
    if body.is_empty() {
        return Err(ApiError::bad_request("Request body must contain .pptx bytes"));
    }
    let report = validate_package_bytes(&body);
    Ok(Json(validation_json(&report)))
}

async fn export_markdown(
    Query(query): Query<ExportQuery>,
    body: Bytes,
) -> Result<Response, ApiError> {
    let pres = import_pptx_bytes(&body)?;
    let md = if query.group_by_sections {
        crate::export::md::export_to_markdown_with_options(
            &pres,
            &crate::export::md::MarkdownOptions::new().with_section_grouping(true),
        )
    } else {
        crate::export::md::export_to_markdown(&pres)
    }
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok((
        [(header::CONTENT_TYPE, "text/markdown; charset=utf-8")],
        md,
    )
        .into_response())
}

async fn export_html(Query(query): Query<ExportQuery>, body: Bytes) -> Result<Response, ApiError> {
    let pres = import_pptx_bytes(&body)?;
    let html = if query.group_by_sections {
        crate::export::html::export_to_html_with_options(
            &pres,
            &crate::export::html::HtmlExportOptions::new().with_section_grouping(true),
        )
    } else {
        crate::export::html::export_to_html(&pres)
    }
    .map_err(|e| ApiError::internal(e.to_string()))?;
    Ok((
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        html,
    )
        .into_response())
}

async fn diff(Json(req): Json<DiffRequest>) -> Result<Json<DiffResponse>, ApiError> {
    use base64::Engine;
    let old = base64::engine::general_purpose::STANDARD
        .decode(req.old_pptx.trim())
        .map_err(|e| ApiError::bad_request(format!("Invalid old_pptx base64: {e}")))?;
    let new = base64::engine::general_purpose::STANDARD
        .decode(req.new_pptx.trim())
        .map_err(|e| ApiError::bad_request(format!("Invalid new_pptx base64: {e}")))?;
    let a = import_pptx_bytes(&old)?;
    let b = import_pptx_bytes(&new)?;
    Ok(Json(diff_json(&compare_presentations(&a, &b))))
}

fn build_pptx(title: &str, slides: &[SlideRequest]) -> Result<Vec<u8>, String> {
    if slides.is_empty() {
        return Err("At least one slide is required".to_string());
    }
    let slides: Vec<SlideContent> = slides
        .iter()
        .map(|slide| {
            let mut content = SlideContent::new(&slide.title);
            for bullet in &slide.bullets {
                content = content.add_bullet(bullet);
            }
            content
        })
        .collect();
    create_pptx_with_content(title, slides).map_err(|e| e.to_string())
}

fn pptx_response(bytes: Vec<u8>) -> Response {
    (
        [(header::CONTENT_TYPE, PPTX_CONTENT_TYPE)],
        bytes,
    )
        .into_response()
}

fn import_pptx_bytes(bytes: &[u8]) -> Result<Presentation, ApiError> {
    if bytes.is_empty() {
        return Err(ApiError::bad_request("Request body must contain .pptx bytes"));
    }
    let path = write_temp_pptx(bytes)?;
    let pres = import_pptx(path.to_str().unwrap())
        .map_err(|e| ApiError::bad_request(e.to_string()))?;
    let _ = std::fs::remove_file(&path);
    Ok(pres)
}

fn write_temp_pptx(bytes: &[u8]) -> Result<std::path::PathBuf, ApiError> {
    let path = std::env::temp_dir().join(format!("ppt_rs_http_{}.pptx", uuid::Uuid::new_v4()));
    std::fs::write(&path, bytes).map_err(|e| ApiError::internal(e.to_string()))?;
    Ok(path)
}

fn validation_json(report: &PackageValidationReport) -> ValidationResponse {
    ValidationResponse {
        valid: report.is_valid(),
        error_count: report.error_count(),
        warning_count: report.warning_count(),
        issues: report
            .issues
            .iter()
            .map(|issue| ValidationIssueResponse {
                severity: match issue.severity {
                    crate::core::ValidationSeverity::Error => "error",
                    crate::core::ValidationSeverity::Warning => "warning",
                },
                category: format!("{:?}", issue.category),
                path: issue.path.clone(),
                message: issue.message.clone(),
            })
            .collect(),
    }
}

fn diff_json(diff: &PresentationDiff) -> DiffResponse {
    DiffResponse {
        equal: diff.is_equal(),
        summary: diff.summary(),
        title_changed: diff.title_changed.as_ref().map(|(before, after)| TitleChangeResponse {
            before: before.clone(),
            after: after.clone(),
        }),
        slide_changes: diff
            .slide_changes
            .iter()
            .map(|change| match change {
                crate::SlideChange::Added { index, slide } => SlideChangeResponse::Added {
                    index: *index,
                    title: slide.title.clone(),
                },
                crate::SlideChange::Removed { index, slide } => SlideChangeResponse::Removed {
                    index: *index,
                    title: slide.title.clone(),
                },
                crate::SlideChange::Modified { index, fields, .. } => {
                    SlideChangeResponse::Modified {
                        index: *index,
                        fields: format!("{fields:?}"),
                    }
                }
            })
            .collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[tokio::test]
    async fn health_endpoint_returns_ok() {
        let response = router()
            .oneshot(
                axum::http::Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert!(std::str::from_utf8(&body).unwrap().contains("ok"));
    }

    #[tokio::test]
    async fn generate_endpoint_returns_pptx_bytes() {
        let payload = GenerateRequest {
            title: "HTTP Deck".to_string(),
            slides: vec![SlideRequest {
                title: "Intro".to_string(),
                bullets: vec!["One".to_string()],
            }],
        };
        let response = router()
            .oneshot(
                axum::http::Request::builder()
                    .method("POST")
                    .uri("/v1/generate")
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(Body::from(serde_json::to_vec(&payload).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert!(body.starts_with(b"PK"));
    }

    #[test]
    fn build_pptx_requires_slides() {
        assert!(build_pptx("Empty", &[]).is_err());
    }
}
