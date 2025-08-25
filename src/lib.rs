pub mod config;
pub mod database;
pub mod error;
pub mod logging;
pub mod media;
pub mod platform;
pub mod ssdp;
pub mod watcher;
pub mod web;

pub mod state {
    use crate::{
        config::AppConfig,
        database::DatabaseManager,
        platform::PlatformInfo,
    };
    use std::sync::Arc;

    #[derive(Clone)]
    pub struct AppState {
        pub config: Arc<AppConfig>,
        pub database: Arc<dyn DatabaseManager>,
        pub platform_info: Arc<PlatformInfo>,
        pub content_update_id: Arc<std::sync::atomic::AtomicU32>,
    }
}