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
// `AdvertResource` is only spelled by the code that builds an advert, which is
// behind `transcode`; it stays exported so the XML writers' vocabulary is one
// list rather than a conditional one.
#[allow(unused_imports)]
pub use rendering::{
    container_class, generate_indexed_browse_response, generate_indexed_items_response,
    AdvertResource, BrowseRenderContext, TranscodeAdvert, ContainerSpec,
};

#[cfg(test)]
mod tests;
