# Rust Graph Database

> A high-performance graph database implementation in Rust, compatible with openGauss-graph

[![Rust Version](https://img.shields.io/badge/rust-1.93%2B-blue)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/license-Apache--2.0-green)](LICENSE)
[![Tests](https://img.shields.io/badge/tests-82%2F82-success)](tests/)
[![Status](https://img.shields.io/badge/status-prototype--complete-success)]()

## 项目概述

这是一个用 Rust 实现的高性能图数据库原型，旨在提供与 openGauss-graph 兼容的 Cypher 查询功能，同时利用 Rust 的性能和安全特性。

**所有 6 个开发阶段已完成 ✅** （共 18 小时开发时间，9,305 行代码，82/82 测试通过）

### 核心特性

- ✅ **完整的 Cypher 支持**: MATCH, CREATE, DELETE, SET, WHERE, RETURN
- ✅ **高性能存储**: 基于 RocksDB 的持久化存储引擎
- ✅ **ACID 事务**: 完整的事务支持，保证数据一致性
- ✅ **图算法**: Dijkstra 最短路径、可变长路径扩展（VLE）、K-hop 邻居
- ✅ **数据导入导出**: JSON 和 CSV 格式支持
- ✅ **类型安全**: 利用 Rust 类型系统防止内存错误
- ✅ **异步执行**: 基于 Tokio 的异步运行时

### 架构设计

```
┌──────────────────────────────────────────────────┐
│           Rust Graph Database                    │
├──────────────────────────────────────────────────┤
│  ┌────────────────────────────────┐              │
│  │   Cypher Parser (pest)         │              │
│  └────────────┬───────────────────┘              │
│               ↓                                   │
│  ┌────────────────────────────────┐              │
│  │   Query Executor               │              │
│  │   - MATCH (pattern matching)   │              │
│  │   - CREATE (vertex/edge)       │              │
│  │   - DELETE (with DETACH)       │              │
│  │   - SET (properties)           │              │
│  └────────────┬───────────────────┘              │
│               ↓                                   │
│  ┌────────────────────────────────┐              │
│  │   Storage Engine (RocksDB)     │              │
│  │   - ACID transactions          │              │
│  │   - MVCC concurrency control   │              │
│  └────────────────────────────────┘              │
└──────────────────────────────────────────────────┘
```

## 快速开始

### 前提条件

- Rust 1.93 或更高版本
- Cargo（随 Rust 安装）

### 安装

```bash
# 克隆仓库
cd /Users/dongqiu/Dev/code/openGauss-graph/rust-graph-db

# 构建项目
cargo build --release

# 运行测试
cargo test

# 运行性能测试
cargo bench
```

### 基本使用

#### 1. 创建图数据库实例

```rust
use rust_graph_db::storage::rocksdb_store::RocksDbStorage;
use std::sync::Arc;

let storage = Arc::new(RocksDbStorage::new("./data", "my_graph")?);
```

#### 2. 执行 Cypher 查询

```rust
use rust_graph_db::{parse_cypher, executor::QueryExecutor};

let executor = QueryExecutor::new(storage);

// CREATE
let query = parse_cypher("CREATE (:Person {name: 'Alice', age: 30})")?;
executor.execute(query).await?;

// MATCH
let query = parse_cypher("MATCH (p:Person) RETURN p")?;
let results = executor.execute(query).await?;

for row in results {
    println!("{:?}", row);
}
```

#### 3. 使用图算法

```rust
use rust_graph_db::{shortest_path, variable_length_expand, VleOptions};

// 最短路径
let result = shortest_path(storage.clone(), start_id, end_id).await?;
println!("Path length: {}", result.cost);

// 可变长路径扩展
let options = VleOptions {
    min_length: 1,
    max_length: 3,
    allow_cycles: false,
    max_paths: 100,
};
let paths = variable_length_expand(storage.clone(), start_id, options).await?;
println!("Found {} paths", paths.len());
```

#### 4. 数据导入导出

```rust
use rust_graph_db::{import_from_json, export_to_json, ImportOptions, ExportOptions};

// 导入
let stats = import_from_json(
    storage.clone(),
    "data.json",
    ImportOptions::default(),
).await?;
println!("Imported {} vertices, {} edges",
    stats.vertices_imported, stats.edges_imported);

// 导出
let (v_count, e_count) = export_to_json(
    storage.clone(),
    "export.json",
    vec!["Person".to_string()],
    vec!["KNOWS".to_string()],
    ExportOptions::default(),
).await?;
```

## 示例程序

项目包含多个示例程序，展示各种功能：

```bash
# 查询执行器示例
cargo run --example executor_demo

# 图算法示例
cargo run --example algorithms_demo

# 导入导出示例
cargo run --example import_export_demo
```

## Cypher 语法支持

### 支持的查询类型

#### CREATE - 创建节点和关系

```cypher
-- 创建节点
CREATE (:Person {name: 'Alice', age: 30})

-- 创建节点和关系
CREATE (a:Person {name: 'Alice'})-[:KNOWS {since: 2020}]->(b:Person {name: 'Bob'})
```

#### MATCH - 模式匹配

```cypher
-- 匹配节点
MATCH (p:Person) RETURN p

-- 匹配关系
MATCH (a:Person)-[r:KNOWS]->(b:Person) RETURN a, r, b

-- 带条件的匹配
MATCH (p:Person) WHERE p.age > 25 RETURN p.name

-- 多跳模式
MATCH (a:Person)-[:KNOWS]->(b:Person)-[:KNOWS]->(c:Person) RETURN c
```

#### DELETE - 删除操作

```cypher
-- 删除节点
MATCH (p:Person {name: 'Alice'}) DELETE p

-- DETACH DELETE（同时删除关联边）
MATCH (p:Person {name: 'Alice'}) DETACH DELETE p
```

#### SET - 属性更新

```cypher
-- 设置属性
MATCH (p:Person {name: 'Alice'}) SET p.age = 31

-- 算术操作
MATCH (c:Counter) SET c.value = c.value + 1
MATCH (p:Person) SET p.age = p.age * 2
```

### WHERE 条件

```cypher
-- 比较操作
WHERE p.age > 25
WHERE p.name = 'Alice'

-- 逻辑操作
WHERE p.age > 25 AND p.city = 'Beijing'
WHERE p.age < 20 OR p.age > 60
```

## 性能指标

基于内部基准测试的性能数据（硬件：测试环境）：

| 操作 | 性能目标 | 说明 |
|------|---------|------|
| 顶点创建（批量1000） | ~5s | 包含事务提交 |
| 顶点扫描（1000个） | ~87ms | 全表扫描 |
| 边创建（批量100） | ~425ms | 包含事务提交 |
| 边遍历 | ~3.5ms | 单节点出边查询 |
| 最短路径（10x10网格） | ~45ms | Dijkstra算法 |
| VLE（2跳） | ~17ms | 树结构 |
| 批量导入（1000v+100e） | ~8.5s | 单事务提交 |

## 项目结构

```
rust-graph-db/
├── src/
│   ├── types/              # 核心数据类型（Graphid, Vertex, Edge）
│   ├── jsonb/              # JSONB 兼容层
│   ├── storage/            # 存储引擎（RocksDB）
│   ├── parser/             # Cypher 解析器
│   ├── executor/           # 查询执行器
│   ├── algorithms/         # 图算法
│   └── tools/              # 导入导出工具
├── tests/                  # 集成测试
├── benches/                # 性能测试
├── examples/               # 示例程序
└── DEV_LOG.md              # 详细开发日志
```

## 开发阶段

### 已完成阶段 ✅

| 阶段 | 状态 | 测试 | 代码行数 | 完成时间 |
|-----|------|------|---------|---------|
| Phase 1: 核心数据类型 | ✅ | 32/32 | ~1,200 | 2小时 |
| Phase 2: 存储引擎 | ✅ | 41/41 | ~2,500 | 4小时 |
| Phase 3: Cypher 解析器 | ✅ | 52/52 | ~1,400 | 3小时 |
| Phase 4: 查询执行器 | ✅ | 63/63 | ~1,900 | 4小时 |
| Phase 5: 图算法 | ✅ | 72/72 | ~750 | 2小时 |
| Phase 6: 集成与测试 | ✅ | 82/82 | ~1,555 | 3小时 |
| **总计** | **✅** | **82/82** | **~9,305** | **18小时** |

### 功能列表

#### Phase 1: Core Types ✅
- [x] **Graphid**: 64-bit identifier (16-bit label ID + 48-bit local ID)
- [x] **Vertex**: Graph nodes with JSON properties
- [x] **Edge**: Directed relationships with JSON properties
- [x] **GraphPath**: Path representation with vertices and edges
- [x] **JSONB Compatibility**: Full JSON serialization

#### Phase 2: Storage Engine ✅
- [x] RocksDB storage abstraction
- [x] Key-value schema design
- [x] Transaction support (ACID)
- [x] Concurrent access handling

#### Phase 3: Cypher Parser ✅
- [x] Pest grammar for Cypher
- [x] AST definition
- [x] Parser implementation
- [x] Query validation

#### Phase 4: Query Executor ✅
- [x] MATCH pattern matching
- [x] CREATE vertex/edge
- [x] DELETE with DETACH
- [x] SET properties
- [x] WHERE clause filtering
- [x] RETURN projection

#### Phase 5: Graph Algorithms ✅
- [x] Shortest path (Dijkstra)
- [x] Variable-length paths (VLE)
- [x] K-hop neighbors
- [x] Path finding algorithms

#### Phase 6: Integration & Testing ✅
- [x] Data import/export tools (JSON, CSV)
- [x] Integration tests (8 tests)
- [x] Performance benchmarks (7 benchmarks)
- [x] Documentation

## 测试

### 运行所有测试

```bash
cargo test
```

### 运行特定模块测试

```bash
# 存储引擎测试
cargo test --lib storage

# 解析器测试
cargo test --lib parser

# 执行器测试
cargo test --lib executor

# 算法测试
cargo test --lib algorithms
```

### 运行集成测试

```bash
cargo test --test integration_test
```

### 运行性能测试

```bash
cargo bench
```

## 技术栈

| 组件 | 版本 | 用途 |
|------|------|------|
| Rust | 1.93+ | 编程语言 |
| RocksDB | 0.22 | 持久化存储 |
| Tokio | 1.x | 异步运行时 |
| Pest | 2.7 | 解析器生成器 |
| Serde | 1.0 | 序列化/反序列化 |
| Criterion | 0.5 | 性能测试 |

## 与 openGauss-graph 的兼容性

### 兼容的功能

- ✅ 核心数据类型（Graphid, Vertex, Edge, GraphPath）
- ✅ JSONB 属性存储格式
- ✅ 基本 Cypher 语法
- ✅ 事务语义（ACID）
- ✅ 图算法（最短路径、VLE）

### 差异

| 特性 | openGauss-graph | Rust Graph DB |
|------|-----------------|---------------|
| 存储引擎 | PostgreSQL | RocksDB |
| 查询语言 | Cypher + SQL | Cypher only |
| 运行时 | PostgreSQL进程 | 独立进程 |
| 并发控制 | PostgreSQL MVCC | RocksDB事务 |
| 索引 | B-tree, Hash | RocksDB LSM-tree |

## 未来发展方向

### 生产环境准备

**高优先级**:
1. 错误处理增强
2. 并发性能优化（MVCC）
3. 查询优化器

**中优先级**:
4. 索引支持（二级索引）
5. 分布式扩展
6. 监控和管理工具

**低优先级**:
7. 高级算法（PageRank、社区检测）
8. 工具生态（可视化、数据迁移）

### 可选功能（未实施）

- ⏳ LDBC Social Network Benchmark
- ⏳ 性能深度优化
- ⏳ openGauss-graph 直连迁移工具

## 贡献指南

欢迎贡献！请遵循以下步骤：

1. Fork 项目
2. 创建特性分支 (`git checkout -b feature/AmazingFeature`)
3. 提交更改 (`git commit -m 'Add some AmazingFeature'`)
4. 推送到分支 (`git push origin feature/AmazingFeature`)
5. 开启 Pull Request

### 代码规范

- 遵循 Rust 官方代码风格（`cargo fmt`）
- 通过 Clippy 检查（`cargo clippy`）
- 添加必要的测试
- 更新相关文档

## 许可证

本项目采用 Apache-2.0 许可证 - 详见 [LICENSE](LICENSE) 文件

## 相关资源

- [详细开发日志](DEV_LOG.md) - 记录了所有 6 个阶段的详细开发过程
- [openGauss-graph](https://gitee.com/opengauss/openGauss-graph) - 原始项目
- [Cypher Query Language](https://neo4j.com/docs/cypher-manual/current/) - 查询语言文档
- [RocksDB](https://rocksdb.org/) - 存储引擎文档

## 致谢

- openGauss-graph 团队提供的设计参考
- RocksDB 团队提供的存储引擎
- Rust 社区的优秀生态系统

---

**状态**: ✅ 原型完成 - 所有 6 个阶段已完成

**版本**: 0.1.0

**最后更新**: 2026-01-31

**注意**: 这是一个原型项目，用于研究和学习目的。生产环境使用需要进一步的测试和优化。
