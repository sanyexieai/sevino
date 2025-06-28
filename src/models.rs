use serde::{Deserialize, Serialize};
use chrono::{DateTime, Utc};
use std::collections::HashMap;

/// 存储桶模型
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Bucket {
    /// 桶名称
    pub name: String,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 桶的元数据
    pub metadata: HashMap<String, String>,
}

/// 对象模型
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct Object {
    /// 对象键（文件名）
    pub key: String,
    /// 所属桶名称
    pub bucket_name: String,
    /// 对象大小（字节）
    pub size: u64,
    /// 内容类型
    pub content_type: String,
    /// ETag（用于缓存验证）
    pub etag: String,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后修改时间
    pub last_modified: DateTime<Utc>,
    /// 用户自定义元数据
    pub user_metadata: HashMap<String, String>,
}

/// 对象元数据
#[derive(Debug, Clone, Serialize, Deserialize, utoipa::ToSchema)]
pub struct ObjectMetadata {
    /// 对象键（文件名）
    pub key: String,
    /// 所属桶名称
    pub bucket_name: String,
    /// 对象大小（字节）
    pub size: u64,
    /// 内容类型
    pub content_type: String,
    /// ETag（用于缓存验证）
    pub etag: String,
    /// 创建时间
    pub created_at: DateTime<Utc>,
    /// 最后修改时间
    pub last_modified: DateTime<Utc>,
    /// 用户自定义元数据
    pub user_metadata: HashMap<String, String>,
    /// 版本ID（用于版本控制）
    pub version_id: Option<String>,
    /// 是否为删除标记（用于版本控制）
    pub is_delete_marker: bool,
    /// 引用计数（用于去重）
    pub reference_count: u32,
    /// 数据持有者对象ID（如果为None，则自己是数据持有者）
    pub data_holder_id: Option<String>,
}

impl Bucket {
    pub fn new(name: String) -> Self {
        Self {
            name,
            created_at: Utc::now(),
            metadata: HashMap::new(),
        }
    }
}

impl Object {
    pub fn new(
        key: String,
        bucket_name: String,
        size: u64,
        content_type: String,
        etag: String,
        user_metadata: HashMap<String, String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            key,
            bucket_name,
            size,
            content_type,
            etag,
            created_at: now,
            last_modified: now,
            user_metadata,
        }
    }
}

impl From<Object> for ObjectMetadata {
    fn from(obj: Object) -> Self {
        Self {
            key: obj.key,
            bucket_name: obj.bucket_name,
            size: obj.size,
            content_type: obj.content_type,
            etag: obj.etag,
            created_at: obj.created_at,
            last_modified: obj.last_modified,
            user_metadata: obj.user_metadata,
            version_id: None,
            is_delete_marker: false,
            reference_count: 0,
            data_holder_id: None,
        }
    }
} 