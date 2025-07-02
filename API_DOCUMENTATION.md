# Sevino 对象存储服务 API 文档

## 📋 目录

- [概述](#概述)
- [基础信息](#基础信息)
- [认证](#认证)
- [CORS 配置](#cors-配置)
- [通用响应格式](#通用响应格式)
- [错误码](#错误码)
- [API 端点](#api-端点)
  - [健康检查](#健康检查)
  - [桶管理](#桶管理)
  - [对象管理](#对象管理)
  - [测试接口](#测试接口)
- [数据模型](#数据模型)
- [示例代码](#示例代码)
- [最佳实践](#最佳实践)

## 概述

Sevino 是一个基于 Rust 开发的高性能对象存储服务，提供去中心化的重复数据删除功能。本文档详细描述了所有可用的 API 接口。

### 主要特性

- **对象存储**: 完整的 CRUD 操作支持
- **去中心化去重**: 智能的重复数据删除机制
- **版本控制**: 对象版本管理
- **高性能索引**: 内存索引实现 O(1) 查找
- **RESTful API**: 标准 REST 接口
- **Swagger UI**: 内置 API 文档和测试界面
- **CORS 支持**: 完整的跨域资源共享支持
- **分片上传**: 支持大文件分片上传

## 基础信息

- **服务地址**: `http://127.0.0.1:8000`
- **API 基础路径**: `/api`
- **文档地址**: `http://127.0.0.1:8000/swagger-ui/`
- **内容类型**: `application/json`
- **字符编码**: UTF-8

## 认证

当前版本暂不支持认证，所有接口均为公开访问。

## CORS 配置

Sevino 支持完整的跨域资源共享(CORS)功能，允许从不同域名的前端应用访问API。

### 环境变量配置

```bash
# 启用CORS
SEVINO_ENABLE_CORS=true

# 允许的域名（逗号分隔）
SEVINO_CORS_ORIGINS=http://localhost:3000,http://127.0.0.1:3000,http://localhost:8080,http://127.0.0.1:8080,*

# 允许的HTTP方法（逗号分隔）
SEVINO_CORS_METHODS=GET,POST,PUT,DELETE,OPTIONS

# 允许的请求头（逗号分隔）
SEVINO_CORS_HEADERS=Content-Type,Authorization,X-Requested-With,Accept,Origin

# 是否允许发送凭据（cookies等）
SEVINO_CORS_ALLOW_CREDENTIALS=false
```

### 默认配置

如果不设置环境变量，将使用以下默认配置：

- **允许的域名**: `http://localhost:3000`, `http://127.0.0.1:3000`, `http://localhost:8080`, `http://127.0.0.1:8080`, `*`
- **允许的方法**: `GET`, `POST`, `PUT`, `DELETE`, `OPTIONS`
- **允许的头部**: `Content-Type`, `Authorization`, `X-Requested-With`, `Accept`, `Origin`
- **允许凭据**: `false`

### 生产环境配置

在生产环境中，建议使用更严格的CORS配置：

```bash
# 只允许特定域名
SEVINO_CORS_ORIGINS=https://yourdomain.com,https://app.yourdomain.com

# 只允许必要的HTTP方法
SEVINO_CORS_METHODS=GET,POST,PUT,DELETE

# 只允许必要的请求头
SEVINO_CORS_HEADERS=Content-Type,Authorization

# 如果需要发送凭据
SEVINO_CORS_ALLOW_CREDENTIALS=true
```

### CORS 响应头

当CORS启用时，服务器会在响应中包含以下头部：

```
Access-Control-Allow-Origin: [配置的域名或*]
Access-Control-Allow-Methods: [配置的方法]
Access-Control-Allow-Headers: [配置的头部]
Access-Control-Max-Age: 3600
Access-Control-Allow-Credentials: [true/false]
```

### 预检请求

对于复杂请求（如包含自定义头部的POST请求），浏览器会发送OPTIONS预检请求。Sevino会自动处理这些请求并返回适当的CORS头部。

### 测试CORS

可以使用提供的 `cors_test.html` 文件来测试CORS功能：

1. 启动Sevino服务
2. 在浏览器中打开 `cors_test.html`
3. 输入服务地址并测试各种API调用

## 通用响应格式

所有 API 响应都遵循统一的格式：

```json
{
  "success": true,
  "data": {
    // 具体数据内容
  },
  "error": null
}
```

### 成功响应

```json
{
  "success": true,
  "data": {
    "id": "bucket-123",
    "name": "my-bucket",
    "created_at": "2024-01-01T00:00:00Z"
  },
  "error": null
}
```

### 错误响应

```json
{
  "success": false,
  "data": null,
  "error": "Bucket not found"
}
```

## 错误码

| HTTP 状态码 | 说明 | 描述 |
|-------------|------|------|
| 200 | OK | 请求成功 |
| 400 | Bad Request | 请求参数错误 |
| 404 | Not Found | 资源不存在 |
| 409 | Conflict | 资源冲突（如桶已存在） |
| 500 | Internal Server Error | 服务器内部错误 |

## API 端点

### 健康检查

#### 获取服务根路径

```http
GET /
```

**描述**: 获取服务欢迎信息

**响应**:
```json
"Welcome to Sevino Object Storage Service!"
```

**示例**:
```bash
curl -X GET "http://127.0.0.1:8000/"
```

#### 健康检查

```http
GET /health
```

**描述**: 检查服务健康状态

**响应**:
```json
{
  "status": "healthy",
  "timestamp": "2024-01-01T00:00:00Z"
}
```

**示例**:
```bash
curl -X GET "http://127.0.0.1:8000/health"
```

### 桶管理

#### 列出所有桶

```http
GET /api/buckets
```

**描述**: 获取所有桶的列表

**查询参数**: 无

**响应**:
```json
{
  "success": true,
  "data": {
    "buckets": [
      {
        "id": "bucket-123",
        "name": "my-bucket",
        "created_at": "2024-01-01T00:00:00Z",
        "object_count": 10,
        "total_size": 1024000
      }
    ]
  },
  "error": null
}
```

**示例**:
```bash
curl -X GET "http://127.0.0.1:8000/api/buckets"
```

#### 创建桶

```http
POST /api/buckets
```

**描述**: 创建新的存储桶

**请求体**:
```json
{
  "name": "my-bucket"
}
```

**参数说明**:
- `name` (string, 必需): 桶名称，必须唯一

**响应**:
```json
{
  "success": true,
  "data": {
    "id": "bucket-123",
    "name": "my-bucket",
    "created_at": "2024-01-01T00:00:00Z",
    "object_count": 0,
    "total_size": 0
  },
  "error": null
}
```

**示例**:
```bash
curl -X POST "http://127.0.0.1:8000/api/buckets" \
  -H "Content-Type: application/json" \
  -d '{"name": "my-bucket"}'
```

#### 获取桶信息

```http
GET /api/buckets/{name}
```

**描述**: 获取指定桶的详细信息

**路径参数**:
- `name` (string, 必需): 桶名称

**响应**:
```json
{
  "success": true,
  "data": {
    "id": "bucket-123",
    "name": "my-bucket",
    "created_at": "2024-01-01T00:00:00Z",
    "object_count": 10,
    "total_size": 1024000
  },
  "error": null
}
```

**示例**:
```bash
curl -X GET "http://127.0.0.1:8000/api/buckets/my-bucket"
```

#### 删除桶

```http
DELETE /api/buckets/{name}
```

**描述**: 删除指定的桶及其所有对象

**路径参数**:
- `name` (string, 必需): 桶名称

**响应**:
```json
{
  "success": true,
  "data": null,
  "error": null
}
```

**示例**:
```bash
curl -X DELETE "http://127.0.0.1:8000/api/buckets/my-bucket"
```

### 对象管理

#### 列出对象

```http
GET /api/buckets/{bucket_name}/objects
```

**描述**: 获取指定桶中的所有对象列表

**路径参数**:
- `bucket_name` (string, 必需): 桶名称

**查询参数**:
- `prefix` (string, 可选): 对象键前缀过滤
- `delimiter` (string, 可选): 分隔符
- `max_keys` (integer, 可选): 最大返回数量，默认1000
- `marker` (string, 可选): 分页标记
- `etag_filter` (string, 可选): ETag过滤，支持通配符
- `custom_xxx` (string, 可选): 按自定义元数据过滤，如 `custom_bizid=123`

**自定义元数据过滤说明**:
- 通过在查询参数中添加 `custom_标签名=值`，可以筛选 user_metadata 里对应键值的对象。例如 `custom_bizid=123` 只返回 user_metadata 里 `bizid=123` 的对象。
- 支持多个 custom_xxx 组合过滤（AND关系），如 `custom_bizid=123&custom_tag=abc` 会筛选出同时满足 bizid=123 且 tag=abc 的对象。
- 仅支持字符串类型的 user_metadata 字段。
- 如果 user_metadata 中没有该字段，或值不等于指定值，则不会返回该对象。

**示例**:
```bash
# 按自定义标签 bizid 过滤
curl -X GET "http://127.0.0.1:8000/api/buckets/my-bucket/objects?custom_bizid=123"

# 按多个自定义标签组合过滤
curl -X GET "http://127.0.0.1:8000/api/buckets/my-bucket/objects?custom_bizid=123&custom_tag=abc"
```

#### 上传对象

```http
PUT /api/buckets/{bucket_name}/objects/{key}
```

**描述**: 上传对象到指定桶

**路径参数**:
- `bucket_name` (string, 必需): 桶名称
- `key` (string, 必需): 对象键

**查询参数**:
- `deduplication_mode` (string, 可选): 去重模式，可选值：`reject`, `allow`, `reference`
- `content_type` (string, 可选): 内容类型，默认为 `application/octet-stream`
- `custom` (string, 可选): 自定义元数据，json字符串，内容会合并到user_metadata

**自定义元数据示例**:
- `custom={"bizid":"123","tag":"abc"}`

**去重模式说明**:
- `reject`: 拒绝重复内容，如果检测到相同内容则返回错误
- `allow`: 允许重复内容，正常上传（默认模式）
- `reference`: 创建引用，如果检测到相同内容则创建引用而不是重复存储

**响应**:
```json
{
  "success": true,
  "data": {
    "id": "obj-123",
    "key": "example.txt",
    "bucket_name": "my-bucket",
    "size": 1024,
    "etag": "\"d41d8cd98f00b204e9800998ecf8427e\"",
    "content_type": "text/plain",
    "created_at": "2024-01-01T00:00:00Z",
    "last_modified": "2024-01-01T00:00:00Z",
    "user_metadata": { "bizid": "123", "tag": "abc" }
  },
  "error": null
}
```

**示例**:
```bash
# 基本上传
curl -X PUT "http://127.0.0.1:8000/api/buckets/my-bucket/objects/example.txt" \
  -H "Content-Type: text/plain" \
  --data-binary "Hello, World!"

# 添加自定义元数据
curl -X PUT "http://127.0.0.1:8000/api/buckets/my-bucket/objects/example.txt?custom={\"bizid\":\"123\",\"tag\":\"abc\"}" \
  -H "Content-Type: text/plain" \
  --data-binary "Hello, World!"
```

#### 修改对象元数据

```http
PUT /api/buckets/{bucket_name}/objects/{key}/metadata
Content-Type: application/json
```

**描述**：只修改对象的元数据（如 content_type、user_metadata、ETag），不影响对象内容。

**路径参数**:
- `bucket_name` (string, 必需): 桶名称
- `key` (string, 必需): 对象键

**请求体**（application/json，可选字段只传需要修改的）:
| 字段名         | 类型                | 说明                                 |
|----------------|---------------------|--------------------------------------|
| content_type   | string (可选)       | 新的内容类型                         |
| user_metadata  | object (可选)       | 新的自定义元数据（键值对）           |
| custom_etag    | string (可选)       | 新的自定义ETag，需符合ETag格式要求   |

**示例**:
```bash
curl -X PUT "http://127.0.0.1:8000/api/buckets/my-bucket/objects/example.txt/metadata" \
  -H "Content-Type: application/json" \
  -d '{"content_type":"text/plain","user_metadata":{"tag":"abc"},"custom_etag":"\"my-custom-etag\""}'
```

**响应**:
```json
{
  "success": true,
  "data": {
    "id": "obj-123",
    "key": "example.txt",
    "bucket_name": "my-bucket",
    "size": 1024,
    "etag": "\"my-custom-etag\"",
    "content_type": "text/plain",
    "created_at": "2024-01-01T00:00:00Z",
    "last_modified": "2024-01-01T00:00:00Z",
    "user_metadata": { "tag": "abc" }
  },
  "error": null
}
```

**注意事项**：
- 只会修改元数据，不会影响对象内容。
- custom_etag 必须符合ETag格式，否则返回400。
- user_metadata 只支持字符串类型的键值对。

#### 分片上传

```http
PUT /api/buckets/{bucket_name}/objects/{key}/multipart
```

**描述**: 分片上传大文件

**路径参数**:
- `bucket_name` (string, 必需): 桶名称
- `key` (string, 必需): 对象键

**查询参数**:
- `part_number` (integer, 必需): 分片编号，从1开始
- `total_parts` (integer, 必需): 总分片数
- `upload_id` (string, 必需): 上传ID，用于标识同一个文件的分片
- `content_type` (string, 可选): 内容类型

**请求体**: 分片的二进制数据

**响应**:
```json
{
  "success": true,
  "data": {
    "id": "obj-123",
    "key": "large-file.part.1",
    "bucket_name": "my-bucket",
    "size": 5242880,
    "etag": "d41d8cd98f00b204e9800998ecf8427e",
    "content_type": "application/octet-stream",
    "created_at": "2024-01-01T00:00:00Z",
    "last_modified": "2024-01-01T00:00:00Z",
    "user_metadata": {
      "multipart_upload_id": "upload-123",
      "part_number": "1",
      "total_parts": "3"
    }
  },
  "error": null
}
```

**示例**:
```bash
# 上传第一个分片
curl -X PUT "http://127.0.0.1:8000/api/buckets/my-bucket/objects/large-file/multipart?part_number=1&total_parts=3&upload_id=upload-123" \
  -H "Content-Type: application/octet-stream" \
  --data-binary @part1.bin

# 上传第二个分片
curl -X PUT "http://127.0.0.1:8000/api/buckets/my-bucket/objects/large-file/multipart?part_number=2&total_parts=3&upload_id=upload-123" \
  -H "Content-Type: application/octet-stream" \
  --data-binary @part2.bin
```

**分片上传流程**:
1. 生成唯一的 `upload_id`
2. 将大文件分割成多个分片
3. 逐个上传分片，每个分片使用相同的 `upload_id`
4. 所有分片上传完成后，可以合并分片或直接使用分片文件

#### 下载对象

```http
GET /api/buckets/{bucket_name}/objects/{key}
```

**描述**: 下载指定对象

**路径参数**:
- `bucket_name` (string, 必需): 桶名称
- `key` (string, 必需): 对象键

**响应头**:
- `Content-Type`: 对象的内容类型
- `ETag`: 对象的ETag
- `Content-Length`: 对象大小

**响应体**: 对象的二进制数据

**示例**:
```bash
curl -X GET "http://127.0.0.1:8000/api/buckets/my-bucket/objects/example.txt"
```

#### 删除对象

```http
DELETE /api/buckets/{bucket_name}/objects/{key}
```

**描述**: 删除指定对象

**路径参数**:
- `bucket_name` (string, 必需): 桶名称
- `key` (string, 必需): 对象键

**响应**:
```json
{
  "success": true,
  "data": null,
  "error": null
}
```

**示例**:
```bash
curl -X DELETE "http://127.0.0.1:8000/api/buckets/my-bucket/objects/example.txt"
```

#### 获取对象元数据

```http
GET /api/buckets/{bucket_name}/objects/{key}/metadata
```

**描述**: 获取指定对象的元数据信息

**路径参数**:
- `bucket_name` (string, 必需): 桶名称
- `key` (string, 必需): 对象键

**响应**:
```json
{
  "success": true,
  "data": {
    "id": "obj-123",
    "key": "example.txt",
    "bucket_name": "my-bucket",
    "size": 1024,
    "etag": "d41d8cd98f00b204e9800998ecf8427e",
    "content_type": "text/plain",
    "created_at": "2024-01-01T00:00:00Z",
    "last_modified": "2024-01-01T00:00:00Z",
    "user_metadata": {
      "author": "user1"
    },
    "data_holder_id": null,
    "reference_count": 0,
    "version_id": "v1"
  },
  "error": null
}
```

**示例**:
```bash
curl -X GET "http://127.0.0.1:8000/api/buckets/my-bucket/objects/example.txt/metadata"
```

#### 列出对象版本

```http
GET /api/buckets/{bucket_name}/objects/{key}/versions
```

**描述**: 获取指定对象的所有版本

**路径参数**:
- `bucket_name` (string, 必需): 桶名称
- `key` (string, 必需): 对象键

**响应**:
```json
{
  "success": true,
  "data": [
    {
      "id": "obj-123",
      "key": "example.txt",
      "bucket_name": "my-bucket",
      "size": 1024,
      "etag": "d41d8cd98f00b204e9800998ecf8427e",
      "content_type": "text/plain",
      "created_at": "2024-01-01T00:00:00Z",
      "last_modified": "2024-01-01T00:00:00Z",
      "user_metadata": {},
      "data_holder_id": null,
      "reference_count": 0,
      "version_id": "v1"
    }
  ],
  "error": null
}
```

**示例**:
```bash
curl -X GET "http://127.0.0.1:8000/api/buckets/my-bucket/objects/example.txt/versions"
```

#### 测试重复数据处理

```http
POST /api/buckets/{bucket_name}/objects/{key}/duplicate-test
```

**描述**: 测试去中心化Reference模式的重复数据处理

**路径参数**:
- `bucket_name` (string, 必需): 桶名称
- `key` (string, 必需): 对象键

**请求体**: 二进制数据

**响应**:
```json
{
  "success": true,
  "data": "=== 去中心化Reference模式测试 ===\n\n1. 创建测试桶\n   ✓ 桶创建成功\n\n2. 上传第一个文件 (key: file1.txt)\n   ✓ 文件上传成功\n   - ETag: d41d8cd98f00b204e9800998ecf8427e\n   - 大小: 64 bytes\n   - 对象ID: test-reference-bucket-v2/file1.txt\n\n...",
  "error": null
}
```

**示例**:
```bash
curl -X POST "http://127.0.0.1:8000/api/buckets/my-bucket/objects/test.txt/duplicate-test" \
  -H "Content-Type: text/plain" \
  --data-binary "Hello, this is test content!"
```

### 测试接口

#### Reference模式测试

```http
GET /api/test/reference-mode
```

**描述**: 运行完整的Reference模式测试，验证去中心化去重功能

**响应**:
```json
{
  "success": true,
  "data": "=== 去中心化Reference模式测试 ===\n\n1. 创建测试桶\n   ✓ 桶创建成功\n\n2. 上传第一个文件 (key: file1.txt)\n   ✓ 文件上传成功\n   - ETag: d41d8cd98f00b204e9800998ecf8427e\n   - 大小: 64 bytes\n   - 对象ID: test-reference-bucket-v2/file1.txt\n\n3. 使用Reference模式上传相同内容 (key: file2.txt)\n   ✓ 引用创建成功\n   - ETag: d41d8cd98f00b204e9800998ecf8427e\n   - 大小: 64 bytes\n   - 对象ID: test-reference-bucket-v2/file2.txt\n   - 数据持有者ID: Some(\"test-reference-bucket-v2/file1.txt\")\n   - 引用计数: 0\n\n...",
  "error": null
}
```

**示例**:
```bash
curl -X GET "http://127.0.0.1:8000/api/test/reference-mode"
```

## 数据模型

### Bucket（桶）

```json
{
  "id": "string",
  "name": "string",
  "created_at": "string (ISO 8601)",
  "object_count": "integer",
  "total_size": "integer"
}
```

**字段说明**:
- `id`: 桶的唯一标识符
- `name`: 桶名称
- `created_at`: 创建时间（ISO 8601格式）
- `object_count`: 对象数量
- `total_size`: 总大小（字节）

### Object（对象）

```json
{
  "id": "string",
  "key": "string",
  "bucket_name": "string",
  "size": "integer",
  "etag": "string",
  "content_type": "string",
  "created_at": "string (ISO 8601)",
  "last_modified": "string (ISO 8601)",
  "user_metadata": "object"
}
```

**字段说明**:
- `id`: 对象的唯一标识符
- `key`: 对象键
- `bucket_name`: 所属桶名称
- `size`: 对象大小（字节）
- `etag`: 对象的ETag（MD5哈希）
- `content_type`: 内容类型
- `created_at`: 创建时间
- `last_modified`: 最后修改时间
- `user_metadata`: 用户自定义元数据

### ObjectMetadata（对象元数据）

```json
{
  "id": "string",
  "key": "string",
  "bucket_name": "string",
  "size": "integer",
  "etag": "string",
  "content_type": "string",
  "created_at": "string (ISO 8601)",
  "last_modified": "string (ISO 8601)",
  "user_metadata": "object",
  "data_holder_id": "string|null",
  "reference_count": "integer",
  "version_id": "string"
}
```

**字段说明**:
- 包含Object的所有字段
- `data_holder_id`: 数据持有者ID（null表示自己是数据持有者）
- `reference_count`: 引用计数
- `version_id`: 版本ID

## 示例代码

### JavaScript/Node.js

```javascript
const axios = require('axios');

const API_BASE = 'http://127.0.0.1:8000/api';

// 创建桶
async function createBucket(name) {
  try {
    const response = await axios.post(`${API_BASE}/buckets`, { name });
    return response.data;
  } catch (error) {
    console.error('创建桶失败:', error.response.data);
  }
}

// 上传对象（基本模式）
async function uploadObject(bucketName, key, data, contentType = 'application/octet-stream') {
  try {
    const response = await axios.put(
      `${API_BASE}/buckets/${bucketName}/objects/${key}`,
      data,
      {
        headers: { 'Content-Type': contentType }
      }
    );
    return response.data;
  } catch (error) {
    console.error('上传对象失败:', error.response.data);
  }
}

// 上传对象（带去重模式）
async function uploadObjectWithDeduplication(bucketName, key, data, dedupMode, contentType = 'application/octet-stream') {
  try {
    const response = await axios.put(
      `${API_BASE}/buckets/${bucketName}/objects/${key}?deduplication_mode=${dedupMode}`,
      data,
      {
        headers: { 'Content-Type': contentType }
      }
    );
    return response.data;
  } catch (error) {
    console.error('上传对象失败:', error.response.data);
  }
}

// 分片上传
async function uploadMultipart(bucketName, key, data, partNumber, totalParts, uploadId, contentType = 'application/octet-stream') {
  try {
    const response = await axios.put(
      `${API_BASE}/buckets/${bucketName}/objects/${key}/multipart`,
      data,
      {
        headers: { 'Content-Type': 'application/json' },
        data: {
          part_number: partNumber,
          total_parts: totalParts,
          upload_id: uploadId,
          content_type: contentType
        }
      }
    );
    return response.data;
  } catch (error) {
    console.error('分片上传失败:', error.response.data);
  }
}

// 下载对象
async function downloadObject(bucketName, key) {
  try {
    const response = await axios.get(
      `${API_BASE}/buckets/${bucketName}/objects/${key}`,
      { responseType: 'arraybuffer' }
    );
    return response.data;
  } catch (error) {
    console.error('下载对象失败:', error.response.data);
  }
}

// 使用示例
async function example() {
  // 创建桶
  await createBucket('my-bucket');
  
  // 基本上传
  const fileData = Buffer.from('Hello, World!');
  await uploadObject('my-bucket', 'hello.txt', fileData, 'text/plain');
  
  // 带去重模式上传
  await uploadObjectWithDeduplication('my-bucket', 'hello2.txt', fileData, 'reference', 'text/plain');
  
  // 分片上传大文件
  const largeFile = Buffer.alloc(10 * 1024 * 1024); // 10MB
  const uploadId = 'upload-' + Date.now();
  await uploadMultipart('my-bucket', 'large-file', largeFile.slice(0, 5 * 1024 * 1024), 1, 2, uploadId);
  await uploadMultipart('my-bucket', 'large-file', largeFile.slice(5 * 1024 * 1024), 2, 2, uploadId);
  
  // 下载文件
  const downloadedData = await downloadObject('my-bucket', 'hello.txt');
  console.log('下载的内容:', downloadedData.toString());
}
```

### Python

```python
import requests
import json

API_BASE = 'http://127.0.0.1:8000/api'

def create_bucket(name):
    """创建桶"""
    try:
        response = requests.post(f'{API_BASE}/buckets', json={'name': name})
        response.raise_for_status()
        return response.json()
    except requests.exceptions.RequestException as e:
        print(f'创建桶失败: {e}')
        return None

def upload_object(bucket_name, key, data, content_type='application/octet-stream', dedup_mode=None):
    """上传对象"""
    try:
        url = f'{API_BASE}/buckets/{bucket_name}/objects/{key}'
        params = {}
        if dedup_mode:
            params['deduplication_mode'] = dedup_mode
        if content_type != 'application/octet-stream':
            params['content_type'] = content_type
            
        response = requests.put(url, data=data, params=params)
        response.raise_for_status()
        return response.json()
    except requests.exceptions.RequestException as e:
        print(f'上传对象失败: {e}')
        return None

def upload_multipart(bucket_name, key, data, part_number, total_parts, upload_id, content_type='application/octet-stream'):
    """分片上传"""
    try:
        url = f'{API_BASE}/buckets/{bucket_name}/objects/{key}/multipart'
        headers = {'Content-Type': 'application/json'}
        payload = {
            'part_number': part_number,
            'total_parts': total_parts,
            'upload_id': upload_id,
            'content_type': content_type
        }
        
        response = requests.put(url, data=data, headers=headers, json=payload)
        response.raise_for_status()
        return response.json()
    except requests.exceptions.RequestException as e:
        print(f'分片上传失败: {e}')
        return None

def download_object(bucket_name, key):
    """下载对象"""
    try:
        response = requests.get(f'{API_BASE}/buckets/{bucket_name}/objects/{key}')
        response.raise_for_status()
        return response.content
    except requests.exceptions.RequestException as e:
        print(f'下载对象失败: {e}')
        return None

def list_objects(bucket_name):
    """列出对象"""
    try:
        response = requests.get(f'{API_BASE}/buckets/{bucket_name}/objects')
        response.raise_for_status()
        return response.json()
    except requests.exceptions.RequestException as e:
        print(f'列出对象失败: {e}')
        return None

# 使用示例
if __name__ == '__main__':
    # 创建桶
    create_bucket('my-bucket')
    
    # 基本上传
    file_data = b'Hello, World!'
    upload_object('my-bucket', 'hello.txt', file_data, 'text/plain')
    
    # 带去重模式上传
    upload_object('my-bucket', 'hello2.txt', file_data, 'text/plain', 'reference')
    
    # 分片上传大文件
    large_file = b'x' * (10 * 1024 * 1024)  # 10MB
    upload_id = f'upload-{int(time.time())}'
    upload_multipart('my-bucket', 'large-file', large_file[:5*1024*1024], 1, 2, upload_id)
    upload_multipart('my-bucket', 'large-file', large_file[5*1024*1024:], 2, 2, upload_id)
    
    # 列出对象
    objects = list_objects('my-bucket')
    print('对象列表:', json.dumps(objects, indent=2))
    
    # 下载文件
    downloaded_data = download_object('my-bucket', 'hello.txt')
    print('下载的内容:', downloaded_data.decode())
```

### cURL 示例

```bash
#!/bin/bash

API_BASE="http://127.0.0.1:8000/api"

# 创建桶
echo "创建桶..."
curl -X POST "$API_BASE/buckets" \
  -H "Content-Type: application/json" \
  -d '{"name": "test-bucket"}' | jq

# 基本上传
echo "基本上传..."
curl -X PUT "$API_BASE/buckets/test-bucket/objects/example.txt" \
  -H "Content-Type: text/plain" \
  --data-binary "Hello, World!" | jq

# 带去重模式上传
echo "带去重模式上传..."
curl -X PUT "$API_BASE/buckets/test-bucket/objects/example2.txt?deduplication_mode=reference" \
  -H "Content-Type: text/plain" \
  --data-binary "Hello, World!" | jq

# 分片上传
echo "分片上传..."
UPLOAD_ID="upload-$(date +%s)"
curl -X PUT "$API_BASE/buckets/test-bucket/objects/large-file/multipart" \
  -H "Content-Type: application/json" \
  -d "{\"part_number\": 1, \"total_parts\": 2, \"upload_id\": \"$UPLOAD_ID\"}" \
  --data-binary "Part 1 data" | jq

curl -X PUT "$API_BASE/buckets/test-bucket/objects/large-file/multipart" \
  -H "Content-Type: application/json" \
  -d "{\"part_number\": 2, \"total_parts\": 2, \"upload_id\": \"$UPLOAD_ID\"}" \
  --data-binary "Part 2 data" | jq

# 列出对象
echo "列出对象..."
curl -X GET "$API_BASE/buckets/test-bucket/objects" | jq

# 获取对象元数据
echo "获取对象元数据..."
curl -X GET "$API_BASE/buckets/test-bucket/objects/example.txt/metadata" | jq

# 下载文件
echo "下载文件..."
curl -X GET "$API_BASE/buckets/test-bucket/objects/example.txt"

# 清理
echo "清理..."
curl -X DELETE "$API_BASE/buckets/test-bucket/objects/example.txt" | jq
curl -X DELETE "$API_BASE/buckets/test-bucket" | jq
```

## 最佳实践

### 1. 去重模式选择

```javascript
// 根据业务需求选择合适的去重模式
const dedupModes = {
  // 拒绝重复内容，适用于需要确保唯一性的场景
  REJECT: 'reject',
  
  // 允许重复内容，适用于一般上传场景
  ALLOW: 'allow',
  
  // 创建引用，适用于节省存储空间的场景
  REFERENCE: 'reference'
};

// 示例：上传配置文件，拒绝重复
await uploadObjectWithDeduplication('config-bucket', 'app.conf', configData, dedupModes.REJECT);

// 示例：上传用户头像，允许重复
await uploadObjectWithDeduplication('avatar-bucket', 'user1.jpg', avatarData, dedupModes.ALLOW);

// 示例：上传备份文件，使用引用模式节省空间
await uploadObjectWithDeduplication('backup-bucket', 'backup1.zip', backupData, dedupModes.REFERENCE);
```

### 2. 分片上传优化

```javascript
// 分片上传大文件
async function uploadLargeFile(bucketName, key, file, chunkSize = 5 * 1024 * 1024) {
  const uploadId = `upload-${Date.now()}`;
  const totalChunks = Math.ceil(file.size / chunkSize);
  const uploadPromises = [];
  
  for (let i = 0; i < totalChunks; i++) {
    const start = i * chunkSize;
    const end = Math.min(start + chunkSize, file.size);
    const chunk = file.slice(start, end);
    
    const promise = uploadMultipart(
      bucketName, 
      key, 
      chunk, 
      i + 1, 
      totalChunks, 
      uploadId
    );
    uploadPromises.push(promise);
  }
  
  // 并行上传所有分片
  const results = await Promise.all(uploadPromises);
  console.log(`文件 ${key} 分片上传完成，共 ${totalChunks} 个分片`);
  
  return results;
}

// 使用示例
const fileInput = document.getElementById('fileInput');
fileInput.addEventListener('change', async (event) => {
  const file = event.target.files[0];
  if (file.size > 10 * 1024 * 1024) { // 大于10MB使用分片上传
    await uploadLargeFile('my-bucket', file.name, file);
  } else {
    await uploadObject('my-bucket', file.name, file);
  }
});
```

### 3. 错误处理

```javascript
async function handleApiCall(apiCall) {
  try {
    const response = await apiCall();
    if (response.data.success) {
      return response.data.data;
    } else {
      throw new Error(response.data.error);
    }
  } catch (error) {
    console.error('API调用失败:', error.message);
    
    // 根据错误类型进行不同处理
    if (error.message.includes('Invalid deduplication mode')) {
      console.error('去重模式参数错误');
    } else if (error.message.includes('Bucket not found')) {
      console.error('桶不存在');
    } else if (error.message.includes('Object not found')) {
      console.error('对象不存在');
    }
    
    throw error;
  }
}
```

### 4. 批量操作

```javascript
// 批量上传文件
async function batchUpload(bucketName, files, dedupMode = 'allow') {
  const results = [];
  for (const file of files) {
    try {
      const result = await uploadObjectWithDeduplication(
        bucketName, 
        file.name, 
        file, 
        dedupMode, 
        file.type
      );
      results.push({ success: true, file: file.name, result });
    } catch (error) {
      results.push({ success: false, file: file.name, error: error.message });
    }
  }
  return results;
}
```

### 5. 监控和日志

```javascript
// 添加请求日志
axios.interceptors.request.use(config => {
  console.log(`[${new Date().toISOString()}] ${config.method.toUpperCase()} ${config.url}`);
  if (config.params?.deduplication_mode) {
    console.log(`去重模式: ${config.params.deduplication_mode}`);
  }
  return config;
});

axios.interceptors.response.use(
  response => {
    console.log(`[${new Date().toISOString()}] ${response.status} ${response.config.url}`);
    return response;
  },
  error => {
    console.error(`[${new Date().toISOString()}] ${error.response?.status} ${error.config?.url}: ${error.message}`);
    return Promise.reject(error);
  }
);
```

## 限制和注意事项

### 1. 文件大小限制

- 默认最大文件大小: 100MB
- 可通过环境变量 `SEVINO_MAX_FILE_SIZE` 配置
- 大文件建议使用分片上传

### 2. 命名规范

- 桶名称: 3-63个字符，只能包含小写字母、数字、连字符和下划线
- 对象键: 最大1024个字符，不能包含控制字符

### 3. 并发限制

- 建议并发请求数不超过100
- 分片上传可以并行处理

### 4. 存储限制

- 确保有足够的磁盘空间
- 定期清理不需要的对象和桶

### 5. 去重模式限制

- `reject` 模式：检测到重复内容时会返回错误
- `reference` 模式：需要确保数据持有者不被删除
- 去重基于ETag（MD5哈希），相同内容会有相同的ETag

### 6. 分片上传限制

- 分片大小建议5MB-100MB
- 同一文件的所有分片必须使用相同的 `upload_id`
- 分片编号从1开始，必须连续
- 分片上传后需要客户端自行管理分片的合并

## 故障排除

### 常见问题

1. **服务无法启动**
   - 检查端口是否被占用
   - 确认数据目录权限
   - 查看日志输出

2. **上传失败**
   - 检查文件大小是否超限
   - 确认桶是否存在
   - 验证网络连接

3. **去重模式错误**
   - 检查去重模式参数是否正确
   - 确认模式名称拼写正确
   - 查看错误信息

4. **分片上传失败**
   - 检查分片编号是否连续
   - 确认 `upload_id` 是否一致
   - 验证分片大小是否合理

5. **下载失败**
   - 确认对象是否存在
   - 检查对象键是否正确
   - 验证桶名称

6. **性能问题**
   - 检查磁盘I/O性能
   - 监控内存使用情况
   - 考虑增加索引缓存

7. **CORS 错误**
   - 检查 `SEVINO_ENABLE_CORS` 是否设置为 `true`
   - 确认允许的域名配置正确
   - 检查浏览器控制台的错误信息

### 调试技巧

```bash
# 启用详细日志
RUST_LOG=debug cargo run

# 检查服务状态
curl -X GET "http://127.0.0.1:8000/health"

# 查看API文档
open "http://127.0.0.1:8000/swagger-ui/"

# 测试去重功能
curl -X PUT "http://127.0.0.1:8000/api/buckets/test/objects/file1.txt?deduplication_mode=reference" \
  -H "Content-Type: text/plain" \
  --data-binary "Hello, World!"

curl -X PUT "http://127.0.0.1:8000/api/buckets/test/objects/file2.txt?deduplication_mode=reference" \
  -H "Content-Type: text/plain" \
  --data-binary "Hello, World!"

# 检查对象元数据
curl -X GET "http://127.0.0.1:8000/api/buckets/test/objects/file1.txt/metadata" | jq
curl -X GET "http://127.0.0.1:8000/api/buckets/test/objects/file2.txt/metadata" | jq
```

---

**版本**: 0.1.0  
**最后更新**: 2024年1月  
**维护者**: Sevino Team 