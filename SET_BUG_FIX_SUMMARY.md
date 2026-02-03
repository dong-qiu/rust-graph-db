# SET 语句 Bug 修复总结

## 问题描述

SET 语句的属性路径解析不完整，导致嵌套属性无法正确设置。

**症状**:
- `SET p.address.city = 'Shanghai'` 无法正确解析属性路径
- 解析器返回空的 `properties` 数组而不是 `["address", "city"]`
- 多个 SET 操作对同一实体的更新会相互覆盖

## 根本原因

### 问题 1: 解析器 Bug（src/parser/builder.rs:658）

**原始实现**:
```rust
fn build_property_expression(pair: Pair<Rule>) -> ParseResult<PropertyExpression> {
    let mut parts: Vec<String> = Vec::new();

    for inner_pair in pair.into_inner() {
        if inner_pair.as_rule() == Rule::identifier {
            parts.push(inner_pair.as_str().to_string());
        }
    }
    // ...
}
```

**问题**:
- 只提取 `Rule::identifier`，忽略了 `Rule::property_lookup`
- `property_lookup = { "." ~ identifier }` 的嵌套结构未被处理
- 导致 `n.address.city` 中的 `address` 和 `city` 无法被提取

### 问题 2: SET Executor 缺陷（src/executor/set_executor.rs:29）

**原始实现**:
```rust
pub async fn execute_with_context(&mut self, items: &[SetItem], rows: &[Row]) -> ExecutionResult<()> {
    let mut tx = self.storage.begin_transaction().await?;

    for row in rows {
        for item in items {
            self.apply_set_item(&mut tx, item, row).await?;  // 每次都读取原始值
        }
    }

    tx.commit().await?;
    Ok(())
}
```

**问题**:
- 每个 SET 操作都单独执行 get → update → save
- RocksDB 事务使用 WriteBatch，没有写缓存
- 第二次 SET 会读取原始值，覆盖第一次的修改
- 例如: `SET p.age = 31, p.city = 'Shanghai'` 中，第二个 SET 会丢失 age 的更新

## 修复方案

### 修复 1: 正确解析属性路径

**位置**: `src/parser/builder.rs:658-686`

```rust
fn build_property_expression(pair: Pair<Rule>) -> ParseResult<PropertyExpression> {
    let mut base: Option<String> = None;
    let mut properties: Vec<String> = Vec::new();

    for inner_pair in pair.into_inner() {
        match inner_pair.as_rule() {
            Rule::identifier => {
                // First identifier is the base
                if base.is_none() {
                    base = Some(inner_pair.as_str().to_string());
                }
            }
            Rule::property_lookup => {
                // Extract identifier from property_lookup
                for lookup_inner in inner_pair.into_inner() {
                    if lookup_inner.as_rule() == Rule::identifier {
                        properties.push(lookup_inner.as_str().to_string());
                    }
                }
            }
            _ => {}
        }
    }

    let base = base.ok_or_else(|| ParseError::InvalidSyntax(
        "Property expression must have a base identifier".into(),
    ))?;

    Ok(PropertyExpression { base, properties })
}
```

**改进**:
- 正确处理 `Rule::property_lookup` 嵌套结构
- 明确区分 base identifier 和属性路径
- 支持任意深度的嵌套属性（如 `p.contact.address.city`）

### 修复 2: 批量处理同一实体的多个 SET 操作

**位置**: `src/executor/set_executor.rs:29-148`

**关键改进**:
1. 添加 `apply_set_items_for_row` 方法，按实体分组 SET 操作
2. 添加 `update_vertex_properties_batch` 方法，一次读取-多次修改-一次保存
3. 添加 `update_edge_properties_batch` 方法，处理边的批量更新

```rust
pub async fn execute_with_context(&mut self, items: &[SetItem], rows: &[Row]) -> ExecutionResult<()> {
    let mut tx = self.storage.begin_transaction().await?;

    for row in rows {
        // Group SET operations by entity to handle multiple updates correctly
        self.apply_set_items_for_row(&mut tx, items, row).await?;
    }

    tx.commit().await?;
    Ok(())
}
```

**优势**:
- 对同一实体的多个 SET 操作只需一次数据库读取
- 所有修改在内存中完成后一次性保存
- 避免了事务写缓存缺失导致的更新丢失问题
- 性能更优（减少数据库 I/O）

## 新增测试

### 解析器测试（src/parser/mod.rs）

1. `test_parse_set_nested_property` - 测试两层嵌套属性
2. `test_parse_set_deep_nested_property` - 测试三层嵌套属性

### Executor 测试（src/executor/set_executor.rs）

1. `test_set_nested_property` - 测试嵌套属性更新
2. `test_set_nested_property_nonexistent` - 测试不存在的嵌套路径（应失败）

### 集成测试（tests/test_set_nested_integration.rs）

1. `test_set_nested_property_end_to_end` - 完整流程测试（解析+执行）
2. `test_set_deep_nested_property` - 深层嵌套属性测试
3. `test_set_multiple_nested_properties` - 多个 SET 操作测试

## 测试结果

```bash
$ cargo test
test result: ok. 118 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

**覆盖场景**:
- ✅ 简单属性: `SET p.age = 30`
- ✅ 两层嵌套: `SET p.address.city = 'Shanghai'`
- ✅ 三层嵌套: `SET p.contact.address.city = 'Beijing'`
- ✅ 多个 SET: `SET p.age = 31, p.city = 'Shanghai'`
- ✅ 表达式求值: `SET p.age = p.age + 1`
- ✅ 不存在的路径: 正确报错

## 影响范围

**修改的文件**:
- `src/parser/builder.rs` - 修复属性表达式解析
- `src/executor/set_executor.rs` - 重构 SET 执行逻辑
- `src/parser/mod.rs` - 添加解析器测试
- `tests/test_set_nested_integration.rs` - 新增集成测试

**新增代码**:
- +150 行（executor 批量处理逻辑）
- +100 行（测试代码）

**删除代码**:
- -75 行（移除旧的单项处理方法）

**净增加**: ~175 行

## 后续改进建议

1. **事务写缓存**: 在 RocksDbTransaction 中添加写缓存，避免在同一事务中重复读取
2. **性能优化**: 考虑使用 `update_in_place` API 减少序列化开销
3. **错误处理**: 为嵌套属性不存在的情况提供更友好的错误信息
4. **自动创建**: 支持自动创建不存在的嵌套对象（可选特性）

## 兼容性

- ✅ 向后兼容：所有原有测试仍然通过
- ✅ API 不变：SET executor 的公共接口未修改
- ✅ 性能提升：批量处理减少了数据库 I/O

---

**修复完成日期**: 2026-02-03
**测试通过率**: 100% (118/118)
**代码审查**: 已通过
**状态**: ✅ 已合并
