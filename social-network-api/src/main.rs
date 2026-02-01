use axum::{
    routing::{delete, get, post, put},
    Router,
};
use rust_graph_db::storage::rocksdb_store::RocksDbStorage;
use social_network_api::{
    config::Config,
    handlers::{health, post as post_handlers, social, user},
    repository::{PostRepository, SocialGraphRepository, UserRepository},
    services::{ContentService, SocialGraphService, UserService},
    AppState,
};
use std::{net::SocketAddr, sync::Arc};
use tower_http::{
    cors::CorsLayer,
    trace::{DefaultMakeSpan, DefaultOnResponse, TraceLayer},
};
use tracing::Level;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load configuration
    let config = Config::from_env()?;

    // Initialize logging
    init_logging(&config)?;

    tracing::info!("Starting Social Network API");
    tracing::info!("Configuration: {:?}", config);

    // Initialize storage
    let storage = Arc::new(RocksDbStorage::new(
        &config.database.path,
        &config.database.namespace,
    )?);

    tracing::info!("Database initialized at {}", config.database.path);

    // Create repositories
    let user_repo = Arc::new(UserRepository::new(storage.clone()));
    let social_repo = Arc::new(SocialGraphRepository::new(storage.clone()));
    let post_repo = Arc::new(PostRepository::new(storage.clone()));

    // Create services
    let user_service = Arc::new(UserService::new(user_repo.clone()));
    let social_service = Arc::new(SocialGraphService::new(social_repo.clone(), user_repo.clone()));
    let content_service = Arc::new(ContentService::new(
        post_repo,
        user_repo.clone(),
        social_repo,
    ));

    // Create application state
    let state = AppState {
        user_service,
        social_service,
        content_service,
    };

    // Build router
    let app = Router::new()
        // Health check
        .route("/health", get(health::health_check))
        // User endpoints
        .route("/api/v1/users", post(user::create_user))
        .route("/api/v1/users/:username", get(user::get_user))
        .route("/api/v1/users/:username", put(user::update_user))
        .route("/api/v1/users/:username", delete(user::delete_user))
        // Social graph endpoints
        .route(
            "/api/v1/users/:username/follow/:target",
            post(social::follow_user),
        )
        .route(
            "/api/v1/users/:username/follow/:target",
            delete(social::unfollow_user),
        )
        .route(
            "/api/v1/users/:username/followers",
            get(social::get_followers),
        )
        .route(
            "/api/v1/users/:username/following",
            get(social::get_following),
        )
        .route(
            "/api/v1/users/:username/suggested-friends",
            get(social::get_friend_suggestions),
        )
        .route(
            "/api/v1/users/:username/network",
            get(social::analyze_network),
        )
        // Post endpoints
        .route("/api/v1/posts", post(post_handlers::create_post))
        .route("/api/v1/posts/:id", get(post_handlers::get_post))
        .route("/api/v1/posts/:id", delete(post_handlers::delete_post))
        .route(
            "/api/v1/users/:username/timeline",
            get(post_handlers::get_timeline),
        )
        .route("/api/v1/posts/:id/like", post(post_handlers::like_post))
        .route(
            "/api/v1/posts/:id/like",
            delete(post_handlers::unlike_post),
        )
        .route(
            "/api/v1/posts/:id/likes",
            get(post_handlers::get_post_likes),
        )
        .with_state(state)
        // Middleware
        .layer(
            TraceLayer::new_for_http()
                .make_span_with(DefaultMakeSpan::new().level(Level::INFO))
                .on_response(DefaultOnResponse::new().level(Level::INFO)),
        )
        .layer(CorsLayer::permissive());

    // Start server
    let addr = SocketAddr::from(([0, 0, 0, 0], config.server.port));
    tracing::info!("Server listening on {}", addr);

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

fn init_logging(config: &Config) -> anyhow::Result<()> {
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(&config.logging.level));

    match config.logging.format.as_str() {
        "json" => {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer().json())
                .init();
        }
        _ => {
            tracing_subscriber::registry()
                .with(filter)
                .with(tracing_subscriber::fmt::layer().pretty())
                .init();
        }
    }

    Ok(())
}
