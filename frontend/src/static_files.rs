use axum::{
    extract::Path,
    http::{header, StatusCode},
    response::{IntoResponse, Response},
    body::Body,
};
use crate::templates::StaticAssets;

pub async fn serve_static(Path(path): Path<String>) -> impl IntoResponse {
    // 从path中移除开头的斜杠（如果有的话），并添加static/前缀
    let clean_path = path.strip_prefix('/').unwrap_or(&path);
    let full_path = format!("static/{}", clean_path);
    
    match StaticAssets::get(&full_path) {
        Some(content) => {
            let mime = mime_guess::from_path(clean_path).first_or_octet_stream();
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