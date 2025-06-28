use crate::models::{Bucket, Object, ObjectMetadata};
use crate::utils::{validate_bucket_name, validate_object_key, generate_etag, get_mime_type, sanitize_path, sha256_hash};
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json;
use chrono;
use std::time::{SystemTime, UNIX_EPOCH};

/// 重复数据删除模式
#[derive(Debug, Clone)]
pub enum DeduplicationMode {
    /// 拒绝重复内容
    Reject,
    /// 允许重复内容
    Allow,
    /// 创建引用（节省存储空间）
    Reference,
}

/// 存储服务 - 参考MinIO的存储结构
#[derive(Clone)]
pub struct StorageService {
    data_dir: PathBuf,
    buckets: Arc<RwLock<HashMap<String, Bucket>>>,
    object_index: Arc<RwLock<HashMap<String, HashMap<String, String>>>>,
    etag_index: Arc<RwLock<HashMap<String, HashMap<String, Vec<String>>>>>,
}

impl StorageService {
    pub async fn new(data_dir: String) -> Result<Self> {
        let data_path = PathBuf::from(data_dir);
        
        // 创建数据目录
        if !data_path.exists() {
            fs::create_dir_all(&data_path)?;
        }
        
        // 加载现有桶
        let buckets = Self::load_buckets(&data_path).await?;
        
        // 构建对象索引
        let object_index = Self::build_object_index(&data_path).await?;
        
        // 构建ETag索引
        let etag_index = Self::build_etag_index(&data_path).await?;
        
        Ok(Self {
            data_dir: data_path,
            buckets: Arc::new(RwLock::new(buckets)),
            object_index: Arc::new(RwLock::new(object_index)),
            etag_index: Arc::new(RwLock::new(etag_index)),
        })
    }
    
    async fn load_buckets(data_dir: &Path) -> Result<HashMap<String, Bucket>> {
        let mut buckets = HashMap::new();
        
        if data_dir.exists() {
            for entry in fs::read_dir(data_dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.is_dir() {
                    let bucket_name = path.file_name()
                        .and_then(|name| name.to_str())
                        .ok_or_else(|| anyhow!("Invalid bucket name"))?;
                    
                    // 跳过系统目录
                    if bucket_name.starts_with('.') {
                        continue;
                    }
                    
                    let metadata_path = path.join(".sevino.meta").join("bucket.json");
                    let bucket = if metadata_path.exists() {
                        let content = fs::read_to_string(metadata_path)?;
                        serde_json::from_str(&content)?
                    } else {
                        Bucket::new(bucket_name.to_string())
                    };
                    
                    buckets.insert(bucket_name.to_string(), bucket);
                }
            }
        }
        
        Ok(buckets)
    }
    
    async fn build_object_index(data_dir: &Path) -> Result<HashMap<String, HashMap<String, String>>> {
        let mut index = HashMap::new();
        
        if data_dir.exists() {
            for entry in fs::read_dir(data_dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.is_dir() {
                    let bucket_name = path.file_name()
                        .and_then(|name| name.to_str())
                        .ok_or_else(|| anyhow!("Invalid bucket name"))?;
                    
                    // 跳过系统目录
                    if bucket_name.starts_with('.') {
                        continue;
                    }
                    
                    let mut bucket_index = HashMap::new();
                    let meta_dir = path.join(".sevino.meta").join("objects");
                    
                    if meta_dir.exists() {
                        for meta_entry in fs::read_dir(meta_dir)? {
                            let meta_entry = meta_entry?;
                            let meta_path = meta_entry.path();
                            
                            if meta_path.is_file() && meta_path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                                if let Ok(content) = fs::read_to_string(&meta_path) {
                                    if let Ok(metadata) = serde_json::from_str::<ObjectMetadata>(&content) {
                                        let object_id = Self::generate_object_id(bucket_name, &metadata.key);
                                        bucket_index.insert(metadata.key, object_id);
                                    }
                                }
                            }
                        }
                    }
                    
                    if !bucket_index.is_empty() {
                        index.insert(bucket_name.to_string(), bucket_index);
                    }
                }
            }
        }
        
        Ok(index)
    }
    
    async fn build_etag_index(data_dir: &Path) -> Result<HashMap<String, HashMap<String, Vec<String>>>> {
        let mut etag_index = HashMap::new();
        
        if data_dir.exists() {
            for entry in fs::read_dir(data_dir)? {
                let entry = entry?;
                let path = entry.path();
                
                if path.is_dir() {
                    let bucket_name = path.file_name()
                        .and_then(|name| name.to_str())
                        .ok_or_else(|| anyhow!("Invalid bucket name"))?;
                    
                    // 跳过系统目录
                    if bucket_name.starts_with('.') {
                        continue;
                    }
                    
                    let mut bucket_etag_index = HashMap::new();
                    let meta_dir = path.join(".sevino.meta").join("objects");
                    
                    if meta_dir.exists() {
                        for meta_entry in fs::read_dir(meta_dir)? {
                            let meta_entry = meta_entry?;
                            let meta_path = meta_entry.path();
                            
                            if meta_path.is_file() && meta_path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                                if let Ok(content) = fs::read_to_string(&meta_path) {
                                    if let Ok(metadata) = serde_json::from_str::<ObjectMetadata>(&content) {
                                        let object_id = Self::generate_object_id(bucket_name, &metadata.key);
                                        bucket_etag_index
                                            .entry(metadata.etag)
                                            .or_insert_with(Vec::new)
                                            .push(object_id);
                                    }
                                }
                            }
                        }
                    }
                    
                    if !bucket_etag_index.is_empty() {
                        etag_index.insert(bucket_name.to_string(), bucket_etag_index);
                    }
                }
            }
        }
        
        Ok(etag_index)
    }
    
    /// 生成对象ID（类似MinIO的哈希化文件名）
    pub fn generate_object_id(bucket_name: &str, key: &str) -> String {
        let combined = format!("{}:{}", bucket_name, key);
        sha256_hash(combined.as_bytes())
    }
    
    /// 获取对象存储路径（使用哈希化文件名）
    fn get_object_data_path(&self, bucket_name: &str, object_id: &str) -> PathBuf {
        // 使用前4个字符作为目录名，避免单个目录文件过多
        let prefix = &object_id[..4];
        let sub_prefix = &object_id[4..6];
        
        self.data_dir
            .join(bucket_name)
            .join(prefix)
            .join(sub_prefix)
            .join(object_id)
    }
    
    /// 获取对象元数据路径
    fn get_object_metadata_path(&self, bucket_name: &str, object_id: &str) -> PathBuf {
        self.data_dir
            .join(bucket_name)
            .join(".sevino.meta")
            .join("objects")
            .join(format!("{}.json", object_id))
    }
    
    /// 获取桶元数据路径
    fn get_bucket_metadata_path(&self, bucket_name: &str) -> PathBuf {
        self.data_dir
            .join(bucket_name)
            .join(".sevino.meta")
            .join("bucket.json")
    }
    
    pub async fn save_bucket_metadata(&self, bucket: &Bucket) -> Result<()> {
        let bucket_dir = self.data_dir.join(&bucket.name);
        if !bucket_dir.exists() {
            fs::create_dir_all(&bucket_dir)?;
        }
        
        // 创建.sevino.meta目录
        let meta_dir = bucket_dir.join(".sevino.meta");
        if !meta_dir.exists() {
            fs::create_dir_all(&meta_dir)?;
        }
        
        let metadata_path = self.get_bucket_metadata_path(&bucket.name);
        let content = serde_json::to_string_pretty(bucket)?;
        fs::write(metadata_path, content)?;
        
        Ok(())
    }
    
    pub async fn delete_bucket_directory(&self, bucket_name: &str) -> Result<()> {
        let bucket_dir = self.data_dir.join(bucket_name);
        if bucket_dir.exists() {
            fs::remove_dir_all(bucket_dir)?;
        }
        Ok(())
    }
    
    pub async fn save_object_metadata(&self, bucket_name: &str, object_id: &str, metadata: &ObjectMetadata) -> Result<()> {
        let meta_dir = self.data_dir
            .join(bucket_name)
            .join(".sevino.meta")
            .join("objects");
        
        if !meta_dir.exists() {
            fs::create_dir_all(&meta_dir)?;
        }
        
        let metadata_path = self.get_object_metadata_path(bucket_name, object_id);
        let content = serde_json::to_string_pretty(metadata)?;
        fs::write(metadata_path, content)?;
        
        Ok(())
    }
    
    pub async fn load_object_metadata(&self, bucket_name: &str, object_id: &str) -> Result<Option<ObjectMetadata>> {
        let metadata_path = self.get_object_metadata_path(bucket_name, object_id);
        
        if metadata_path.exists() {
            let content = fs::read_to_string(metadata_path)?;
            let metadata: ObjectMetadata = serde_json::from_str(&content)?;
            Ok(Some(metadata))
        } else {
            Ok(None)
        }
    }
    
    pub async fn delete_object_metadata(&self, bucket_name: &str, object_id: &str) -> Result<()> {
        let metadata_path = self.get_object_metadata_path(bucket_name, object_id);
        if metadata_path.exists() {
            fs::remove_file(metadata_path)?;
        }
        Ok(())
    }
    
    pub async fn list_object_metadata(&self, bucket_name: &str) -> Result<Vec<ObjectMetadata>> {
        self.list_object_metadata_with_pagination(bucket_name, None, None).await
    }
    
    pub async fn list_object_metadata_with_pagination(
        &self,
        bucket_name: &str,
        max_keys: Option<usize>,
        marker: Option<String>,
    ) -> Result<Vec<ObjectMetadata>> {
        let meta_dir = self.data_dir
            .join(bucket_name)
            .join(".sevino.meta")
            .join("objects");
        
        let mut objects = Vec::new();
        let mut count = 0;
        let max_keys = max_keys.unwrap_or(usize::MAX);
        
        if meta_dir.exists() {
            let mut entries: Vec<_> = fs::read_dir(meta_dir)?
                .filter_map(|entry| entry.ok())
                .collect();
            
            // 按文件名排序，确保一致性
            entries.sort_by(|a, b| a.file_name().cmp(&b.file_name()));
            
            let mut started = marker.is_none();
            
            for entry in entries {
                if count >= max_keys {
                    break;
                }
                
                let path = entry.path();
                
                if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("json") {
                    // 处理marker逻辑
                    if !started {
                        if let Some(marker_val) = &marker {
                            let file_name = path.file_name()
                                .and_then(|name| name.to_str())
                                .unwrap_or("");
                            if file_name == marker_val {
                                started = true;
                            }
                            continue;
                        }
                    }
                    
                    if let Ok(content) = fs::read_to_string(&path) {
                        if let Ok(metadata) = serde_json::from_str::<ObjectMetadata>(&content) {
                            objects.push(metadata);
                            count += 1;
                        }
                    }
                }
            }
        }
        
        Ok(objects)
    }
    
    /// 根据key查找对象ID
    pub async fn find_object_id_by_key(&self, bucket_name: &str, key: &str) -> Result<Option<String>> {
        let index = self.object_index.read().await;
        
        if let Some(bucket_index) = index.get(bucket_name) {
            if let Some(object_id) = bucket_index.get(key) {
                return Ok(Some(object_id.clone()));
            }
        }
        
        Ok(None)
    }
    
    /// 添加对象到索引
    pub async fn add_object_to_index(&self, bucket_name: &str, key: &str, object_id: &str) -> Result<()> {
        let mut index = self.object_index.write().await;
        
        let bucket_index = index.entry(bucket_name.to_string())
            .or_insert_with(HashMap::new);
        
        bucket_index.insert(key.to_string(), object_id.to_string());
        
        Ok(())
    }
    
    /// 从索引中删除对象
    pub async fn remove_object_from_index(&self, bucket_name: &str, key: &str) -> Result<()> {
        let mut index = self.object_index.write().await;
        
        if let Some(bucket_index) = index.get_mut(bucket_name) {
            bucket_index.remove(key);
            
            // 如果桶索引为空，删除整个桶索引
            if bucket_index.is_empty() {
                index.remove(bucket_name);
            }
        }
        
        Ok(())
    }
    
    /// 获取桶中对象数量（使用索引，O(1)性能）
    pub async fn get_bucket_object_count(&self, bucket_name: &str) -> usize {
        let index = self.object_index.read().await;
        
        if let Some(bucket_index) = index.get(bucket_name) {
            bucket_index.len()
        } else {
            0
        }
    }
    
    /// 检查桶是否为空（使用索引，O(1)性能）
    pub async fn is_bucket_empty(&self, bucket_name: &str) -> bool {
        self.get_bucket_object_count(bucket_name).await == 0
    }
    
    /// 重建对象索引（用于修复索引不一致问题）
    pub async fn rebuild_object_index(&self) -> Result<()> {
        let new_index = Self::build_object_index(&self.data_dir).await?;
        let mut index = self.object_index.write().await;
        *index = new_index;
        Ok(())
    }
    
    /// 验证索引一致性
    pub async fn validate_index_consistency(&self, bucket_name: &str) -> Result<bool> {
        let index_count = self.get_bucket_object_count(bucket_name).await;
        let disk_objects = self.list_object_metadata(bucket_name).await?;
        let disk_count = disk_objects.len();
        
        Ok(index_count == disk_count)
    }
    
    /// 添加ETag到索引
    pub async fn add_etag_to_index(&self, bucket_name: &str, etag: &str, object_id: &str) -> Result<()> {
        let mut etag_index = self.etag_index.write().await;
        
        let bucket_etag_index = etag_index.entry(bucket_name.to_string())
            .or_insert_with(HashMap::new);
        
        bucket_etag_index
            .entry(etag.to_string())
            .or_insert_with(Vec::new)
            .push(object_id.to_string());
        
        Ok(())
    }
    
    /// 从ETag索引中删除
    pub async fn remove_etag_from_index(&self, bucket_name: &str, etag: &str, object_id: &str) -> Result<()> {
        let mut etag_index = self.etag_index.write().await;
        
        if let Some(bucket_etag_index) = etag_index.get_mut(bucket_name) {
            if let Some(object_ids) = bucket_etag_index.get_mut(etag) {
                object_ids.retain(|id| id != object_id);
                
                // 如果没有对象引用这个ETag，删除整个ETag条目
                if object_ids.is_empty() {
                    bucket_etag_index.remove(etag);
                }
            }
            
            // 如果桶的ETag索引为空，删除整个桶索引
            if bucket_etag_index.is_empty() {
                etag_index.remove(bucket_name);
            }
        }
        
        Ok(())
    }
    
    /// 根据ETag查找所有对象
    pub async fn find_objects_by_etag(&self, bucket_name: &str, etag: &str) -> Result<Vec<String>> {
        let etag_index = self.etag_index.read().await;
        
        if let Some(bucket_etag_index) = etag_index.get(bucket_name) {
            if let Some(object_ids) = bucket_etag_index.get(etag) {
                return Ok(object_ids.clone());
            }
        }
        
        Ok(Vec::new())
    }
    
    /// 检查ETag是否已存在（跨key检测）
    pub async fn is_etag_exists(&self, bucket_name: &str, etag: &str) -> Result<bool> {
        let object_ids = self.find_objects_by_etag(bucket_name, etag).await?;
        Ok(!object_ids.is_empty())
    }
}

/// 桶服务
#[derive(Clone)]
pub struct BucketService {
    storage: StorageService,
}

impl BucketService {
    pub fn new(storage: StorageService) -> Self {
        Self { storage }
    }
    
    pub async fn list_buckets(&self) -> Vec<Bucket> {
        let buckets = self.storage.buckets.read().await;
        buckets.values().cloned().collect()
    }
    
    pub async fn create_bucket(&self, name: String) -> Result<Bucket> {
        validate_bucket_name(&name).map_err(|e| anyhow!(e))?;
        
        let mut buckets = self.storage.buckets.write().await;
        
        if buckets.contains_key(&name) {
            return Err(anyhow!("Bucket '{}' already exists", name));
        }
        
        let bucket = Bucket::new(name.clone());
        self.storage.save_bucket_metadata(&bucket).await?;
        buckets.insert(name, bucket.clone());
        
        Ok(bucket)
    }
    
    pub async fn get_bucket(&self, name: &str) -> Option<Bucket> {
        let buckets = self.storage.buckets.read().await;
        buckets.get(name).cloned()
    }
    
    pub async fn delete_bucket(&self, name: &str) -> Result<()> {
        let mut buckets = self.storage.buckets.write().await;
        
        if !buckets.contains_key(name) {
            return Err(anyhow!("Bucket '{}' not found", name));
        }
        
        // 检查桶是否为空（使用索引，O(1)性能）
        if !self.storage.is_bucket_empty(name).await {
            return Err(anyhow!("Cannot delete non-empty bucket '{}'", name));
        }
        
        self.storage.delete_bucket_directory(name).await?;
        buckets.remove(name);
        
        Ok(())
    }
}

/// 对象服务
#[derive(Clone)]
pub struct ObjectService {
    storage: StorageService,
}

impl ObjectService {
    pub fn new(storage: StorageService) -> Self {
        Self { storage }
    }
    
    pub async fn put_object(
        &self,
        bucket_name: &str,
        key: &str,
        data: Vec<u8>,
        content_type: &str,
        user_metadata: HashMap<String, String>,
    ) -> Result<Object> {
        self.put_object_with_versioning(bucket_name, key, data, content_type, user_metadata, false).await
    }
    
    pub async fn put_object_with_versioning(
        &self,
        bucket_name: &str,
        key: &str,
        data: Vec<u8>,
        content_type: &str,
        user_metadata: HashMap<String, String>,
        enable_versioning: bool,
    ) -> Result<Object> {
        validate_object_key(key).map_err(|e| anyhow!(e))?;
        
        // 检查桶是否存在
        let bucket = self.storage.buckets.read().await;
        if !bucket.contains_key(bucket_name) {
            return Err(anyhow!("Bucket '{}' not found", bucket_name));
        }
        drop(bucket);
        
        let etag = generate_etag(&data);
        let mime_type = if content_type == "application/octet-stream" {
            get_mime_type(key)
        } else {
            content_type.to_string()
        };
        
        // 检查是否存在相同内容的文件（跨key检测）
        if self.storage.is_etag_exists(bucket_name, &etag).await? {
            // 找到相同内容的文件，可以选择：
            // 1. 拒绝上传（避免重复）
            // 2. 创建软链接（节省空间）
            // 3. 正常上传（覆盖）
            
            // 这里我们实现选项1：拒绝上传
            let existing_objects = self.storage.find_objects_by_etag(bucket_name, &etag).await?;
            if !existing_objects.is_empty() {
                // 获取第一个相同内容的对象的key
                if let Some(first_object_id) = existing_objects.first() {
                    if let Some(existing_metadata) = self.storage.load_object_metadata(bucket_name, first_object_id).await? {
                        return Err(anyhow!(
                            "Content already exists with key '{}' (ETag: {}). Use different content or enable deduplication.",
                            existing_metadata.key, etag
                        ));
                    }
                }
            }
        }
        
        // 检查是否存在相同内容的文件
        if let Some(existing_object_id) = self.storage.find_object_id_by_key(bucket_name, key).await? {
            if let Some(existing_metadata) = self.storage.load_object_metadata(bucket_name, &existing_object_id).await? {
                // 如果ETag相同，说明内容相同
                if existing_metadata.etag == etag {
                    // 更新元数据（时间戳等），但不重新存储数据
                    let mut updated_metadata = existing_metadata.clone();
                    updated_metadata.last_modified = chrono::Utc::now();
                    updated_metadata.user_metadata = user_metadata;
                    
                    self.storage.save_object_metadata(bucket_name, &existing_object_id, &updated_metadata).await?;
                    
                    return Ok(Object::new(
                        key.to_string(),
                        bucket_name.to_string(),
                        updated_metadata.size,
                        updated_metadata.content_type,
                        updated_metadata.etag,
                        updated_metadata.user_metadata,
                    ));
                }
            }
        }
        
        // 生成版本ID（如果启用版本控制）
        let version_id = if enable_versioning {
            Some(self.generate_version_id())
        } else {
            None
        };
        
        let object = Object::new(
            key.to_string(),
            bucket_name.to_string(),
            data.len() as u64,
            mime_type,
            etag.clone(),
            user_metadata,
        );
        
        // 生成对象ID（包含版本信息）
        let object_id = if let Some(vid) = &version_id {
            format!("{}_{}", StorageService::generate_object_id(bucket_name, key), vid)
        } else {
            StorageService::generate_object_id(bucket_name, key)
        };
        
        // 保存对象数据（使用哈希化文件名）
        let object_path = self.storage.get_object_data_path(bucket_name, &object_id);
        if let Some(parent) = object_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
        fs::write(&object_path, data)?;
        
        // 保存元数据
        let mut metadata: ObjectMetadata = object.clone().into();
        if let Some(vid) = version_id {
            metadata.version_id = Some(vid);
        }
        self.storage.save_object_metadata(bucket_name, &object_id, &metadata).await?;
        
        // 更新索引
        self.storage.add_object_to_index(bucket_name, key, &object_id).await?;
        self.storage.add_etag_to_index(bucket_name, &etag, &object_id).await?;
        
        Ok(object)
    }
    
    /// 生成版本ID
    fn generate_version_id(&self) -> String {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("{:016x}", now)
    }
    
    /// 检查文件是否重复（基于ETag）
    pub async fn is_duplicate_content(&self, bucket_name: &str, key: &str, etag: &str) -> Result<bool> {
        if let Some(existing_object_id) = self.storage.find_object_id_by_key(bucket_name, key).await? {
            if let Some(existing_metadata) = self.storage.load_object_metadata(bucket_name, &existing_object_id).await? {
                return Ok(existing_metadata.etag == etag);
            }
        }
        Ok(false)
    }
    
    /// 检查是否存在相同内容的其他文件（跨key检测）
    pub async fn find_duplicate_content_keys(&self, bucket_name: &str, etag: &str, exclude_key: Option<&str>) -> Result<Vec<String>> {
        let object_ids = self.storage.find_objects_by_etag(bucket_name, etag).await?;
        let mut duplicate_keys = Vec::new();
        
        for object_id in object_ids {
            if let Some(metadata) = self.storage.load_object_metadata(bucket_name, &object_id).await? {
                // 排除指定的key
                if let Some(exclude) = exclude_key {
                    if metadata.key != exclude {
                        duplicate_keys.push(metadata.key);
                    }
                } else {
                    duplicate_keys.push(metadata.key);
                }
            }
        }
        
        Ok(duplicate_keys)
    }
    
    /// 条件上传（只有当文件不存在或内容不同时才上传）
    pub async fn put_object_if_not_exists(
        &self,
        bucket_name: &str,
        key: &str,
        data: Vec<u8>,
        content_type: &str,
        user_metadata: HashMap<String, String>,
    ) -> Result<Object> {
        let etag = generate_etag(&data);
        
        // 检查文件是否已存在且内容相同
        if self.is_duplicate_content(bucket_name, key, &etag).await? {
            return Err(anyhow!("Object '{}' already exists with same content", key));
        }
        
        self.put_object(bucket_name, key, data, content_type, user_metadata).await
    }
    
    /// 条件上传（只有当ETag不匹配时才上传）
    pub async fn put_object_if_etag_mismatch(
        &self,
        bucket_name: &str,
        key: &str,
        data: Vec<u8>,
        content_type: &str,
        user_metadata: HashMap<String, String>,
        expected_etag: &str,
    ) -> Result<Object> {
        let etag = generate_etag(&data);
        
        // 检查当前ETag是否与期望的ETag匹配
        if let Some(existing_object_id) = self.storage.find_object_id_by_key(bucket_name, key).await? {
            if let Some(existing_metadata) = self.storage.load_object_metadata(bucket_name, &existing_object_id).await? {
                if existing_metadata.etag == expected_etag {
                    return Err(anyhow!("ETag precondition failed: expected '{}', got '{}'", expected_etag, existing_metadata.etag));
                }
            }
        }
        
        self.put_object(bucket_name, key, data, content_type, user_metadata).await
    }
    
    /// 智能上传：如果内容已存在，可以选择创建引用或拒绝上传
    pub async fn put_object_with_deduplication(
        &self,
        bucket_name: &str,
        key: &str,
        data: Vec<u8>,
        content_type: &str,
        user_metadata: HashMap<String, String>,
        deduplication_mode: DeduplicationMode,
    ) -> Result<Object> {
        let etag = generate_etag(&data);
        
        // 检查是否存在相同内容的其他文件
        let duplicate_keys = self.find_duplicate_content_keys(bucket_name, &etag, Some(key)).await?;
        
        match deduplication_mode {
            DeduplicationMode::Reject => {
                if !duplicate_keys.is_empty() {
                    return Err(anyhow!(
                        "Content already exists with keys: {}. Use different content or enable deduplication.",
                        duplicate_keys.join(", ")
                    ));
                }
                self.put_object(bucket_name, key, data, content_type, user_metadata).await
            },
            DeduplicationMode::Allow => {
                // 允许重复，正常上传
                self.put_object(bucket_name, key, data, content_type, user_metadata).await
            },
            DeduplicationMode::Reference => {
                if !duplicate_keys.is_empty() {
                    // 找到引用计数最高的对象作为数据持有者
                    let mut best_holder_id = None;
                    let mut max_reference_count = 0;
                    
                    for duplicate_key in &duplicate_keys {
                        if let Some(object_id) = self.storage.find_object_id_by_key(bucket_name, duplicate_key).await? {
                            if let Some(metadata) = self.storage.load_object_metadata(bucket_name, &object_id).await? {
                                let current_ref_count = if metadata.data_holder_id.is_none() {
                                    metadata.reference_count
                                } else {
                                    // 如果这个对象指向其他数据持有者，计算间接引用数
                                    if let Some(holder_id) = &metadata.data_holder_id {
                                        if let Some(holder_metadata) = self.storage.load_object_metadata(bucket_name, holder_id).await? {
                                            holder_metadata.reference_count
                                        } else {
                                            0
                                        }
                                    } else {
                                        0
                                    }
                                };
                                
                                if current_ref_count > max_reference_count {
                                    max_reference_count = current_ref_count;
                                    best_holder_id = Some(object_id);
                                }
                            }
                        }
                    }
                    
                    // 如果没有找到合适的数据持有者，选择第一个重复对象
                    let data_holder_id = if let Some(holder_id) = best_holder_id {
                        holder_id
                    } else {
                        let first_key = &duplicate_keys[0];
                        self.storage.find_object_id_by_key(bucket_name, first_key).await?
                            .ok_or_else(|| anyhow!("Duplicate object not found"))?
                    };
                    
                    // 增加数据持有者的引用计数
                    if let Some(mut holder_metadata) = self.storage.load_object_metadata(bucket_name, &data_holder_id).await? {
                        holder_metadata.reference_count += 1;
                        self.storage.save_object_metadata(bucket_name, &data_holder_id, &holder_metadata).await?;
                    }
                    
                    // 创建新对象（指向数据持有者）
                    let new_object = Object::new(
                        key.to_string(),
                        bucket_name.to_string(),
                        data.len() as u64,
                        content_type.to_string(),
                        etag.clone(),
                        user_metadata,
                    );
                    
                    // 生成新对象ID
                    let new_object_id = StorageService::generate_object_id(bucket_name, key);
                    
                    // 保存新对象元数据
                    let mut new_metadata: ObjectMetadata = new_object.clone().into();
                    new_metadata.data_holder_id = Some(data_holder_id.clone());
                    new_metadata.reference_count = 0; // 新对象本身不计算引用计数
                    
                    self.storage.save_object_metadata(bucket_name, &new_object_id, &new_metadata).await?;
                    
                    // 更新索引
                    self.storage.add_object_to_index(bucket_name, key, &new_object_id).await?;
                    self.storage.add_etag_to_index(bucket_name, &etag, &new_object_id).await?;
                    
                    Ok(new_object)
                } else {
                    // 没有重复，正常上传
                    self.put_object(bucket_name, key, data, content_type, user_metadata).await
                }
            }
        }
    }
    
    /// 获取对象的所有版本
    pub async fn list_object_versions(
        &self,
        bucket_name: &str,
        key: &str,
    ) -> Result<Vec<ObjectMetadata>> {
        let all_objects = self.storage.list_object_metadata(bucket_name).await?;
        
        let mut versions: Vec<ObjectMetadata> = all_objects
            .into_iter()
            .filter(|obj| obj.key == key)
            .collect();
        
        // 按创建时间排序（最新的在前）
        versions.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        
        Ok(versions)
    }
    
    /// 获取特定版本的对象
    pub async fn get_object_version(
        &self,
        bucket_name: &str,
        key: &str,
        version_id: &str,
    ) -> Result<(Vec<u8>, ObjectMetadata)> {
        let object_id = format!("{}_{}", StorageService::generate_object_id(bucket_name, key), version_id);
        
        // 加载元数据
        let metadata = self.storage.load_object_metadata(bucket_name, &object_id).await?
            .ok_or_else(|| anyhow!("Object version not found"))?;
        
        // 读取对象数据
        let object_path = self.storage.get_object_data_path(bucket_name, &object_id);
        if !object_path.exists() {
            return Err(anyhow!("Object data not found"));
        }
        
        let data = fs::read(object_path)?;
        
        Ok((data, metadata))
    }
    
    pub async fn get_object(&self, bucket_name: &str, key: &str) -> Result<(Vec<u8>, ObjectMetadata)> {
        // 检查桶是否存在
        let bucket = self.storage.buckets.read().await;
        if !bucket.contains_key(bucket_name) {
            return Err(anyhow!("Bucket '{}' not found", bucket_name));
        }
        drop(bucket);
        
        // 查找对象ID
        let object_id = self.storage.find_object_id_by_key(bucket_name, key).await?
            .ok_or_else(|| anyhow!("Object '{}' not found in bucket '{}'", key, bucket_name))?;
        
        // 加载元数据
        let metadata = self.storage.load_object_metadata(bucket_name, &object_id).await?
            .ok_or_else(|| anyhow!("Object metadata not found"))?;
        
        // 确定数据持有者ID
        let data_object_id = if let Some(holder_id) = &metadata.data_holder_id {
            // 检查数据持有者是否还存在
            if let Some(_holder_metadata) = self.storage.load_object_metadata(bucket_name, holder_id).await? {
                holder_id.clone()
            } else {
                return Err(anyhow!("Data holder for object '{}' not found", key));
            }
        } else {
            // 自己是数据持有者
            object_id
        };
        
        // 读取对象数据
        let object_path = self.storage.get_object_data_path(bucket_name, &data_object_id);
        if !object_path.exists() {
            return Err(anyhow!("Object data not found"));
        }
        
        let data = fs::read(object_path)?;
        
        Ok((data, metadata))
    }
    
    pub async fn delete_object(&self, bucket_name: &str, key: &str) -> Result<()> {
        // 检查桶是否存在
        let bucket = self.storage.buckets.read().await;
        if !bucket.contains_key(bucket_name) {
            return Err(anyhow!("Bucket '{}' not found", bucket_name));
        }
        drop(bucket);
        
        // 查找对象ID
        let object_id = self.storage.find_object_id_by_key(bucket_name, key).await?
            .ok_or_else(|| anyhow!("Object '{}' not found in bucket '{}'", key, bucket_name))?;
        
        // 获取对象元数据
        let metadata = self.storage.load_object_metadata(bucket_name, &object_id).await?
            .ok_or_else(|| anyhow!("Object metadata not found"))?;
        
        if let Some(data_holder_id) = &metadata.data_holder_id {
            // 删除引用对象
            self.storage.delete_object_metadata(bucket_name, &object_id).await?;
            self.storage.remove_object_from_index(bucket_name, key).await?;
            self.storage.remove_etag_from_index(bucket_name, &metadata.etag, &object_id).await?;
            
            // 减少数据持有者的引用计数
            if let Some(mut holder_metadata) = self.storage.load_object_metadata(bucket_name, data_holder_id).await? {
                if holder_metadata.reference_count > 0 {
                    holder_metadata.reference_count -= 1;
                    self.storage.save_object_metadata(bucket_name, data_holder_id, &holder_metadata).await?;
                }
            }
        } else {
            // 自己是数据持有者，检查是否有其他对象引用
            if metadata.reference_count > 0 {
                return Err(anyhow!("Cannot delete object '{}' because it has {} reference(s). Delete all references first.", key, metadata.reference_count));
            }
            
            // 删除对象数据
            let object_path = self.storage.get_object_data_path(bucket_name, &object_id);
            if object_path.exists() {
                fs::remove_file(object_path)?;
            }
            
            // 删除元数据
            self.storage.delete_object_metadata(bucket_name, &object_id).await?;
            
            // 更新索引
            self.storage.remove_object_from_index(bucket_name, key).await?;
            self.storage.remove_etag_from_index(bucket_name, &metadata.etag, &object_id).await?;
        }
        
        Ok(())
    }
    
    pub async fn get_object_metadata(&self, bucket_name: &str, key: &str) -> Result<ObjectMetadata> {
        // 检查桶是否存在
        let bucket = self.storage.buckets.read().await;
        if !bucket.contains_key(bucket_name) {
            return Err(anyhow!("Bucket '{}' not found", bucket_name));
        }
        drop(bucket);
        
        // 查找对象ID
        let object_id = self.storage.find_object_id_by_key(bucket_name, key).await?
            .ok_or_else(|| anyhow!("Object '{}' not found in bucket '{}'", key, bucket_name))?;
        
        self.storage.load_object_metadata(bucket_name, &object_id).await?
            .ok_or_else(|| anyhow!("Object metadata not found"))
    }
    
    pub async fn list_objects(
        &self,
        bucket_name: &str,
        prefix: Option<String>,
        delimiter: Option<String>,
        max_keys: Option<u32>,
        marker: Option<String>,
    ) -> Result<Vec<Object>> {
        // 检查桶是否存在
        let bucket = self.storage.buckets.read().await;
        if !bucket.contains_key(bucket_name) {
            return Err(anyhow!("Bucket '{}' not found", bucket_name));
        }
        drop(bucket);
        
        // 使用分页获取元数据
        let metadata_objects = self.storage.list_object_metadata_with_pagination(
            bucket_name,
            max_keys.map(|k| k as usize),
            marker,
        ).await?;
        
        // 转换为Object列表
        let mut objects: Vec<Object> = metadata_objects.into_iter().map(|obj| Object::new(
            obj.key,
            obj.bucket_name,
            obj.size,
            obj.content_type,
            obj.etag,
            obj.user_metadata,
        )).collect();
        
        // 应用前缀过滤
        if let Some(prefix) = prefix {
            objects.retain(|obj| obj.key.starts_with(&prefix));
        }
        
        // 应用分隔符（简化实现）
        if let Some(delimiter) = delimiter {
            let mut filtered_objects = Vec::new();
            let mut seen_prefixes = std::collections::HashSet::new();
            
            for obj in objects {
                if let Some(pos) = obj.key.find(&delimiter) {
                    let prefix = obj.key[..pos + delimiter.len()].to_string();
                    if !seen_prefixes.contains(&prefix) {
                        seen_prefixes.insert(prefix.clone());
                        // 创建一个虚拟对象来表示公共前缀
                        let virtual_obj = Object::new(
                            prefix,
                            bucket_name.to_string(),
                            0,
                            "application/x-directory".to_string(),
                            "".to_string(),
                            HashMap::new(),
                        );
                        filtered_objects.push(virtual_obj);
                    }
                } else {
                    filtered_objects.push(obj);
                }
            }
            objects = filtered_objects;
        }
        
        Ok(objects)
    }
    
    /// 测试重复文件处理
    pub async fn test_duplicate_handling(
        &self,
        bucket_name: &str,
        key: &str,
        data: Vec<u8>,
        content_type: &str,
        user_metadata: HashMap<String, String>,
    ) -> Result<String> {
        let etag = generate_etag(&data);
        let mut result = String::new();
        
        // 测试1：检查是否重复
        result.push_str(&format!("1. 检查文件是否重复 (ETag: {})\n", etag));
        let is_duplicate = self.is_duplicate_content(bucket_name, key, &etag).await?;
        result.push_str(&format!("   结果: {}\n\n", if is_duplicate { "重复" } else { "不重复" }));
        
        // 测试2：尝试条件上传
        result.push_str("2. 尝试条件上传（如果不存在）\n");
        match self.put_object_if_not_exists(bucket_name, key, data.clone(), content_type, user_metadata.clone()).await {
            Ok(_) => result.push_str("   结果: 上传成功\n\n"),
            Err(e) => result.push_str(&format!("   结果: {}\n\n", e)),
        }
        
        // 测试3：再次检查重复
        result.push_str("3. 再次检查文件是否重复\n");
        let is_duplicate_after = self.is_duplicate_content(bucket_name, key, &etag).await?;
        result.push_str(&format!("   结果: {}\n\n", if is_duplicate_after { "重复" } else { "不重复" }));
        
        // 测试4：尝试上传相同内容
        result.push_str("4. 尝试上传相同内容\n");
        match self.put_object_if_not_exists(bucket_name, key, data, content_type, user_metadata).await {
            Ok(_) => result.push_str("   结果: 上传成功\n\n"),
            Err(e) => result.push_str(&format!("   结果: {}\n\n", e)),
        }
        
        // 测试5：列出所有版本
        result.push_str("5. 列出所有版本\n");
        match self.list_object_versions(bucket_name, key).await {
            Ok(versions) => {
                result.push_str(&format!("   版本数量: {}\n", versions.len()));
                for (i, version) in versions.iter().enumerate() {
                    result.push_str(&format!("   版本 {}: ETag={}, 大小={}, 时间={}\n", 
                        i + 1, 
                        version.etag, 
                        version.size,
                        version.created_at.format("%Y-%m-%d %H:%M:%S")
                    ));
                }
            },
            Err(e) => result.push_str(&format!("   结果: {}\n", e)),
        }
        
        Ok(result)
    }
    
    /// 查找引用某个对象的所有引用对象
    pub async fn find_references_to_object(&self, bucket_name: &str, object_id: &str) -> Result<Vec<ObjectMetadata>> {
        let all_objects = self.storage.list_object_metadata(bucket_name).await?;
        
        let references: Vec<ObjectMetadata> = all_objects
            .into_iter()
            .filter(|obj| obj.data_holder_id.as_ref() == Some(&object_id.to_string()))
            .collect();
        
        Ok(references)
    }
    
    /// 强制删除对象及其所有引用（危险操作）
    pub async fn force_delete_object_with_references(&self, bucket_name: &str, key: &str) -> Result<()> {
        // 查找对象ID
        let object_id = self.storage.find_object_id_by_key(bucket_name, key).await?
            .ok_or_else(|| anyhow!("Object '{}' not found in bucket '{}'", key, bucket_name))?;
        
        // 查找所有引用
        let references = self.find_references_to_object(bucket_name, &object_id).await?;
        
        // 删除所有引用
        for reference in references {
            self.storage.delete_object_metadata(bucket_name, &StorageService::generate_object_id(bucket_name, &reference.key)).await?;
            self.storage.remove_object_from_index(bucket_name, &reference.key).await?;
            self.storage.remove_etag_from_index(bucket_name, &reference.etag, &StorageService::generate_object_id(bucket_name, &reference.key)).await?;
        }
        
        // 删除原始对象
        let object_path = self.storage.get_object_data_path(bucket_name, &object_id);
        if object_path.exists() {
            fs::remove_file(object_path)?;
        }
        
        self.storage.delete_object_metadata(bucket_name, &object_id).await?;
        self.storage.remove_object_from_index(bucket_name, key).await?;
        
        Ok(())
    }
}