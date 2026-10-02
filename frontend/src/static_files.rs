use crate::templates::StaticAssets;
use axum::{
    body::Body,
    extract::Path,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};

pub async fn serve_static(Path(path): Path<String>) -> impl IntoResponse {
    let clean_path = path.strip_prefix('/').unwrap_or(&path);
    asset_response(&format!("static/{}", clean_path))
}

pub async fn favicon() -> Response {
    asset_response("favicon.ico")
}

pub async fn robots() -> Response {
    asset_response("robots.txt")
}

fn asset_response(path: &str) -> Response {
    match StaticAssets::get(path) {
        Some(content) => {
            let mime = mime_guess::from_path(path).first_or_octet_stream();
            Response::builder()
                .status(StatusCode::OK)
                .header(header::CONTENT_TYPE, mime.as_ref())
                .body(Body::from(content.data))
                .expect("Failed to build response")
        }
        None => Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::from("File not found"))
            .expect("Failed to build response"),
    }
}
