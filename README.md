# Sevino - 去中心化对象存储服务

Sevino 是一个基于 Rust 开发的高性能对象存储服务，参考 MinIO 的设计理念，提供去中心化的重复数据删除功能。

## 🚀 特性

### 核心功能
- **对象存储**: 支持桶和对象的基本操作（创建、读取、更新、删除）
- **高性能索引**: 使用内存索引实现 O(1) 的对象查找
- **分页支持**: 支持大容量对象列表的分页查询
- **版本控制**: 支持对象版本管理
- **去中心化去重**: 智能的重复数据删除机制

### 去中心化Reference模式
- **无中心节点**: 没有"原始对象"和"引用对象"的区分
- **智能数据持有者**: 自动选择引用计数最高的对象作为数据持有者
- **灵活删除**: 任何对象都可以被删除，只要没有其他对象引用
- **存储优化**: 相同内容只存储一份数据，节省存储空间

### API 接口
- **RESTful API**: 完整的 REST API 接口
- **Swagger UI**: 内置 API 文档和测试界面
- **健康检查**: 服务状态监控
- **CORS 支持**: 跨域请求支持

## 📦 安装和运行

### 环境要求
- Rust 1.70+
- Linux/macOS/Windows

### 快速开始

1. **克隆项目**
```bash
git clone https://github.com/sanyexieai/sevino.git
cd sevino
```

2. **编译和运行**
```bash
cargo run
```

3. **访问服务**
- 服务地址: http://127.0.0.1:8000
- API 文档: http://127.0.0.1:8000/swagger-ui/

## 🏗️ 架构设计

### 存储结构
```
./data/
├── bucket1/
│   ├── .sevino.meta/
│   │   ├── bucket.json          # 桶元数据
│   │   └── objects/
│   │       ├── object1.json     # 对象元数据
│   │       └── object2.json
│   ├── a1b2/                    # 哈希化目录结构
│   │   └── c3/
│   │       └── a1b2c3d4...      # 对象数据文件
│   └── e5f6/
│       └── g7/
│           └── e5f6g7h8...
└── bucket2/
    └── ...
```

### 索引系统
- **对象索引**: `HashMap<bucket_name, HashMap<key, object_id>>`
- **ETag索引**: `HashMap<bucket_name, HashMap<etag, Vec<object_id>>>`
- **内存缓存**: 提供 O(1) 的查找性能

## 🔧 API 使用指南

### 桶操作

#### 创建桶
```bash
curl -X POST "http://127.0.0.1:8000/api/buckets" \
  -H "Content-Type: application/json" \
  -d '{"name": "my-bucket"}'
```

#### 列出桶
```bash
curl -X GET "http://127.0.0.1:8000/api/buckets"
```

#### 删除桶
```bash
curl -X DELETE "http://127.0.0.1:8000/api/buckets/my-bucket"
```

### 对象操作

#### 上传对象
```bash
curl -X PUT "http://127.0.0.1:8000/api/buckets/my-bucket/objects/test.txt" \
  -H "Content-Type: text/plain" \
  --data-binary "Hello, World!"
```

#### 下载对象
```bash
curl -X GET "http://127.0.0.1:8000/api/buckets/my-bucket/objects/test.txt"
```

#### 列出对象
```bash
curl -X GET "http://127.0.0.1:8000/api/buckets/my-bucket/objects"
```

#### 删除对象
```bash
curl -X DELETE "http://127.0.0.1:8000/api/buckets/my-bucket/objects/test.txt"
```

## 🔄 去中心化Reference模式详解

### 设计理念

传统的重复数据删除系统通常有一个"原始对象"的概念，其他相同内容的对象作为"引用对象"指向它。这种设计存在以下问题：

1. **中心化问题**: 原始对象成为单点故障
2. **删除限制**: 原始对象不能被删除，除非所有引用都被删除
3. **管理复杂**: 需要区分原始对象和引用对象

Sevino 的去中心化Reference模式解决了这些问题：

### 核心概念

#### 数据持有者 (Data Holder)
- 任何对象都可以成为数据持有者
- 数据持有者负责存储实际的数据文件
- 系统自动选择引用计数最高的对象作为数据持有者

#### 引用计数 (Reference Count)
- 每个数据持有者维护一个引用计数
- 引用计数表示有多少其他对象指向这个数据持有者
- 当引用计数为0时，数据持有者可以被删除

#### 数据持有者ID (Data Holder ID)
- 每个对象都有一个 `data_holder_id` 字段
- 如果为 `None`，表示自己是数据持有者
- 如果为 `Some(id)`，表示指向其他数据持有者

### 工作流程

#### 1. 创建引用
```rust
// 当检测到重复内容时
let data_holder_id = find_best_data_holder(duplicate_objects);
increase_reference_count(data_holder_id);

// 创建新对象，指向数据持有者
let new_metadata = ObjectMetadata {
    data_holder_id: Some(data_holder_id),
    reference_count: 0,
    // ... 其他字段
};
```

#### 2. 读取对象
```rust
// 确定数据持有者ID
let data_object_id = if let Some(holder_id) = &metadata.data_holder_id {
    holder_id.clone()  // 从数据持有者读取
} else {
    object_id          // 自己是数据持有者
};

// 读取数据
let data = fs::read(get_object_data_path(data_object_id));
```

#### 3. 删除对象
```rust
if let Some(data_holder_id) = &metadata.data_holder_id {
    // 删除引用对象
    delete_metadata();
    decrease_reference_count(data_holder_id);
} else {
    // 删除数据持有者
    if reference_count > 0 {
        return Error("Cannot delete: has references");
    }
    delete_data_file();
    delete_metadata();
}
```

### 优势

1. **去中心化**: 没有特殊的"原始对象"概念
2. **灵活删除**: 任何对象都可以被删除
3. **自动优化**: 系统自动选择最优的数据持有者
4. **容错性**: 数据持有者被删除时，可以自动转移给其他对象

## 🧪 测试

### 运行Reference模式测试
```bash
curl -X GET "http://127.0.0.1:8000/api/test/reference-mode"
```

### 测试场景
1. **基本引用创建**: 上传相同内容的不同对象
2. **引用计数管理**: 验证引用计数的增减
3. **数据读取**: 确保引用对象能正确读取数据
4. **删除机制**: 测试引用对象和数据持有者的删除
5. **多对象引用**: 测试多个对象指向同一个数据持有者

## ⚙️ 配置

### 环境变量
```bash
# 服务配置
SEVINO_HOST=127.0.0.1
SEVINO_PORT=8000
SEVINO_DATA_DIR=./data
SEVINO_MAX_FILE_SIZE=104857600
SEVINO_ENABLE_CORS=true
```

### 配置说明
- `SEVINO_HOST`: 服务监听地址
- `SEVINO_PORT`: 服务端口
- `SEVINO_DATA_DIR`: 数据存储目录
- `SEVINO_MAX_FILE_SIZE`: 最大文件大小（字节）
- `SEVINO_ENABLE_CORS`: 是否启用CORS

## 🔍 监控和调试

### 健康检查
```bash
curl -X GET "http://127.0.0.1:8000/health"
```

### 日志
服务使用 `tracing` 进行日志记录，可以通过环境变量控制日志级别：
```bash
RUST_LOG=debug cargo run
```

## 🚀 性能优化

### 索引优化
- 使用内存索引实现 O(1) 查找
- 分页查询避免内存溢出
- 哈希化目录结构避免单目录文件过多

### 存储优化
- 去中心化去重节省存储空间
- 智能数据持有者选择
- 引用计数管理

## 🔧 开发

### 项目结构
```
src/
├── main.rs          # 主程序入口
├── config.rs        # 配置管理
├── models.rs        # 数据模型
├── services.rs      # 业务逻辑
└── utils.rs         # 工具函数
```

### 添加新功能
1. 在 `models.rs` 中定义数据结构
2. 在 `services.rs` 中实现业务逻辑
3. 在 `main.rs` 中添加API路由
4. 更新Swagger文档

## 📄 许可证

MIT License

## 🤝 贡献

欢迎提交 Issue 和 Pull Request！

---

**Sevino** - 去中心化的高性能对象存储服务 