use super::*;

/// Extract audio metadata using audiotags library
pub(crate) async fn extract_audio_metadata(
    media_file: &mut MediaFile,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    use std::time::Duration;

    // Clone the path for the blocking operation
    let path = media_file.path.clone();

    // Wrap the synchronous I/O operation in spawn_blocking to prevent blocking the async runtime
    let metadata_result =
        tokio::task::spawn_blocking(move || audiotags::Tag::new().read_from_path(&path)).await;

    // Handle the result from spawn_blocking
    match metadata_result {
        Ok(Ok(tag)) => {
            // Extract basic metadata
            if let Some(title) = tag.title() {
                media_file.title = Some(title.to_string());
            }

            if let Some(artist) = tag.artist() {
                media_file.artist = Some(artist.to_string());
            }

            if let Some(album) = tag.album_title() {
                media_file.album = Some(album.to_string());
            }

            if let Some(genre) = tag.genre() {
                media_file.genre = Some(genre.to_string());
            }

            // Extract track number
            if let Some(track_num) = tag.track_number() {
                media_file.track_number = Some(track_num as u32);
            }

            // Extract year
            if let Some(year) = tag.year() {
                media_file.year = Some(year as u32);
            }

            // Extract album artist
            if let Some(album_artist) = tag.album_artist() {
                media_file.album_artist = Some(album_artist.to_string());
            }

            // Extract duration if available
            if let Some(duration) = tag.duration() {
                media_file.duration = Some(Duration::from_secs(duration as u64));
            }
        }
        Ok(Err(e)) => {
            // Failed to parse tags, but we still apply fallback filename parsing
            debug!(
                "Failed to extract metadata for {}: {}",
                media_file.path.display(),
                e
            );
        }
        Err(e) => {
            // spawn_blocking failed
            debug!(
                "Failed to execute blocking metadata extraction for {}: {}",
                media_file.path.display(),
                e
            );
        }
    }

    // Always fall back to parsing from filename for missing fields
    fallback_parse_filename(media_file);

    Ok(())
}

/// Parse metadata fields from a file path when tags are missing
pub(crate) fn fallback_parse_filename(media_file: &mut MediaFile) {
    if media_file.title.is_some() {
        return;
    }

    let filename_sans_ext = media_file
        .path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| media_file.filename.clone());

    if filename_sans_ext.contains(" - ") {
        let parts: Vec<&str> = filename_sans_ext.split(" - ").collect();
        if parts.len() >= 3 {
            let track_part = parts[0].trim();
            let mut track_num = None;
            let clean_track: String = track_part.chars().filter(|c| c.is_ascii_digit()).collect();
            if !clean_track.is_empty() {
                if let Ok(num) = clean_track.parse::<u32>() {
                    track_num = Some(num);
                }
            }

            if track_num.is_some() {
                if media_file.track_number.is_none() {
                    media_file.track_number = track_num;
                }
                if media_file.artist.is_none() {
                    media_file.artist = Some(parts[1].trim().to_string());
                }
                media_file.title = Some(parts[2..].join(" - ").trim().to_string());
            } else {
                if media_file.artist.is_none() {
                    media_file.artist = Some(parts[0].trim().to_string());
                }
                media_file.title = Some(parts[1..].join(" - ").trim().to_string());
            }
        } else if parts.len() == 2 {
            let part0 = parts[0].trim();
            let part1 = parts[1].trim();

            let clean_track: String = part0.chars().filter(|c| c.is_ascii_digit()).collect();
            if !clean_track.is_empty() && clean_track == part0 {
                if let Ok(num) = clean_track.parse::<u32>() {
                    if media_file.track_number.is_none() {
                        media_file.track_number = Some(num);
                    }
                }
                media_file.title = Some(part1.to_string());
            } else {
                let mut artist_name = part0;
                let mut track_num = None;
                if let Some(first_space) = part0.find(' ') {
                    let maybe_num = &part0[..first_space].trim_end_matches('.');
                    let clean: String = maybe_num.chars().filter(|c| c.is_ascii_digit()).collect();
                    if !clean.is_empty() && clean == *maybe_num {
                        if let Ok(num) = clean.parse::<u32>() {
                            track_num = Some(num);
                            artist_name = &part0[first_space + 1..];
                        }
                    }
                }

                if media_file.artist.is_none() {
                    media_file.artist = Some(artist_name.trim().to_string());
                }
                if media_file.track_number.is_none() && track_num.is_some() {
                    media_file.track_number = track_num;
                }
                media_file.title = Some(part1.to_string());
            }
        }
    } else {
        let mut title_part = filename_sans_ext.as_str();

        if let Some(first_space) = filename_sans_ext.find(' ') {
            let maybe_num = &filename_sans_ext[..first_space].trim_end_matches('.');
            let clean: String = maybe_num.chars().filter(|c| c.is_ascii_digit()).collect();
            if !clean.is_empty() && clean == *maybe_num {
                if let Ok(num) = clean.parse::<u32>() {
                    if media_file.track_number.is_none() {
                        media_file.track_number = Some(num);
                    }
                    title_part = &filename_sans_ext[first_space + 1..];
                }
            }
        }

        media_file.title = Some(title_part.trim().to_string());
    }
}
