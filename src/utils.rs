use sha2::{Sha256, Digest};
use std::path::Path;

/// 计算SHA256哈希
pub fn sha256_hash(data: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(data);
    format!("{:x}", hasher.finalize())
}

/// 计算MD5哈希
pub fn md5_hash(data: &[u8]) -> String {
    let digest = md5::compute(data);
    format!("{:x}", digest)
}

/// 清理路径，防止路径遍历攻击
pub fn sanitize_path(path: &str) -> String {
    path_clean::clean(path)
}

/// 验证桶名称
pub fn validate_bucket_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Bucket name cannot be empty".to_string());
    }
    
    if name.len() > 63 {
        return Err("Bucket name cannot be longer than 63 characters".to_string());
    }
    
    if !name.chars().all(|c| c.is_alphanumeric() || c == '-') {
        return Err("Bucket name can only contain alphanumeric characters and hyphens".to_string());
    }
    
    if name.starts_with('-') || name.ends_with('-') {
        return Err("Bucket name cannot start or end with a hyphen".to_string());
    }
    
    if name.chars().next().unwrap().is_numeric() {
        return Err("Bucket name cannot start with a number".to_string());
    }
    
    Ok(())
}

/// 验证对象键
pub fn validate_object_key(key: &str) -> Result<(), String> {
    if key.is_empty() {
        return Err("Object key cannot be empty".to_string());
    }
    
    if key.len() > 1024 {
        return Err("Object key cannot be longer than 1024 characters".to_string());
    }
    
    if key.contains("..") {
        return Err("Object key cannot contain '..'".to_string());
    }
    
    Ok(())
}

/// 生成ETag
pub fn generate_etag(data: &[u8]) -> String {
    format!("\"{}\"", md5_hash(data))
}

/// 获取文件扩展名对应的MIME类型
pub fn get_mime_type(filename: &str) -> String {
    let ext = Path::new(filename)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or("")
        .to_lowercase();
    
    match ext.as_str() {
        "html" | "htm" => "text/html",
        "css" => "text/css",
        "js" => "application/javascript",
        "json" => "application/json",
        "xml" => "application/xml",
        "txt" => "text/plain",
        "pdf" => "application/pdf",
        "jpg" | "jpeg" => "image/jpeg",
        "png" => "image/png",
        "gif" => "image/gif",
        "svg" => "image/svg+xml",
        "ico" => "image/x-icon",
        "zip" => "application/zip",
        "tar" => "application/x-tar",
        "gz" => "application/gzip",
        "mp4" => "video/mp4",
        "mp3" => "audio/mpeg",
        "wav" => "audio/wav",
        _ => "application/octet-stream",
    }.to_string()
}

/// 验证ETag格式
pub fn is_valid_etag_format(etag: &str) -> bool {
    // 支持以下格式：
    // 1. "hash" - 标准ETag
    // 2. W/"weak-etag" - 弱ETag
    // 3. "sha256-hash" - SHA256格式
    // 4. "version-id" - 版本标识
    
    if etag.is_empty() {
        return false;
    }
    
    // 弱ETag格式: W/"weak-etag"
    if etag.starts_with("W/\"") && etag.ends_with("\"") {
        return true;
    }
    
    // 标准ETag格式: "hash"
    if etag.starts_with("\"") && etag.ends_with("\"") {
        return true;
    }
    
    // 无引号格式（向后兼容）
    if !etag.contains('"') && !etag.contains(' ') {
        return true;
    }
    
    false
} 