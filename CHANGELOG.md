# Changelog

All notable changes to the Rust Graph Database project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Planned for Phase 2
- RocksDB storage engine implementation
- Transaction support with MVCC
- Key-value schema for vertices and edges
- Storage abstraction trait

### Planned for Phase 3
- Cypher query parser using pest
- AST definition and builder
- Basic query validation

### Planned for Phase 4
- MATCH pattern matching executor
- CREATE/DELETE/SET query execution
- WHERE clause filtering
- RETURN projection

## [0.1.0] - 2026-01-30

### Added - Phase 1 Complete ✅

#### Core Data Types
- **Graphid** implementation (`src/types/graphid.rs`)
  - 64-bit identifier with 16-bit label ID and 48-bit local ID
  - Validation and range checking
  - Display formatting (`{labid}.{locid}`)
  - Serialization/deserialization support
  - 8 comprehensive tests

- **Vertex** implementation (`src/types/vertex.rs`)
  - Vertex structure with ID, label, and JSON properties
  - Property manipulation methods (get, set, remove, has)
  - Property key enumeration
  - JSON serialization support
  - 7 comprehensive tests

- **Edge** implementation (`src/types/edge.rs`)
  - Directed edge with start, end, label, and properties
  - Property manipulation methods
  - Self-loop detection
  - Edge reversal operation
  - 8 comprehensive tests

- **GraphPath** implementation (`src/types/path.rs`)
  - Path construction and validation
  - Continuity checking
  - Path reversal
  - Vertex/edge containment checks
  - 9 comprehensive tests

#### JSONB Compatibility
- **JsonbContainer** implementation (`src/jsonb/mod.rs`)
  - MVP JSON serialization using serde_json
  - PostgreSQL JSONB format documentation
  - Roundtrip conversion support
  - Binary format stubs for Phase 2
  - 6 comprehensive tests

#### Project Infrastructure
- Cargo.toml configuration
  - Phase 1 dependencies (serde, serde_json, thiserror)
  - Commented Phase 2+ dependencies for future use
  - Benchmark configuration with criterion
  - Release profile optimization

- Project structure
  - Modular directory layout
  - Separate folders for types, jsonb, storage, parser, executor, algorithms
  - Examples directory
  - Tests and benchmarks setup

#### Documentation
- **README.md**: Comprehensive project overview
  - Architecture diagram
  - Feature checklist
  - Usage examples
  - Performance goals
  - Development roadmap

- **GETTING_STARTED.md**: Developer onboarding guide
  - Quick start instructions
  - Core concepts explanation
  - Usage examples
  - Testing guide
  - Troubleshooting

- **IMPLEMENTATION_STATUS.md**: Detailed progress tracking
  - Phase-by-phase breakdown
  - Milestone tracking
  - Risk assessment
  - Reference file locations

- **CHANGELOG.md**: This file

#### Examples
- **basic_usage.rs**: Comprehensive demonstration
  - Vertex and edge creation
  - Graph path construction
  - Property manipulation
  - Graphid operations
  - Serialization examples

#### Testing
- 32 comprehensive unit tests across all modules
- 100% passing test suite
- Test coverage for all public APIs
- Integration test infrastructure

#### Benchmarks
- Criterion-based benchmarking setup
- Benchmarks for:
  - Graphid creation
  - Vertex creation
  - Edge creation
  - Property access

### Technical Decisions

#### Dependency Management
- **Minimal Phase 1 dependencies**: Only serde, serde_json, and thiserror
  - Reduces build time and complexity
  - Avoids version conflicts
  - Enables focused development

- **Deferred heavy dependencies**: datafusion, arrow, rocksdb, etc.
  - Will be enabled in appropriate phases
  - Prevents unused dependency bloat
  - Cleaner dependency tree

#### JSONB Strategy
- **MVP approach**: UTF-8 JSON strings for Phase 1
  - Allows rapid development
  - Maintains API compatibility
  - Easy testing and debugging

- **Full binary format**: Planned for Phase 2
  - Complete PostgreSQL JSONB compatibility
  - Binary format parsing and generation
  - Performance optimization

#### Type System
- **Strong typing**: Leveraging Rust's type system
  - Graphid range validation
  - Path continuity enforcement
  - Property type safety

- **Serialization**: Full serde support
  - JSON interoperability
  - Future binary format support
  - Debug and testing convenience

### Quality Metrics

| Metric | Value |
|--------|-------|
| Total Tests | 32 |
| Test Pass Rate | 100% |
| Modules | 5 |
| Lines of Code | ~1,500 |
| Build Time (dev) | 3.74s |
| Documentation Files | 4 |
| Examples | 1 |

### Known Limitations

- **No storage engine yet**: In-memory only (Phase 2)
- **No query language**: Manual graph construction (Phase 3)
- **No graph algorithms**: Path operations only (Phase 5)
- **Simplified JSONB**: UTF-8 strings, not binary format (Phase 2)

### Breaking Changes
None (initial release)

### Deprecated
None

### Removed
None

### Fixed
- Arrow/DataFusion version conflicts resolved by deferring to Phase 2+

### Security
- Input validation on Graphid ranges
- Path continuity validation prevents invalid graphs
- No known security issues

---

## Development Notes

### Version Numbering
- **0.x.y**: Development versions (pre-1.0)
- **1.x.y**: Stable API (post Phase 6 completion)
- **x**: Major version (breaking changes)
- **y**: Minor version (features, non-breaking)
- **z**: Patch version (bug fixes)

### Release Checklist
- [ ] All tests passing
- [ ] Documentation updated
- [ ] CHANGELOG updated
- [ ] Version bumped in Cargo.toml
- [ ] Examples working
- [ ] Benchmarks run
- [ ] README reflects current state

### Next Release (0.2.0)
Target: Phase 2 completion (Week 7)
- Storage engine implementation
- RocksDB integration
- Transaction support

---

[Unreleased]: https://github.com/opengauss/rust-graph-db/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/opengauss/rust-graph-db/releases/tag/v0.1.0
