#!/bin/bash
# Demo script for Social Network API
# This script demonstrates the full functionality of the API

set -e

BASE_URL="http://localhost:3000"

echo "========================================="
echo "Social Network API Demo"
echo "========================================="
echo ""

# Check if server is running
echo "Checking server health..."
curl -s $BASE_URL/health | jq .
echo ""

echo "========================================="
echo "1. Creating Users"
echo "========================================="

echo "Creating Alice..."
curl -s -X POST $BASE_URL/api/v1/users \
  -H "Content-Type: application/json" \
  -d '{
    "username": "alice",
    "email": "alice@example.com",
    "display_name": "Alice Johnson",
    "password": "password123",
    "bio": "Graph database enthusiast"
  }' | jq .
echo ""

echo "Creating Bob..."
curl -s -X POST $BASE_URL/api/v1/users \
  -H "Content-Type: application/json" \
  -d '{
    "username": "bob",
    "email": "bob@example.com",
    "display_name": "Bob Smith",
    "password": "password123",
    "bio": "Full-stack developer"
  }' | jq .
echo ""

echo "Creating Charlie..."
curl -s -X POST $BASE_URL/api/v1/users \
  -H "Content-Type: application/json" \
  -d '{
    "username": "charlie",
    "email": "charlie@example.com",
    "display_name": "Charlie Brown",
    "password": "password123",
    "bio": "Data scientist"
  }' | jq .
echo ""

echo "========================================="
echo "2. Building Social Graph"
echo "========================================="

echo "Alice follows Bob..."
curl -s -X POST $BASE_URL/api/v1/users/alice/follow/bob
echo "✓ Alice -> Bob"
echo ""

echo "Bob follows Charlie..."
curl -s -X POST $BASE_URL/api/v1/users/bob/follow/charlie
echo "✓ Bob -> Charlie"
echo ""

echo "Charlie follows Alice..."
curl -s -X POST $BASE_URL/api/v1/users/charlie/follow/alice
echo "✓ Charlie -> Alice"
echo ""

echo "Alice's following:"
curl -s $BASE_URL/api/v1/users/alice/following | jq '.[] | .username'
echo ""

echo "Alice's followers:"
curl -s $BASE_URL/api/v1/users/alice/followers | jq '.[] | .username'
echo ""

echo "========================================="
echo "3. Creating Posts"
echo "========================================="

echo "Bob creates a post..."
POST1=$(curl -s -X POST $BASE_URL/api/v1/posts \
  -H "Content-Type: application/json" \
  -d '{
    "username": "bob",
    "content": "Just built my first graph database!",
    "visibility": "public"
  }' | jq -r '.id')
echo "Post ID: $POST1"
echo ""

echo "Charlie creates a post..."
POST2=$(curl -s -X POST $BASE_URL/api/v1/posts \
  -H "Content-Type: application/json" \
  -d '{
    "username": "charlie",
    "content": "Exploring graph algorithms today 📊",
    "visibility": "public"
  }' | jq -r '.id')
echo "Post ID: $POST2"
echo ""

echo "========================================="
echo "4. Viewing Timeline"
echo "========================================="

echo "Alice's timeline (should see Bob's post):"
curl -s "$BASE_URL/api/v1/users/alice/timeline?limit=10" | jq '.[] | {author: .author.username, content: .post.content}'
echo ""

echo "Bob's timeline (should see Charlie's post):"
curl -s "$BASE_URL/api/v1/users/bob/timeline?limit=10" | jq '.[] | {author: .author.username, content: .post.content}'
echo ""

echo "========================================="
echo "5. Liking Posts"
echo "========================================="

echo "Alice likes Bob's post..."
curl -s -X POST "$BASE_URL/api/v1/posts/$POST1/like" \
  -H "Content-Type: application/json" \
  -d '{"username": "alice"}'
echo "✓"
echo ""

echo "Charlie also likes Bob's post..."
curl -s -X POST "$BASE_URL/api/v1/posts/$POST1/like" \
  -H "Content-Type: application/json" \
  -d '{"username": "charlie"}'
echo "✓"
echo ""

echo "Who liked Bob's post:"
curl -s "$BASE_URL/api/v1/posts/$POST1/likes" | jq '.[] | .username'
echo ""

echo "========================================="
echo "6. Friend Recommendations"
echo "========================================="

echo "Friend suggestions for Alice (should see Charlie through Bob):"
curl -s "$BASE_URL/api/v1/users/alice/suggested-friends?limit=5" | jq '.[] | {username: .user.username, mutual_friends: .mutual_friends}'
echo ""

echo "========================================="
echo "7. Network Analysis"
echo "========================================="

echo "Analyzing network between Alice and Charlie:"
curl -s "$BASE_URL/api/v1/users/alice/network?target=charlie" | jq .
echo ""

echo "========================================="
echo "8. Getting User Info"
echo "========================================="

echo "Get Alice's profile:"
curl -s $BASE_URL/api/v1/users/alice | jq '{username, display_name, bio, created_at}'
echo ""

echo "========================================="
echo "Demo Complete!"
echo "========================================="
echo ""
echo "The social network has:"
echo "- 3 users (Alice, Bob, Charlie)"
echo "- 3 follow relationships"
echo "- 2 posts"
echo "- 2 likes on Bob's post"
echo ""
echo "Try exploring the API further!"
