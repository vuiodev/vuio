use super::*;

/// BrowseMetadata for a single container. Samsung TVs call this before opening
/// folders and use `childCount` to decide whether the folder is empty.
pub(super) async fn handle_browse_metadata<D: DatabaseManager + 'static>(
    params: &BrowseParams,
    state: &AppState<D>,
) -> Response {
    use crate::web::xml::generate_container_metadata_response;

    let update_id = state.content_update_id.load(Ordering::SeqCst);
    let (parent_id, title, class, child_count) =
        resolve_container_metadata(&params.object_id, state).await;
    let response = generate_container_metadata_response(
        &params.object_id,
        &parent_id,
        &title,
        class,
        child_count,
        update_id,
    );
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, "text/xml; charset=utf-8"),
            (header::HeaderName::from_static("ext"), ""),
        ],
        response,
    )
        .into_response()
}

pub(super) async fn resolve_container_metadata<D: DatabaseManager + 'static>(
    object_id: &str,
    state: &AppState<D>,
) -> (String, String, &'static str, usize) {
    use crate::web::xml::container_class::STORAGE_FOLDER;

    if object_id == "0" {
        return (
            "-1".to_string(),
            state.current_config().server.name.clone(),
            STORAGE_FOLDER,
            4, // Video, Music, Pictures, Radio
        );
    }

    if object_id == "video" {
        let count = count_media_folder_children(state, "video/", "").await;
        return (
            "0".to_string(),
            "Video".to_string(),
            STORAGE_FOLDER,
            count.max(1),
        );
    }
    if object_id == "image" {
        let count = count_media_folder_children(state, "image/", "").await;
        return (
            "0".to_string(),
            "Pictures".to_string(),
            STORAGE_FOLDER,
            count.max(1),
        );
    }
    if object_id == "radio" {
        return (
            "0".to_string(),
            "Radio".to_string(),
            STORAGE_FOLDER,
            1,
        );
    }

    // Music containers describe themselves, so their title, parent and class
    // come from the same place the browse response builds them from. Samsung
    // decides a folder is empty from childCount, so a container that is known
    // to hold something must never report zero.
    if let Some(audio_path) = object_id.strip_prefix("audio") {
        let audio_path = audio_path.trim_start_matches('/');
        match parse_music_path(audio_path) {
            Some(MusicNode::Folders(folder)) if !folder.is_empty() => {
                let count = count_media_folder_children(state, "audio/", &folder).await;
                let node = MusicNode::Folders(folder);
                return (node.parent_id(), node.title(), node.class(), count.max(1));
            }
            Some(node) => {
                let count = music_child_count(&node, state).await;
                return (node.parent_id(), node.title(), node.class(), count);
            }
            None => {}
        }
    }

    let (media_filter, path_prefix, parent_id, title) =
        if let Some(rest) = object_id.strip_prefix("video") {
            let rest = rest.trim_start_matches('/');
            let title = rest
                .rsplit('/')
                .next()
                .filter(|part| !part.is_empty())
                .unwrap_or("Video")
                .to_string();
            let parent = if rest.is_empty() {
                "0".to_string()
            } else if let Some((parent, _)) = object_id.rsplit_once('/') {
                parent.to_string()
            } else {
                "0".to_string()
            };
            ("video/", rest.to_string(), parent, title)
        } else if let Some(rest) = object_id.strip_prefix("image") {
            let rest = rest.trim_start_matches('/');
            let title = rest
                .rsplit('/')
                .next()
                .filter(|part| !part.is_empty())
                .unwrap_or("Pictures")
                .to_string();
            let parent = object_id
                .rsplit_once('/')
                .map(|(parent, _)| parent.to_string())
                .unwrap_or_else(|| "0".to_string());
            ("image/", rest.to_string(), parent, title)
        } else {
            let title = object_id
                .rsplit('/')
                .next()
                .unwrap_or(object_id)
                .to_string();
            let parent = object_id
                .rsplit_once('/')
                .map(|(parent, _)| parent.to_string())
                .unwrap_or_else(|| "0".to_string());
            ("", object_id.to_string(), parent, title)
        };

    let count = count_media_folder_children(state, media_filter, &path_prefix).await;
    (
        parent_id,
        title,
        crate::web::xml::container_class::STORAGE_FOLDER,
        count.max(1),
    )
}

/// How many children a music container has, for BrowseMetadata.
///
/// A track listing reports at least one rather than counting rows: Samsung only
/// uses the number to decide whether the container is worth opening, and
/// counting every track of every album to answer a probe is not worth it.
async fn music_child_count<D: DatabaseManager + 'static>(
    node: &MusicNode,
    state: &AppState<D>,
) -> usize {
    if node.track_query().is_some() {
        return 1;
    }
    match child_containers(node, state).await {
        Ok(containers) => containers.len().max(1),
        Err(error) => {
            warn!(%error, "failed to count music container children for BrowseMetadata");
            1
        }
    }
}

pub(super) async fn count_media_folder_children<D: DatabaseManager + 'static>(
    state: &AppState<D>,
    media_type_filter: &str,
    path_prefix_str: &str,
) -> usize {
    let monitored_dirs = state.media_directories.read().await.clone();
    let unavailable_roots = state.unavailable_roots.read().await.clone();
    let (dir_index_opt, relative_path) = parse_dir_index_prefix(path_prefix_str);

    if path_prefix_str.is_empty() && monitored_dirs.len() > 1 {
        return monitored_dirs
            .iter()
            .filter(|dir| {
                let path = PathBuf::from(&dir.path);
                path.is_dir() && !unavailable_roots.contains(&path)
            })
            .count();
    }

    let browse_path = match dir_index_opt {
        Some(idx) if idx < monitored_dirs.len() => {
            let base_path = PathBuf::from(&monitored_dirs[idx].path);
            if relative_path.is_empty() {
                base_path
            } else {
                base_path.join(relative_path)
            }
        }
        _ => {
            let media_root = state.current_config().get_primary_media_dir();
            if path_prefix_str.is_empty() {
                media_root
            } else {
                media_root.join(path_prefix_str)
            }
        }
    };

    if !browse_path.is_dir()
        || unavailable_roots
            .iter()
            .any(|root| browse_path.starts_with(root))
    {
        return 0;
    }

    let canonical_browse_path = match state.filesystem_manager.get_canonical_path(&browse_path) {
        Ok(canonical) => PathBuf::from(canonical),
        Err(_) => state.filesystem_manager.normalize_path(&browse_path),
    };
    let canonical_parent = canonical_browse_path.to_string_lossy().into_owned();
    let mime_family = media_type_filter.to_owned();
    let database = state.database.clone();
    match database
        .read(move |session| {
            let dirs = session
                .visit_direct_subdirectories(
                    &canonical_parent,
                    (!mime_family.is_empty()).then_some(mime_family.as_str()),
                    0,
                    0,
                    |_| Ok(()),
                )?
                .matched;
            let files = session
                .visit_files(
                    &crate::database::MediaFileQuery::Directory {
                        path: canonical_parent.clone(),
                        mime_family: (!mime_family.is_empty()).then_some(mime_family),
                    },
                    0,
                    0,
                    |_| Ok(()),
                )?
                .matched;
            Ok(dirs + files)
        })
        .await
    {
        Ok(count) => count,
        Err(error) => {
            warn!(%error, "failed to count container children for BrowseMetadata");
            1
        }
    }
}

