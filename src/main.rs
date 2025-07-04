use axum::{
    extract::{Path, State, Query},
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
use anyhow::Result;
use tower_http::cors::{CorsLayer, Any};
use axum::http::{Method, HeaderName};

mod models;
mod services;
mod utils;
mod config;

use crate::config::Settings;
use crate::services::{StorageService, BucketService, ObjectService, DeduplicationMode};
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
        put_object_multipart,
        get_object,
        delete_object,
        get_object_metadata,
        update_object_metadata,
        list_object_versions,
        test_duplicate_handling,
        test_reference_mode_api
    ),
    components(
        schemas(Bucket, Object, ObjectMetadata, ApiResponse<Bucket>, ApiResponse<Vec<Bucket>>, ApiResponse<Object>, ApiResponse<Vec<Object>>, ApiResponse<ObjectMetadata>, ApiResponse<()>, HealthResponse, CreateBucketRequest, PutObjectQuery, MultipartUploadQuery, UpdateObjectMetadataRequest, BucketListResponse, ObjectListResponse)
    ),
    tags(
        (name = "buckets", description = "Bucket management endpoints"),
        (name = "objects", description = "Object management endpoints"),
        (name = "health", description = "Health check endpoints"),
        (name = "test", description = "Test endpoints")
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

    // 配置CORS
    let cors_layer = if settings.enable_cors {
        let mut cors = CorsLayer::new();
        
        // 配置允许的域名
        if settings.cors_origins.contains(&"*".to_string()) {
            cors = cors.allow_origin(Any);
        } else {
            let origins: Vec<_> = settings.cors_origins
                .iter()
                .filter_map(|origin| origin.parse().ok())
                .collect();
            if !origins.is_empty() {
                cors = cors.allow_origin(origins);
            }
        }
        
        // 配置允许的方法
        let methods: Vec<Method> = settings.cors_methods
            .iter()
            .filter_map(|method| method.parse().ok())
            .collect();
        if !methods.is_empty() {
            cors = cors.allow_methods(methods);
        }
        
        // 配置允许的头部
        let headers: Vec<HeaderName> = settings.cors_headers
            .iter()
            .filter_map(|header| header.parse().ok())
            .collect();
        if !headers.is_empty() {
            cors = cors.allow_headers(headers);
        }
        
        // 配置凭据
        if settings.cors_allow_credentials {
            cors = cors.allow_credentials(true);
        }
        
        // 设置预检请求缓存时间
        cors = cors.max_age(std::time::Duration::from_secs(3600));
        
        cors
    } else {
        CorsLayer::new()
    };

    let app = Router::new()
        .route("/", get(root))
        .route("/health", get(health_check))
        .route("/api/buckets", get(list_buckets))
        .route("/api/buckets", post(create_bucket))
        .route("/api/buckets/:name", get(get_bucket))
        .route("/api/buckets/:name", delete(delete_bucket))
        .route("/api/buckets/:bucket_name/objects", get(list_objects))
        .route("/api/buckets/:bucket_name/objects/:key", put(put_object))
        .route("/api/buckets/:bucket_name/objects/:key/multipart", put(put_object_multipart))
        .route("/api/buckets/:bucket_name/objects/:key", get(get_object))
        .route("/api/buckets/:bucket_name/objects/:key", delete(delete_object))
        .route("/api/buckets/:bucket_name/objects/:key/metadata", get(get_object_metadata))
        .route("/api/buckets/:bucket_name/objects/:key/metadata", put(update_object_metadata))
        .route("/api/buckets/:bucket_name/objects/:key/versions", get(list_object_versions))
        .route("/api/buckets/:bucket_name/objects/:key/duplicate-test", post(test_duplicate_handling))
        .route("/api/test/reference-mode", get(test_reference_mode_api))
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(cors_layer)
        .with_state(Arc::new(AppState {
            bucket_service,
            object_service,
        }));

    let addr = format!("{}:{}", settings.host, settings.port);
    println!("Server running on http://{}", addr);
    println!("Swagger UI available at http://{}/swagger-ui/", addr);
    println!("CORS enabled: {}", settings.enable_cors);
    if settings.enable_cors {
        println!("CORS origins: {:?}", settings.cors_origins);
        println!("CORS methods: {:?}", settings.cors_methods);
        println!("CORS allow credentials: {}", settings.cors_allow_credentials);
    }

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
        ("bucket_name" = String, Path, description = "Bucket name"),
        ("prefix" = Option<String>, Query, description = "Object key prefix filter"),
        ("delimiter" = Option<String>, Query, description = "Delimiter for common prefixes"),
        ("max_keys" = Option<u32>, Query, description = "Maximum number of keys to return"),
        ("marker" = Option<String>, Query, description = "Pagination marker"),
        ("etag_filter" = Option<String>, Query, description = "Filter objects by ETag (supports wildcards: *, ?)"),
        ("custom_*" = Option<String>, Query, description = "Filter by custom metadata, e.g. custom_bizid=123")
    ),
    responses(
        (status = 200, description = "List of objects", body = ApiResponse<ObjectListResponse>),
        (status = 404, description = "Bucket not found", body = ApiResponse<ObjectListResponse>)
    )
)]
async fn list_objects(
    State(state): State<Arc<AppState>>,
    Path(bucket_name): Path<String>,
    Query(query): Query<ListObjectsQuery>,
    axum::extract::RawQuery(raw_query): axum::extract::RawQuery,
) -> Json<ApiResponse<ObjectListResponse>> {
    // 解析 custom_xxx=yyy 过滤条件
    let mut custom_filters = vec![];
    if let Some(raw) = raw_query {
        for (k, v) in url::form_urlencoded::parse(raw.as_bytes()) {
            if let Some(stripped) = k.strip_prefix("custom_") {
                custom_filters.push((stripped.to_string(), v.to_string()));
            }
        }
    }
    match state.object_service.list_objects_with_custom_filter(&bucket_name, query.prefix, query.delimiter, query.max_keys, query.marker, query.etag_filter, custom_filters).await {
        Ok(objects) => {
            let response = ObjectListResponse { objects };
            Json(ApiResponse::success(response))
        }
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

#[derive(Deserialize, utoipa::ToSchema)]
struct PutObjectQuery {
    #[serde(default = "default_deduplication_mode")]
    deduplication_mode: Option<String>,
    #[serde(default)]
    content_type: Option<String>,
    #[serde(default)]
    custom: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
struct ListObjectsQuery {
    #[serde(default)]
    prefix: Option<String>,
    #[serde(default)]
    delimiter: Option<String>,
    #[serde(default)]
    max_keys: Option<u32>,
    #[serde(default)]
    marker: Option<String>,
    #[serde(default)]
    etag_filter: Option<String>,
}

fn default_deduplication_mode() -> Option<String> {
    Some("allow".to_string())
}

#[derive(Deserialize, utoipa::ToSchema)]
struct MultipartUploadQuery {
    part_number: u32,
    total_parts: u32,
    upload_id: String,
    #[serde(default)]
    content_type: Option<String>,
}

#[derive(Deserialize, utoipa::ToSchema)]
struct UpdateObjectMetadataRequest {
    #[serde(default)]
    content_type: Option<String>,
    #[serde(default)]
    user_metadata: Option<HashMap<String, String>>,
    #[serde(default)]
    custom_etag: Option<String>,
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

#[utoipa::path(
    put,
    path = "/api/buckets/{bucket_name}/objects/{key}",
    tag = "objects",
    params(
        ("bucket_name" = String, Path, description = "Bucket name"),
        ("key" = String, Path, description = "Object key"),
        ("deduplication_mode" = Option<String>, Query, description = "Deduplication mode: reject, allow, reference"),
        ("content_type" = Option<String>, Query, description = "Content type"),
        ("custom_etag" = Option<String>, Query, description = "Custom ETag (e.g., \"md5-hash\", \"sha256-hash\", \"W/weak-etag\")")
    ),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses(
        (status = 200, description = "Object uploaded successfully", body = ApiResponse<Object>),
        (status = 400, description = "Invalid deduplication mode or ETag format", body = ApiResponse<Object>),
        (status = 404, description = "Bucket not found", body = ApiResponse<Object>)
    )
)]
async fn put_object(
    State(state): State<Arc<AppState>>,
    Path((bucket_name, key)): Path<(String, String)>,
    Query(query): Query<PutObjectQuery>,
    body: axum::body::Bytes,
) -> Json<ApiResponse<Object>> {
    let data = body.to_vec();
    let content_type = query.content_type.unwrap_or_else(|| "application/octet-stream".to_string());
    let mut user_metadata = std::collections::HashMap::new();

    // 解析 custom 参数（json字符串）
    if let Some(custom_str) = &query.custom {
        match serde_json::from_str::<HashMap<String, String>>(custom_str) {
            Ok(map) => user_metadata.extend(map),
            Err(e) => return Json(ApiResponse::error(format!("Invalid custom metadata: {}", e))),
        }
    }

    // 如果指定了去重模式，使用去重上传
    if let Some(dedup_mode) = query.deduplication_mode {
        let deduplication_mode = match dedup_mode.to_lowercase().as_str() {
            "reject" => DeduplicationMode::Reject,
            "allow" => DeduplicationMode::Allow,
            "reference" => DeduplicationMode::Reference,
            _ => {
                return Json(ApiResponse::error(format!(
                    "Invalid deduplication mode: {}. Valid modes are: reject, allow, reference",
                    dedup_mode
                )));
            }
        };

        match state.object_service.put_object_with_deduplication(
            &bucket_name, 
            &key, 
            data, 
            &content_type, 
            user_metadata,
            deduplication_mode
        ).await {
            Ok(object) => Json(ApiResponse::success(object)),
            Err(e) => Json(ApiResponse::error(e.to_string())),
        }
    } else {
        // 默认上传模式 - 使用 Allow 模式允许重复内容
        match state.object_service.put_object_with_deduplication(
            &bucket_name, 
            &key, 
            data, 
            &content_type, 
            user_metadata,
            DeduplicationMode::Allow
        ).await {
            Ok(object) => Json(ApiResponse::success(object)),
            Err(e) => Json(ApiResponse::error(e.to_string())),
        }
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
    path = "/api/buckets/{bucket_name}/objects/{key}/versions",
    tag = "objects",
    params(
        ("bucket_name" = String, Path, description = "Bucket name"),
        ("key" = String, Path, description = "Object key")
    ),
    responses(
        (status = 200, description = "List of object versions", body = ApiResponse<Vec<ObjectMetadata>>),
        (status = 404, description = "Object not found", body = ApiResponse<Vec<ObjectMetadata>>)
    )
)]
async fn list_object_versions(
    State(state): State<Arc<AppState>>,
    Path((bucket_name, key)): Path<(String, String)>,
) -> Json<ApiResponse<Vec<ObjectMetadata>>> {
    match state.object_service.list_object_versions(&bucket_name, &key).await {
        Ok(versions) => Json(ApiResponse::success(versions)),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

#[utoipa::path(
    post,
    path = "/api/buckets/{bucket_name}/objects/{key}/duplicate-test",
    tag = "objects",
    params(
        ("bucket_name" = String, Path, description = "Bucket name"),
        ("key" = String, Path, description = "Object key")
    ),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses(
        (status = 200, description = "Duplicate handling test result", body = ApiResponse<String>),
        (status = 404, description = "Object not found", body = ApiResponse<String>)
    )
)]
async fn test_duplicate_handling(
    State(state): State<Arc<AppState>>,
    Path((bucket_name, key)): Path<(String, String)>,
    body: axum::body::Bytes,
) -> Json<ApiResponse<String>> {
    let data = body.to_vec();
    let content_type = "application/octet-stream";
    let user_metadata = std::collections::HashMap::new();

    match state.object_service.test_duplicate_handling(&bucket_name, &key, data, content_type, user_metadata).await {
        Ok(result) => Json(ApiResponse::success(result)),
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

/// 测试Reference模式的工作原理
async fn test_reference_mode() -> Result<String> {
    let storage = StorageService::new("./data".to_string()).await?;
    let object_service = ObjectService::new(storage.clone());
    let bucket_service = BucketService::new(storage);
    
    let bucket_name = "test-reference-bucket-v2";
    let test_data = b"Hello, this is test content for decentralized reference mode!".to_vec();
    let content_type = "text/plain";
    let mut user_metadata = HashMap::new();
    user_metadata.insert("test".to_string(), "reference".to_string());
    
    let mut result = String::new();
    result.push_str("=== 去中心化Reference模式测试 ===\n\n");
    
    // 1. 创建桶
    result.push_str("1. 创建测试桶\n");
    match bucket_service.create_bucket(bucket_name.to_string()).await {
        Ok(_) => result.push_str("   ✓ 桶创建成功\n\n"),
        Err(e) => result.push_str(&format!("   ✗ 桶创建失败: {}\n\n", e)),
    }
    
    // 2. 上传第一个文件
    result.push_str("2. 上传第一个文件 (key: file1.txt)\n");
    match object_service.put_object(bucket_name, "file1.txt", test_data.clone(), content_type, user_metadata.clone()).await {
        Ok(obj) => {
            result.push_str(&format!("   ✓ 文件上传成功\n"));
            result.push_str(&format!("   - ETag: {}\n", obj.etag));
            result.push_str(&format!("   - 大小: {} bytes\n", obj.size));
            result.push_str(&format!("   - 对象ID: {}\n\n", StorageService::generate_object_id(bucket_name, "file1.txt")));
        },
        Err(e) => result.push_str(&format!("   ✗ 文件上传失败: {}\n\n", e)),
    }
    
    // 3. 使用Reference模式上传相同内容的不同key
    result.push_str("3. 使用Reference模式上传相同内容 (key: file2.txt)\n");
    match object_service.put_object_with_deduplication(
        bucket_name, 
        "file2.txt", 
        test_data.clone(), 
        content_type, 
        user_metadata.clone(),
        DeduplicationMode::Reference
    ).await {
        Ok(obj) => {
            result.push_str(&format!("   ✓ 引用创建成功\n"));
            result.push_str(&format!("   - ETag: {}\n", obj.etag));
            result.push_str(&format!("   - 大小: {} bytes\n", obj.size));
            result.push_str(&format!("   - 对象ID: {}\n", StorageService::generate_object_id(bucket_name, "file2.txt")));
            
            // 检查元数据
            if let Ok(metadata) = object_service.get_object_metadata(bucket_name, "file2.txt").await {
                result.push_str(&format!("   - 数据持有者ID: {:?}\n", metadata.data_holder_id));
                result.push_str(&format!("   - 引用计数: {}\n", metadata.reference_count));
            }
            result.push_str("\n");
        },
        Err(e) => result.push_str(&format!("   ✗ 引用创建失败: {}\n\n", e)),
    }
    
    // 4. 检查数据持有者的引用计数
    result.push_str("4. 检查数据持有者的引用计数\n");
    if let Ok(metadata) = object_service.get_object_metadata(bucket_name, "file1.txt").await {
        result.push_str(&format!("   file1.txt 引用计数: {}\n", metadata.reference_count));
        result.push_str(&format!("   file1.txt 数据持有者ID: {:?}\n", metadata.data_holder_id));
    }
    result.push_str("\n");
    
    // 5. 读取两个文件并比较
    result.push_str("5. 读取并比较两个文件\n");
    match object_service.get_object(bucket_name, "file1.txt").await {
        Ok((data1, metadata1)) => {
            result.push_str(&format!("   file1.txt 读取成功，大小: {} bytes\n", data1.len()));
            
            match object_service.get_object(bucket_name, "file2.txt").await {
                Ok((data2, metadata2)) => {
                    result.push_str(&format!("   file2.txt 读取成功，大小: {} bytes\n", data2.len()));
                    result.push_str(&format!("   数据相同: {}\n", data1 == data2));
                    result.push_str(&format!("   ETag相同: {}\n", metadata1.etag == metadata2.etag));
                    result.push_str(&format!("   file1数据持有者ID: {:?}\n", metadata1.data_holder_id));
                    result.push_str(&format!("   file2数据持有者ID: {:?}\n", metadata2.data_holder_id));
                },
                Err(e) => result.push_str(&format!("   file2.txt 读取失败: {}\n", e)),
            }
        },
        Err(e) => result.push_str(&format!("   file1.txt 读取失败: {}\n", e)),
    }
    result.push_str("\n");
    
    // 6. 测试删除引用对象
    result.push_str("6. 测试删除引用对象\n");
    match object_service.delete_object(bucket_name, "file2.txt").await {
        Ok(_) => {
            result.push_str("   ✓ 引用对象删除成功\n");
            
            // 检查数据持有者的引用计数是否减少
            if let Ok(metadata) = object_service.get_object_metadata(bucket_name, "file1.txt").await {
                result.push_str(&format!("   file1.txt 引用计数: {}\n", metadata.reference_count));
            }
        },
        Err(e) => result.push_str(&format!("   ✗ 引用对象删除失败: {}\n", e)),
    }
    result.push_str("\n");
    
    // 7. 测试删除数据持有者（应该成功，因为没有引用了）
    result.push_str("7. 测试删除数据持有者（应该成功）\n");
    match object_service.delete_object(bucket_name, "file1.txt").await {
        Ok(_) => result.push_str("   ✓ 数据持有者删除成功\n"),
        Err(e) => result.push_str(&format!("   ✗ 数据持有者删除失败: {}\n", e)),
    }
    result.push_str("\n");
    
    // 8. 测试多个对象的引用关系
    result.push_str("8. 测试多个对象的引用关系\n");
    match object_service.put_object(bucket_name, "file3.txt", test_data.clone(), content_type, user_metadata.clone()).await {
        Ok(_) => {
            result.push_str("   ✓ file3.txt 上传成功\n");
            
            // 创建多个引用
            for i in 4..=6 {
                let key = format!("file{}.txt", i);
                match object_service.put_object_with_deduplication(
                    bucket_name, 
                    &key, 
                    test_data.clone(), 
                    content_type, 
                    user_metadata.clone(),
                    DeduplicationMode::Reference
                ).await {
                    Ok(_) => result.push_str(&format!("   ✓ {} 引用创建成功\n", key)),
                    Err(e) => result.push_str(&format!("   ✗ {} 引用创建失败: {}\n", key, e)),
                }
            }
            
            // 检查引用计数
            if let Ok(metadata) = object_service.get_object_metadata(bucket_name, "file3.txt").await {
                result.push_str(&format!("   file3.txt 引用计数: {}\n", metadata.reference_count));
            }
        },
        Err(e) => result.push_str(&format!("   ✗ file3.txt 上传失败: {}\n", e)),
    }
    result.push_str("\n");
    
    // 9. 验证所有对象都可以正常读取
    result.push_str("9. 验证所有对象都可以正常读取\n");
    for i in 3..=6 {
        let key = format!("file{}.txt", i);
        match object_service.get_object(bucket_name, &key).await {
            Ok((data, _)) => result.push_str(&format!("   ✓ {} 读取成功，大小: {} bytes\n", key, data.len())),
            Err(e) => result.push_str(&format!("   ✗ {} 读取失败: {}\n", key, e)),
        }
    }
    
    Ok(result)
}

#[utoipa::path(
    get,
    path = "/api/test/reference-mode",
    tag = "test",
    responses(
        (status = 200, description = "Reference mode test results", body = ApiResponse<String>)
    )
)]
async fn test_reference_mode_api() -> Json<ApiResponse<String>> {
    match test_reference_mode().await {
        Ok(result) => Json(ApiResponse::success(result)),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

#[utoipa::path(
    put,
    path = "/api/buckets/{bucket_name}/objects/{key}/multipart",
    tag = "objects",
    params(
        ("bucket_name" = String, Path, description = "Bucket name"),
        ("key" = String, Path, description = "Object key"),
        ("part_number" = u32, Query, description = "分片编号，从1开始"),
        ("total_parts" = u32, Query, description = "总分片数"),
        ("upload_id" = String, Query, description = "上传ID"),
        ("content_type" = Option<String>, Query, description = "内容类型")
    ),
    request_body(content = Vec<u8>, content_type = "application/octet-stream"),
    responses(
        (status = 200, description = "Multipart upload part uploaded successfully", body = ApiResponse<Object>),
        (status = 400, description = "Invalid multipart upload request", body = ApiResponse<Object>),
        (status = 404, description = "Bucket not found", body = ApiResponse<Object>)
    )
)]
async fn put_object_multipart(
    State(state): State<Arc<AppState>>,
    Path((bucket_name, key)): Path<(String, String)>,
    Query(query): Query<MultipartUploadQuery>,
    body: axum::body::Bytes,
) -> Json<ApiResponse<Object>> {
    let data = body.to_vec();
    let content_type = query.content_type.clone().unwrap_or_else(|| "application/octet-stream".to_string());
    let mut user_metadata = std::collections::HashMap::new();
    user_metadata.insert("multipart_upload_id".to_string(), query.upload_id.clone());
    user_metadata.insert("part_number".to_string(), query.part_number.to_string());
    user_metadata.insert("total_parts".to_string(), query.total_parts.to_string());
    let part_key = format!("{}.part.{}", key, query.part_number);
    match state.object_service.put_object(&bucket_name, &part_key, data, &content_type, user_metadata).await {
        Ok(object) => Json(ApiResponse::success(object)),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}

#[utoipa::path(
    put,
    path = "/api/buckets/{bucket_name}/objects/{key}/metadata",
    tag = "objects",
    params(
        ("bucket_name" = String, Path, description = "Bucket name"),
        ("key" = String, Path, description = "Object key")
    ),
    request_body(content = UpdateObjectMetadataRequest, content_type = "application/json"),
    responses(
        (status = 200, description = "Object metadata updated successfully", body = ApiResponse<Object>),
        (status = 400, description = "Invalid ETag format", body = ApiResponse<Object>),
        (status = 404, description = "Object not found", body = ApiResponse<Object>)
    )
)]
async fn update_object_metadata(
    State(state): State<Arc<AppState>>,
    Path((bucket_name, key)): Path<(String, String)>,
    Json(request): Json<UpdateObjectMetadataRequest>,
) -> Json<ApiResponse<Object>> {
    match state.object_service.update_object_metadata(
        &bucket_name,
        &key,
        request.content_type,
        request.user_metadata,
        request.custom_etag,
    ).await {
        Ok(object) => Json(ApiResponse::success(object)),
        Err(e) => Json(ApiResponse::error(e.to_string())),
    }
}