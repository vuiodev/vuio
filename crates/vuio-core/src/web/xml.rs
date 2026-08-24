// src\web\xml.rs
use crate::{
    database::{
        DatabaseManager, DatabaseReadSession, DirectoryView, MediaDirectory, MediaFile,
        MediaFileQuery, MediaFileView,
    },
    state::AppState,
};
use anyhow::Result;
use axum::body::Bytes;
use std::collections::HashMap;

mod browse;
mod descriptions;
mod rendering;

pub use browse::*;
pub use descriptions::*;
pub use rendering::{
    container_class, generate_indexed_browse_response, generate_indexed_items_response,
    BrowseRenderContext, TranscodeAdvert, ContainerSpec,
};

#[cfg(test)]
mod tests;
