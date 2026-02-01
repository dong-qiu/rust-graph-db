# Social Network API

A complete social network REST API built on rust-graph-db, demonstrating real-world graph database usage.

## Features

- ✅ User management (create, read, update, delete)
- ✅ Social graph (follow/unfollow, followers, following)
- ✅ Content management (posts, timeline)
- ✅ Interactions (likes)
- ✅ Friend recommendations (2-hop neighbors)
- ✅ Network analysis (shortest path, degrees of separation)
- ✅ Full REST API with JSON responses

## Quick Start

### 1. Setup

```bash
# Set environment variables (or create .env file)
export DB_PATH=./data/social-network
export SERVER_PORT=3000
export LOG_LEVEL=info
```

### 2. Run the server

```bash
cargo run --package social-network-api
```

The server will start on `http://localhost:3000`

### 3. Try it out

```bash
# Create a user
curl -X POST http://localhost:3000/api/v1/users \
  -H "Content-Type: application/json" \
  -d '{
    "username": "alice",
    "email": "alice@example.com",
    "display_name": "Alice Johnson",
    "password": "password123",
    "bio": "Graph database enthusiast"
  }'

# Get user
curl http://localhost:3000/api/v1/users/alice

# Create another user
curl -X POST http://localhost:3000/api/v1/users \
  -H "Content-Type: application/json" \
  -d '{
    "username": "bob",
    "email": "bob@example.com",
    "display_name": "Bob Smith",
    "password": "password123"
  }'

# Alice follows Bob
curl -X POST http://localhost:3000/api/v1/users/alice/follow/bob

# Bob creates a post
curl -X POST http://localhost:3000/api/v1/posts \
  -H "Content-Type: application/json" \
  -d '{
    "username": "bob",
    "content": "Just built my first graph database!",
    "visibility": "public"
  }'

# Alice views timeline (should see Bob's post)
curl http://localhost:3000/api/v1/users/alice/timeline?limit=20
```

## API Documentation

### User Endpoints

#### Create User
```
POST /api/v1/users
```
Request body:
```json
{
  "username": "alice",
  "email": "alice@example.com",
  "display_name": "Alice Johnson",
  "password": "password123",
  "bio": "Optional bio",
  "avatar_url": "https://example.com/avatar.jpg"
}
```

#### Get User
```
GET /api/v1/users/:username
```

#### Update User
```
PUT /api/v1/users/:username
```
Request body:
```json
{
  "email": "newemail@example.com",
  "display_name": "New Name",
  "bio": "Updated bio",
  "avatar_url": "https://example.com/new-avatar.jpg"
}
```

#### Delete User
```
DELETE /api/v1/users/:username
```

### Social Graph Endpoints

#### Follow User
```
POST /api/v1/users/:username/follow/:target
```

#### Unfollow User
```
DELETE /api/v1/users/:username/follow/:target
```

#### Get Followers
```
GET /api/v1/users/:username/followers?limit=50
```

#### Get Following
```
GET /api/v1/users/:username/following?limit=50
```

#### Get Friend Suggestions
```
GET /api/v1/users/:username/suggested-friends?limit=10
```

Returns users based on 2-hop neighbors (friends of friends), ranked by number of mutual friends.

#### Network Analysis
```
GET /api/v1/users/:username/network?target=other_username
```

Returns:
- Shortest path between users
- Degrees of separation
- Mutual friends

### Post Endpoints

#### Create Post
```
POST /api/v1/posts
```
Request body:
```json
{
  "username": "alice",
  "content": "Hello world!",
  "visibility": "public",
  "media_url": "https://example.com/photo.jpg"
}
```

#### Get Post
```
GET /api/v1/posts/:id
```

#### Delete Post
```
DELETE /api/v1/posts/:id
```

#### Get Timeline
```
GET /api/v1/users/:username/timeline?limit=50
```

Returns posts from users that `:username` follows, sorted by creation time (newest first).

### Interaction Endpoints

#### Like Post
```
POST /api/v1/posts/:id/like
```
Request body:
```json
{
  "username": "alice"
}
```

#### Unlike Post
```
DELETE /api/v1/posts/:id/like
```
Request body:
```json
{
  "username": "alice"
}
```

#### Get Post Likes
```
GET /api/v1/posts/:id/likes
```

### Health Check
```
GET /health
```

## Architecture

```
┌─────────────────────────────────────────────────┐
│         HTTP/REST API Layer (axum)              │
│  - User Management   - Social Graph             │
│  - Content           - Discovery                │
├─────────────────────────────────────────────────┤
│         Service Layer (Business Logic)          │
│  - UserService       - SocialGraphService       │
│  - ContentService                               │
├─────────────────────────────────────────────────┤
│      Repository Layer (Data Access)             │
│  - UserRepository    - PostRepository           │
│  - SocialRepository                             │
├─────────────────────────────────────────────────┤
│           rust-graph-db Core                    │
│  - GraphStorage      - Algorithms               │
│  - RocksDB Backend                              │
└─────────────────────────────────────────────────┘
```

## Graph Model

### Vertices
- `User`: username, email, display_name, bio, avatar_url, created_at, password_hash
- `Post`: content, created_at, visibility, media_url

### Edges
- `FOLLOWS`: User -> User (followed_at)
- `POSTED`: User -> Post (posted_at)
- `LIKES`: User -> Post (liked_at)

## Configuration

Environment variables:

- `SERVER_HOST`: Server host (default: `0.0.0.0`)
- `SERVER_PORT`: Server port (default: `3000`)
- `DB_PATH`: Database storage path (default: `./data/social-network`)
- `DB_NAMESPACE`: Database namespace (default: `social_network`)
- `LOG_LEVEL`: Logging level (default: `info`)
- `LOG_FORMAT`: Log format - `pretty` or `json` (default: `pretty`)

## Examples

See the examples directory for:
- Sample data generation
- Performance testing
- Integration tests

## Development

### Run tests
```bash
cargo test --package social-network-api
```

### Run with debug logging
```bash
LOG_LEVEL=debug cargo run --package social-network-api
```

### Build release version
```bash
cargo build --release --package social-network-api
```

## License

Part of the openGauss-graph project.
