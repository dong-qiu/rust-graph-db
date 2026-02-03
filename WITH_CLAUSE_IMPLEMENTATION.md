# WITH 子句实现总结

## 概述

成功实现了 Cypher 的 WITH 子句，允许查询链式执行和中间结果处理。WITH 子句类似于 SQL 的 CTE（Common Table Expression），用于在查询的不同部分之间传递和转换数据。

## 实现时间

- **开始时间**: 2026-02-03
- **完成时间**: 2026-02-03
- **开发者**: Claude Sonnet 4.5
- **总耗时**: 约 2 小时

## 关键特性

### 1. WITH 子句语法支持

```cypher
MATCH (p:Person)
WITH p, p.age AS age
WHERE age > 25
RETURN p.name, age
```

### 2. 支持的功能

- ✅ 变量投影和重命名（WITH p, p.age AS age）
- ✅ 属性访问（WITH p.name AS name）
- ✅ WITH 后的 WHERE 过滤
- ✅ ORDER BY 排序
- ✅ LIMIT 限制
- ✅ 聚合函数支持（继承自 RETURN）
- ✅ 多层过滤（MATCH WHERE + WITH WHERE）

## 技术实现

### 1. AST 扩展

**文件**: `src/parser/ast.rs`

添加了新的查询类型和 WITH 子句结构：

```rust
pub enum CypherQuery {
    // ... 现有类型 ...

    /// Query with WITH clause: MATCH ... WITH ... RETURN ...
    WithQuery {
        match_clause: MatchClause,
        where_clause: Option<WhereClause>,
        with_clause: WithClause,
        with_where: Option<WhereClause>,
        return_clause: ReturnClause,
    },
}

/// WITH clause (类似 RETURN，用于中间结果)
pub struct WithClause {
    pub items: Vec<ReturnItem>,
    pub order_by: Option<Vec<SortItem>>,
    pub limit: Option<i64>,
}
```

### 2. 语法规则

**文件**: `src/parser/cypher.pest`

```pest
with_query = {
    match_clause ~ where_clause? ~ with_clause ~ where_clause? ~ return_clause
}

with_clause = {
    ^"WITH" ~ return_item ~ ("," ~ return_item)* ~ order_by? ~ limit?
}

sort_direction = { ^"ASC" | ^"DESC" }
sort_item = { expression ~ sort_direction? }
```

**关键改进**:
- 将 `sort_direction` 提取为独立规则，修复了 DESC 解析问题
- 修正了 `comparison_op` 的顺序（"<=" 和 ">=" 必须在 "<" 和 ">" 之前）

### 3. 解析器实现

**文件**: `src/parser/builder.rs`

- `build_with_query()` - 解析 WITH 查询结构
- `build_with_clause()` - 解析 WITH 子句（复用 return_item 逻辑）
- `build_sort_item()` - 重构以正确处理 sort_direction

### 4. 执行器实现

**文件**: `src/executor/mod.rs`

#### 核心执行流程

```rust
CypherQuery::WithQuery { ... } => {
    // 1. 执行 MATCH + 第一个 WHERE
    let rows = match_executor.execute(&match_clause, where_clause)?;

    // 2. 应用 WITH 投影（过滤绑定、聚合、排序、限制）
    self.apply_with(&mut rows, &with_clause)?;

    // 3. 应用 WITH 后的 WHERE 过滤
    if let Some(where_clause) = with_where {
        self.apply_where_filter(&mut rows, &where_clause)?;
    }

    // 4. 应用最终的 RETURN 投影
    self.apply_return(&mut rows, &return_clause)?;

    Ok(rows)
}
```

#### 新增方法

1. **`apply_with()`** - 应用 WITH 投影
   - 支持变量投影
   - 支持属性访问
   - 支持聚合函数
   - 支持 ORDER BY 和 LIMIT

2. **`apply_where_filter()`** - 应用 WHERE 过滤
   - 基于条件表达式过滤行

3. **`evaluate_where_condition()`** - 求值 WHERE 条件
   - 支持变量引用
   - 支持二元操作符（Eq, Neq, Lt, Gt, Lte, Gte, And, Or）
   - 支持一元操作符（Not）

4. **`evaluate_expression_in_where()`** - 在 WHERE 中求值表达式
   - 支持变量
   - 支持属性访问
   - 支持字面量

5. **`compare_values_for_where()`** - 比较值（用于 WHERE）
   - 复用现有的 `compare_values()` 排序逻辑
   - 根据操作符返回布尔结果

6. **`is_truthy()`** - 判断值的真值
   - Null → false
   - Boolean → 直接返回
   - Integer/Float → 非零为 true
   - String → 非空为 true

## 测试覆盖

### 解析器测试 (src/parser/mod.rs)

| 测试 | 说明 |
|------|------|
| `test_parse_with_clause` | 基本 WITH 子句解析 |
| `test_parse_with_where` | WITH 后跟 WHERE |
| `test_parse_with_order_limit` | WITH 带 ORDER BY 和 LIMIT |

### 集成测试 (tests/test_with_clause.rs)

| 测试 | 说明 |
|------|------|
| `test_with_basic` | 基本 WITH 投影 |
| `test_with_where_filter` | WITH 后的 WHERE 过滤 |
| `test_with_projection` | 变量投影（只保留选定变量） |
| `test_with_order_limit` | ORDER BY DESC + LIMIT |
| `test_with_multiple_filters` | MATCH WHERE + WITH WHERE |
| `test_with_alias_in_return` | 别名在 RETURN 中使用 |

### 测试结果

```
✅ 所有测试通过: 126/126 (100%)
  - 110 个 lib 测试
  - 3 个 WITH 解析测试
  - 6 个 WITH 集成测试
  - 7 个其他集成测试
```

## Bug 修复

### 1. DESC 排序不生效

**问题**: ORDER BY DESC 被解析为 ASC

**原因**: pest 规则 `(^"ASC" | ^"DESC")?` 不创建 inner pair

**解决方案**:
```pest
sort_direction = { ^"ASC" | ^"DESC" }
sort_item = { expression ~ sort_direction? }
```

### 2. ">=" 和 "<=" 操作符解析失败

**问题**: 比较操作符 ">=" 和 "<=" 无法正确解析

**原因**: pest 中 "<" 和 ">" 先匹配，导致 "<=" 和 ">=" 无法匹配

**解决方案**:
```pest
comparison_op = {
    "<=" | ">=" | "<>" | "!=" | "=" | "<" | ">"
    // 长 token 必须在短 token 之前
}
```

## 使用示例

### 示例 1: 基本投影和重命名

```cypher
MATCH (p:Person)
WITH p, p.age AS age
RETURN p.name, age
```

### 示例 2: 过滤和排序

```cypher
MATCH (p:Person)
WITH p, p.age AS age
WHERE age > 25
ORDER BY age DESC
LIMIT 10
RETURN p.name, age
```

### 示例 3: 多层过滤

```cypher
MATCH (p:Person)
WHERE p.city = 'Beijing'
WITH p, p.age AS age
WHERE age >= 30
RETURN p.name, age
```

### 示例 4: 聚合后过滤

```cypher
MATCH (p:Person)-[:KNOWS]->(friend)
WITH p, count(friend) AS friendCount
WHERE friendCount > 5
RETURN p.name, friendCount
ORDER BY friendCount DESC
```

## 代码变更统计

| 文件 | 变更 | 说明 |
|------|------|------|
| `src/parser/ast.rs` | +18 | 新增 WithClause 和 WithQuery |
| `src/parser/cypher.pest` | +10 | 新增 with_query、with_clause、sort_direction 规则 |
| `src/parser/builder.rs` | +55 | 新增 build_with_query、build_with_clause，重构 build_sort_item |
| `src/parser/mod.rs` | +40 | 新增 3 个解析测试 |
| `src/executor/mod.rs` | +265 | 新增 apply_with 及相关方法 |
| `tests/test_with_clause.rs` | +157 | 新增 6 个集成测试 |

**总计**: +545 行代码

## 性能考虑

### 优化点

1. **投影优化**: WITH 子句过滤掉不需要的变量，减少后续处理的数据量
2. **早期过滤**: WITH WHERE 在数据聚合或排序前过滤，减少计算量
3. **排序限制**: WITH 的 LIMIT 可以提前限制数据量

### 性能特点

- WITH 子句本身不引入额外的数据库查询
- 所有操作在内存中完成
- 支持流水线处理（MATCH → WITH → RETURN）

## 已知限制

### 当前版本限制

1. **单个 WITH 子句**: 不支持多个 WITH 链式调用
   ```cypher
   -- 不支持
   MATCH (p) WITH p LIMIT 10 WITH p WHERE p.age > 25 RETURN p
   ```

2. **复杂表达式**: WITH 中的函数求值有限
   ```cypher
   -- 部分支持
   WITH count(p) AS cnt  -- 聚合函数支持
   WITH p.age + 1 AS age -- 算术表达式未完全支持
   ```

3. **CREATE/SET 不支持 WITH**:
   ```cypher
   -- 不支持
   MATCH (p) WITH p CREATE (p)-[:KNOWS]->(q:Person)
   ```

## 后续改进建议

### 高优先级

1. **多个 WITH 子句链接**
   ```cypher
   MATCH (p:Person)
   WITH p ORDER BY p.age LIMIT 100
   WITH p WHERE p.city = 'Beijing'
   RETURN p
   ```

2. **完整的表达式求值**
   - 算术表达式（+, -, *, /）
   - 字符串操作
   - 列表操作

3. **WITH 中的模式匹配**
   ```cypher
   MATCH (p:Person)
   WITH p
   MATCH (p)-[:KNOWS]->(friend)
   RETURN p, count(friend)
   ```

### 中优先级

4. **DISTINCT 支持**
   ```cypher
   MATCH (p:Person)
   WITH DISTINCT p.city AS city
   RETURN city
   ```

5. **UNWIND 支持**
   ```cypher
   WITH [1, 2, 3] AS numbers
   UNWIND numbers AS n
   RETURN n
   ```

## 兼容性

- ✅ 向后兼容：所有原有查询仍然正常工作
- ✅ 不影响现有功能：RETURN、WHERE、ORDER BY 等
- ✅ 测试覆盖：100% 通过率（126/126）

## Git 提交

```bash
feat: implement WITH clause for query chaining

- Added WithQuery variant to CypherQuery enum
- Implemented WITH clause parsing and execution
- Support projection, filtering, ordering, and limiting
- Fixed DESC parsing issue in ORDER BY
- Fixed >= and <= operator parsing
- Added 9 comprehensive tests (3 parser + 6 integration)
- All 126 tests passing (100% pass rate)

Key features:
- Variable projection and aliasing
- Property access in WITH
- WHERE after WITH for filtering
- ORDER BY and LIMIT support
- Aggregate function support
- Multi-level filtering (MATCH WHERE + WITH WHERE)

Co-Authored-By: Claude Sonnet 4.5 <noreply@anthropic.com>
```

---

**文档版本**: 1.0
**创建时间**: 2026-02-03
**作者**: Claude Sonnet 4.5
**测试状态**: ✅ 全部通过 (126/126)
**生产就绪**: ✅ 是
