use anyhow::Result;
use axum::response::Response;
use clap::Parser;
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_http::trace::TraceLayer;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use axum::{extract, http, response, response::IntoResponse, routing, Extension, Router};

mod api;
mod config;
mod console;
mod html;
mod model;
mod parser;
mod utils;

use config::{Config, LeaderboardConfig};

use self::config::MemberMetadata;

#[derive(Debug, Parser)]
enum Opt {
    /// Start a webserver that serves the leaderboard
    Server {
        /// TOML configuration file
        config: PathBuf,

        /// Bind address and port
        #[clap(default_value = "0.0.0.0:3000")]
        host: String,
    },

    /// Print the current standings of all leaderboards and exit
    Console {
        /// TOML configuration file
        config: PathBuf,
    },
}

impl Opt {
    fn config_path(&self) -> &Path {
        match self {
            Opt::Server { ref config, .. } => config,
            Opt::Console { ref config, .. } => config,
        }
    }
}

#[derive(Debug)]
enum WebError {
    NotFound,
    InternalError,
}

impl<T> From<T> for WebError
where
    T: Into<anyhow::Error>,
{
    fn from(_error: T) -> Self {
        Self::InternalError
    }
}

// API client that is shared across all requests (makes sure that we don't refresh simultaneously)
type AocClient = Arc<Mutex<api::Client>>;

async fn get_latest_leaderboard(
    Extension(cfg): Extension<Arc<HashMap<String, LeaderboardConfig>>>,
    Extension(metadata): Extension<
        Arc<HashMap<i32, HashMap<usize, MemberMetadata>>>,
    >,
    Extension(client): Extension<AocClient>,
) -> Result<response::Html<String>, WebError> {
    // Find the latest leaderboard by year
    let latest_leaderboard_cfg = cfg
        .values()
        .max_by_key(|cfg| cfg.year)
        .ok_or(WebError::NotFound)?;

    let slug = &latest_leaderboard_cfg.slug;
    get_leaderboard(
        extract::Path(slug.clone()),
        Extension(cfg),
        Extension(metadata),
        Extension(client),
    )
    .await
}

async fn get_leaderboard(
    extract::Path(slug): extract::Path<String>,
    Extension(cfg): Extension<Arc<HashMap<String, LeaderboardConfig>>>,
    Extension(metadata): Extension<
        Arc<HashMap<i32, HashMap<usize, MemberMetadata>>>,
    >,
    Extension(client): Extension<AocClient>,
) -> Result<response::Html<String>, WebError> {
    let leaderboard_cfg = if let Some(cfg) = cfg.get(&slug) {
        cfg
    } else {
        return Err(WebError::NotFound);
    };

    let leaderboard = {
        client
            .lock()
            .await
            .fetch(leaderboard_cfg.year, leaderboard_cfg.id)
            .await?
    };
    let scoreboard = model::Scoreboard::from_leaderboard(&leaderboard);

    let empty_metadata = HashMap::new();
    let metadata = metadata
        .get(&leaderboard_cfg.year)
        .unwrap_or(&empty_metadata);

    Ok(response::Html(html::render_template(
        leaderboard_cfg,
        metadata,
        &scoreboard,
    )))
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        let (status, error_message) = match self {
            Self::NotFound => (http::StatusCode::NOT_FOUND, "404 Not Found"),
            Self::InternalError => (
                http::StatusCode::INTERNAL_SERVER_ERROR,
                "500 Internal Server Error",
            ),
        };
        (status, error_message).into_response()
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let opts = Opt::parse();
    let config = Config::from_file(opts.config_path())?;

    match opts {
        Opt::Server { host, .. } => {
            tracing_subscriber::registry()
                .with(tracing_subscriber::EnvFilter::new(
                    std::env::var("RUST_LOG").unwrap_or_else(|_| {
                        "advent_of_code_leaderboard=debug,tower_http=debug".into()
                    }),
                ))
                .with(tracing_subscriber::fmt::layer())
                .init();
            let client = api::Client::new(config.session, config.cache_dir);
            let metadata = config.metadata;
            let config = config
                .leaderboard
                .into_iter()
                .map(|l| (l.slug.clone(), l))
                .collect::<HashMap<_, _>>();

            let app = Router::new()
                .route("/{slug}", routing::get(get_leaderboard))
                .route("/", routing::get(get_latest_leaderboard))
                .layer(TraceLayer::new_for_http())
                .layer(Extension(Arc::new(config)))
                .layer(Extension(Arc::new(metadata)))
                .layer(Extension(Arc::new(Mutex::new(client))));

            let bind: SocketAddr = host.parse()?;
            tracing::info!("Listening on {}", &bind);
            let listener = tokio::net::TcpListener::bind(bind).await?;
            axum::serve(listener, app).await?;
        }
        Opt::Console { .. } => {
            let client = api::Client::new(config.session, config.cache_dir);
            let empty_metadata = HashMap::new();
            for leaderboard_cfg in config.leaderboard.into_iter() {
                let leaderboard = client
                    .fetch(leaderboard_cfg.year, leaderboard_cfg.id)
                    .await?;
                let scoreboard = model::Scoreboard::from_leaderboard(&leaderboard);
                let metadata = config
                    .metadata
                    .get(&leaderboard_cfg.year)
                    .unwrap_or(&empty_metadata);
                console::render_template(&leaderboard_cfg, metadata, &scoreboard);
            }
        }
    };

    Ok(())
}
