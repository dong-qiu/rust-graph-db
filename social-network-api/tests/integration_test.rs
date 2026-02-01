use social_network_api::{
    config::Config,
    repository::{PostRepository, SocialGraphRepository, UserRepository},
    services::{ContentService, SocialGraphService, UserService},
    models::{CreateUserDto, CreatePostDto},
};
use rust_graph_db::storage::rocksdb_store::RocksDbStorage;
use std::sync::Arc;
use tempfile::TempDir;

async fn setup_test_services() -> (Arc<UserService>, Arc<SocialGraphService>, Arc<ContentService>, TempDir) {
    let temp_dir = TempDir::new().unwrap();
    let storage = Arc::new(
        RocksDbStorage::new(temp_dir.path(), "test_social_network").unwrap()
    ) as Arc<dyn rust_graph_db::GraphStorage>;

    let user_repo = Arc::new(UserRepository::new(storage.clone()));
    let social_repo = Arc::new(SocialGraphRepository::new(storage.clone()));
    let post_repo = Arc::new(PostRepository::new(storage.clone()));

    let user_service = Arc::new(UserService::new(user_repo.clone()));
    let social_service = Arc::new(SocialGraphService::new(social_repo.clone(), user_repo.clone()));
    let content_service = Arc::new(ContentService::new(post_repo, user_repo.clone(), social_repo));

    (user_service, social_service, content_service, temp_dir)
}

#[tokio::test]
async fn test_user_lifecycle() {
    let (user_service, _, _, _temp_dir) = setup_test_services().await;

    // Create user
    let user = user_service
        .create_user(CreateUserDto {
            username: "testuser".to_string(),
            email: "test@example.com".to_string(),
            display_name: "Test User".to_string(),
            password: "password123".to_string(),
            bio: Some("Test bio".to_string()),
            avatar_url: None,
        })
        .await
        .unwrap();

    assert_eq!(user.username, "testuser");
    assert_eq!(user.email, "test@example.com");

    // Get user
    let retrieved = user_service.get_user("testuser").await.unwrap();
    assert_eq!(retrieved.username, "testuser");

    // Delete user
    user_service.delete_user("testuser").await.unwrap();

    // Verify deleted
    let result = user_service.get_user("testuser").await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_social_graph() {
    let (user_service, social_service, _, _temp_dir) = setup_test_services().await;

    // Create two users
    user_service
        .create_user(CreateUserDto {
            username: "alice".to_string(),
            email: "alice@example.com".to_string(),
            display_name: "Alice".to_string(),
            password: "password123".to_string(),
            bio: None,
            avatar_url: None,
        })
        .await
        .unwrap();

    user_service
        .create_user(CreateUserDto {
            username: "bob".to_string(),
            email: "bob@example.com".to_string(),
            display_name: "Bob".to_string(),
            password: "password123".to_string(),
            bio: None,
            avatar_url: None,
        })
        .await
        .unwrap();

    // Alice follows Bob
    social_service.follow("alice", "bob").await.unwrap();

    // Get following
    let following = social_service.get_following("alice", 10).await.unwrap();
    assert_eq!(following.len(), 1);
    assert_eq!(following[0].username, "bob");

    // Get followers
    let followers = social_service.get_followers("bob", 10).await.unwrap();
    assert_eq!(followers.len(), 1);
    assert_eq!(followers[0].username, "alice");

    // Unfollow
    social_service.unfollow("alice", "bob").await.unwrap();

    let following = social_service.get_following("alice", 10).await.unwrap();
    assert_eq!(following.len(), 0);
}

#[tokio::test]
async fn test_posts_and_timeline() {
    let (user_service, social_service, content_service, _temp_dir) = setup_test_services().await;

    // Create users
    user_service
        .create_user(CreateUserDto {
            username: "alice".to_string(),
            email: "alice@example.com".to_string(),
            display_name: "Alice".to_string(),
            password: "password123".to_string(),
            bio: None,
            avatar_url: None,
        })
        .await
        .unwrap();

    user_service
        .create_user(CreateUserDto {
            username: "bob".to_string(),
            email: "bob@example.com".to_string(),
            display_name: "Bob".to_string(),
            password: "password123".to_string(),
            bio: None,
            avatar_url: None,
        })
        .await
        .unwrap();

    // Alice follows Bob
    social_service.follow("alice", "bob").await.unwrap();

    // Bob creates a post
    let post = content_service
        .create_post(
            "bob",
            CreatePostDto {
                content: "Hello world!".to_string(),
                visibility: Some("public".to_string()),
                media_url: None,
            },
        )
        .await
        .unwrap();

    assert_eq!(post.content, "Hello world!");

    // Alice should see Bob's post in timeline
    let timeline = content_service.get_timeline("alice", 10).await.unwrap();
    assert_eq!(timeline.len(), 1);
    assert_eq!(timeline[0].post.content, "Hello world!");
    assert_eq!(timeline[0].author.username, "bob");

    // Bob's timeline should be empty (not following anyone)
    let bob_timeline = content_service.get_timeline("bob", 10).await.unwrap();
    assert_eq!(bob_timeline.len(), 0);
}

#[tokio::test]
async fn test_likes() {
    let (user_service, _, content_service, _temp_dir) = setup_test_services().await;

    // Create users
    user_service
        .create_user(CreateUserDto {
            username: "alice".to_string(),
            email: "alice@example.com".to_string(),
            display_name: "Alice".to_string(),
            password: "password123".to_string(),
            bio: None,
            avatar_url: None,
        })
        .await
        .unwrap();

    user_service
        .create_user(CreateUserDto {
            username: "bob".to_string(),
            email: "bob@example.com".to_string(),
            display_name: "Bob".to_string(),
            password: "password123".to_string(),
            bio: None,
            avatar_url: None,
        })
        .await
        .unwrap();

    // Bob creates a post
    let post = content_service
        .create_post(
            "bob",
            CreatePostDto {
                content: "Test post".to_string(),
                visibility: Some("public".to_string()),
                media_url: None,
            },
        )
        .await
        .unwrap();

    let post_id = post.id.to_string();

    // Alice likes the post
    content_service.like_post("alice", &post_id).await.unwrap();

    // Get likes
    let likes = content_service.get_post_likes(&post_id).await.unwrap();
    assert_eq!(likes.len(), 1);
    assert_eq!(likes[0].username, "alice");

    // Unlike
    content_service.unlike_post("alice", &post_id).await.unwrap();

    let likes = content_service.get_post_likes(&post_id).await.unwrap();
    assert_eq!(likes.len(), 0);
}

#[tokio::test]
async fn test_friend_suggestions() {
    let (user_service, social_service, _, _temp_dir) = setup_test_services().await;

    // Create three users: Alice, Bob, Charlie
    for (username, email, display_name) in [
        ("alice", "alice@example.com", "Alice"),
        ("bob", "bob@example.com", "Bob"),
        ("charlie", "charlie@example.com", "Charlie"),
    ] {
        user_service
            .create_user(CreateUserDto {
                username: username.to_string(),
                email: email.to_string(),
                display_name: display_name.to_string(),
                password: "password123".to_string(),
                bio: None,
                avatar_url: None,
            })
            .await
            .unwrap();
    }

    // Alice -> Bob -> Charlie
    social_service.follow("alice", "bob").await.unwrap();
    social_service.follow("bob", "charlie").await.unwrap();

    // Alice should get Charlie as a suggestion (friend of friend)
    let suggestions = social_service.suggest_friends("alice", 10).await.unwrap();

    // Should suggest Charlie
    let charlie_suggested = suggestions.iter().any(|(user, _)| user.username == "charlie");
    assert!(charlie_suggested, "Charlie should be suggested to Alice");
}
