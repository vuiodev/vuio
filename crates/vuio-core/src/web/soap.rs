//! UPnP device/service descriptions and SOAP control handlers.

use crate::{
    database::{DatabaseManager, DatabaseReadSession, MediaDirectory},
    state::AppState,
    web::xml::{generate_description_xml, generate_scpd_xml},
};
use axum::{
    extract::State,
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
};
use std::{path::PathBuf, sync::atomic::Ordering, time::Instant};
use tracing::{debug, error, info, warn};

mod parser;
use parser::*;

mod common;
mod connection;
mod content_directory;
mod metadata;
mod music;

use common::*;
use music::*;
pub use connection::{
    connection_manager_control, connection_manager_scpd, media_receiver_registrar_control,
    media_receiver_registrar_scpd,
};
pub use content_directory::{
    content_directory_control, content_directory_scpd, description_handler,
};
use metadata::*;

#[cfg(test)]
mod tests;
