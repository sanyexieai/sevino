use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::{get, post, put, delete},
    response::Json,
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::collections::HashMap;
use std::time::Instant;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod models;
mod services;
mod utils;
mod config;

use crate::config::Settings;
use crate::services::{StorageService, BucketService, ObjectService};
use crate::models::{Bucket, Object, ObjectMetadata};

#[derive(OpenApi)]
#[openapi(
    paths(
        root,
        health_check,
        list_buckets,
        create_bucket,
        get_bucket,
        delete_bucket,
        list_objects,
        put_object,
        get_object,
        delete_object,
        get_object_metadata
    ),
    components(
        schemas(Bucket, Object, ObjectMetadata, ApiResponse<Bucket>, ApiResponse<Vec<Bucket>>, ApiResponse<Object>, ApiResponse<Vec<Object>>, ApiResponse<ObjectMetadata>, ApiResponse<()>, HealthResponse, CreateBucketRequest, BucketListResponse, ObjectListResponse)
    ),
    tags(
        (name = "buckets", description = "Bucket management endpoints"),
        (name = "objects", description = "Object management endpoints"),
        (name = "health", description = "Health check endpoints")
    )
)]
struct ApiDoc;

#[derive(Clone)]
struct AppState {
    bucket_service: BucketService,
    object_service: ObjectService,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();

    let settings = Settings::from_env();
    println!("Starting Sevino Object Storage Service with settings: {:?}", settings);

    let storage_service = match StorageService::new(settings.data_dir.clone()).await {
        Ok(service) => service,
        Err(e) => {
            eprintln!("Failed to initialize storage service: {}", e);
            std::process::exit(1);
        }
    };

    let bucket_service = BucketService::new(storage_service.clone());
    let object_service = ObjectService::new(storage_service);

    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health_check))
        .route("/api/buckets", get(list_buckets))
        .route("/api/buckets", post(create_bucket))
        .route("/api/buckets/:name", get(get_bucket))
        .route("/api/buckets/:name", delete(delete_bucket))
        .route("/api/buckets/:bucket_name/objects", get(list_objects))
        .route("/api/buckets/:bucket_name/objects/:key", put(put_object))
        .route("/api/buckets/:bucket_name/objects/:key", get(get_object))
        .route("/api/buckets/:bucket_name/objects/:key", delete(delete_object))
        .route("/api/buckets/:bucket_name/objects/:key/metadata", get(get_object_metadata))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .with_state(Arc::new(AppState {
            bucket_service,
            object_service,
        }));

    let addr = format!("{}:{}", settings.host, settings.port);
    println!("Server running on http://{}", addr);
    println!("Swagger UI available at http://{}/swagger-ui/", addr);

    let listener = tokio::net::TcpListener::bind(&addr).await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

#[utoipa::path(
    get,
    path = "/",
    tag = "health",
    responses(
        (status = 200, description = "Welcome message")
    )
)]
async fn root() -> &'static str {
    "Welcome to Sevino Object Storage Service!"
}

#[derive(Serialize, utoipa::ToSchema)]
struct HealthResponse {
    status: String,
    timestamp: String,
}

#[utoipa::path(
    get,
    path = "/health",
    tag = "health",
    responses(
        (status = 200, description = "Health check response", body = HealthResponse)
    )
)]
async fn health_check() -> Json<HealthResponse> {
    let response = HealthResponse {
        status: "healthy".to_string(),
        timestamp: chrono::Utc::now().to_rfc3339(),
    };
    Json(response)
}

#[utoipa::path(
    get,
    path = "/api/buckets",
    tag = "buckets",
    responses(
        (status = 200, description = "List of buckets", body = ApiResponse<BucketListResponse>)
    )
)]
async fn list_buckets(
    State(state): State<Arc<AppState>>,
) -> Json<ApiResponse<BucketListResponse>> {
    let buckets = state.bucket_service.list_buckets().await;
    let response = BucketListResponse { buckets };
    Json(ApiResponse::success(response))
}

#[derive(Deserialize, utoipa::ToSchema)]
struct CreateBucketRequest {
    name: String,
}

#[utoipa::path(
    post,
    path = "/api/buckets",
    tag = "buckets",
    request_body(content = CreateBucketRequest, content_type = "application/json"),
    responses(
        (status = 200, description = "Bucket created successfully", body = ApiResponse<Bucket>),
        (status = 400, description = "Invalid bucket name", body = ApiResponse<Bucket>)
    )
)]
async fn create_bucket(
    State(state): State<Arc<AppState>>,
    Json(request): Json<CreateBucketRequest>,
) -> Json<ApiResponse<Bucket>> {
    match state.bucket_service.create_bucket(request.name).await {
        Ok(bucket) => Json(ApiResponse::success(bucket)),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

#[utoipa::path(
    get,
    path = "/api/buckets/{name}",
    tag = "buckets",
    params(
        ("name" = String, Path, description = "Bucket name")
    ),
    responses(
        (status = 200, description = "Bucket details", body = ApiResponse<Bucket>),
        (status = 404, description = "Bucket not found", body = ApiResponse<Bucket>)
    )
)]
async fn get_bucket(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Json<ApiResponse<Bucket>> {
    match state.bucket_service.get_bucket(&name).await {
        Some(bucket) => Json(ApiResponse::success(bucket)),
        None => Json(ApiResponse::error("Bucket not found".to_string())),
    }
}

#[utoipa::path(
    delete,
    path = "/api/buckets/{name}",
    tag = "buckets",
    params(
        ("name" = String, Path, description = "Bucket name")
    ),
    responses(
        (status = 200, description = "Bucket deleted successfully", body = ApiResponse<()>),
        (status = 404, description = "Bucket not found", body = ApiResponse<()>)
    )
)]
async fn delete_bucket(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Json<ApiResponse<()>> {
    match state.bucket_service.delete_bucket(&name).await {
        Ok(_) => Json(ApiResponse::success(())),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

#[utoipa::path(
    get,
    path = "/api/buckets/{bucket_name}/objects",
    tag = "objects",
    params(
        ("bucket_name" = String, Path, description = "Bucket name")
    ),
    responses(
        (status = 200, description = "List of objects", body = ApiResponse<ObjectListResponse>),
        (status = 404, description = "Bucket not found", body = ApiResponse<ObjectListResponse>)
    )
)]
async fn list_objects(
    State(state): State<Arc<AppState>>,
    Path(bucket_name): Path<String>,
) -> Json<ApiResponse<ObjectListResponse>> {
    match state.object_service.list_objects(&bucket_name, None, None, None, None).await {
        Ok(objects) => {
            let response = ObjectListResponse { objects };
            Json(ApiResponse::success(response))
        }
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

#[utoipa::path(
    put,
    path = "/api/buckets/{bucket_name}/objects/{key}",
    tag = "objects",
    params(
        ("bucket_name" = String, Path, description = "Bucket name"),
        ("key" = String, Path, description = "Object key")
    ),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses(
        (status = 200, description = "Object uploaded successfully", body = ApiResponse<Object>),
        (status = 404, description = "Bucket not found", body = ApiResponse<Object>)
    )
)]
async fn put_object(
    State(state): State<Arc<AppState>>,
    Path((bucket_name, key)): Path<(String, String)>,
    body: axum::body::Bytes,
) -> Json<ApiResponse<Object>> {
    let data = body.to_vec();
    let content_type = "application/octet-stream";
    let user_metadata = std::collections::HashMap::new();

    match state.object_service.put_object(&bucket_name, &key, data, content_type, user_metadata).await {
        Ok(object) => Json(ApiResponse::success(object)),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

#[utoipa::path(
    get,
    path = "/api/buckets/{bucket_name}/objects/{key}",
    tag = "objects",
    params(
        ("bucket_name" = String, Path, description = "Bucket name"),
        ("key" = String, Path, description = "Object key")
    ),
    responses(
        (status = 200, description = "Object data", body = Vec<u8>),
        (status = 404, description = "Object not found")
    )
)]
async fn get_object(
    State(state): State<Arc<AppState>>,
    Path((bucket_name, key)): Path<(String, String)>,
) -> Result<axum::response::Response, StatusCode> {
    match state.object_service.get_object(&bucket_name, &key).await {
        Ok((data, metadata)) => {
            let response = axum::response::Response::builder()
                .status(StatusCode::OK)
                .header("Content-Type", metadata.content_type)
                .header("ETag", metadata.etag)
                .header("Content-Length", metadata.size.to_string())
                .body(axum::body::Body::from(data))
                .unwrap();
            Ok(response)
        }
        Err(_) => Err(StatusCode::NOT_FOUND),
    }
}

#[utoipa::path(
    delete,
    path = "/api/buckets/{bucket_name}/objects/{key}",
    tag = "objects",
    params(
        ("bucket_name" = String, Path, description = "Bucket name"),
        ("key" = String, Path, description = "Object key")
    ),
    responses(
        (status = 200, description = "Object deleted successfully", body = ApiResponse<()>),
        (status = 404, description = "Object not found", body = ApiResponse<()>)
    )
)]
async fn delete_object(
    State(state): State<Arc<AppState>>,
    Path((bucket_name, key)): Path<(String, String)>,
) -> Json<ApiResponse<()>> {
    match state.object_service.delete_object(&bucket_name, &key).await {
        Ok(_) => Json(ApiResponse::success(())),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

#[utoipa::path(
    get,
    path = "/api/buckets/{bucket_name}/objects/{key}/metadata",
    tag = "objects",
    params(
        ("bucket_name" = String, Path, description = "Bucket name"),
        ("key" = String, Path, description = "Object key")
    ),
    responses(
        (status = 200, description = "Object metadata", body = ApiResponse<ObjectMetadata>),
        (status = 404, description = "Object not found", body = ApiResponse<ObjectMetadata>)
    )
)]
async fn get_object_metadata(
    State(state): State<Arc<AppState>>,
    Path((bucket_name, key)): Path<(String, String)>,
) -> Json<ApiResponse<ObjectMetadata>> {
    match state.object_service.get_object_metadata(&bucket_name, &key).await {
        Ok(metadata) => Json(ApiResponse::success(metadata)),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

#[derive(Serialize, utoipa::ToSchema)]
struct BucketListResponse {
    buckets: Vec<Bucket>,
}

#[derive(Serialize, utoipa::ToSchema)]
struct ObjectListResponse {
    objects: Vec<Object>,
}

#[derive(Serialize, utoipa::ToSchema)]
struct ApiResponse<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

impl<T> ApiResponse<T> {
    fn success(data: T) -> Self {
        Self {
            success: true,
            data: Some(data),
            error: None,
        }
    }

    fn error(message: String) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(message),
        }
    }
}