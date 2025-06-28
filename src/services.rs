use crate::models::{Bucket, Object, ObjectMetadata};
use crate::utils::{validate_bucket_name, validate_object_key, generate_etag, get_mime_type, sanitize_path, sha256_hash};
use anyhow::{Result, anyhow};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use std::sync::Arc;
use tokio::sync::RwLock;
use serde_json;

/// 存储服务 - 参考MinIO的存储结构
#[derive(Clone)]
pub struct StorageService {
    data_dir: PathBuf,
    buckets: Arc<RwLock<HashMap<String, Bucket>>>,
    object_index: Arc<RwLock<HashMap<String, HashMap<String, String>>>>,
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
        
        Ok(Self {
            data_dir: data_path,
            buckets: Arc::new(RwLock::new(buckets)),
            object_index: Arc::new(RwLock::new(object_index)),
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
    
    /// 生成对象ID（类似MinIO的哈希化文件名）
    fn generate_object_id(bucket_name: &str, key: &str) -> String {
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
        
        let object = Object::new(
            key.to_string(),
            bucket_name.to_string(),
            data.len() as u64,
            mime_type,
            etag,
            user_metadata,
        );
        
        // 生成对象ID
        let object_id = StorageService::generate_object_id(bucket_name, key);
        
        // 保存对象数据（使用哈希化文件名）
        let object_path = self.storage.get_object_data_path(bucket_name, &object_id);
        if let Some(parent) = object_path.parent() {
            if !parent.exists() {
                fs::create_dir_all(parent)?;
            }
        }
        fs::write(&object_path, data)?;
        
        // 保存元数据
        let metadata: ObjectMetadata = object.clone().into();
        self.storage.save_object_metadata(bucket_name, &object_id, &metadata).await?;
        
        // 更新索引
        self.storage.add_object_to_index(bucket_name, key, &object_id).await?;
        
        Ok(object)
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
        
        // 读取对象数据
        let object_path = self.storage.get_object_data_path(bucket_name, &object_id);
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
        
        // 删除对象数据
        let object_path = self.storage.get_object_data_path(bucket_name, &object_id);
        if object_path.exists() {
            fs::remove_file(object_path)?;
        }
        
        // 删除元数据
        self.storage.delete_object_metadata(bucket_name, &object_id).await?;
        
        // 更新索引
        self.storage.remove_object_from_index(bucket_name, key).await?;
        
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
}