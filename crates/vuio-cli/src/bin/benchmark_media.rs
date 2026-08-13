use anyhow::{Context, Result};
use std::cmp::Ordering;
use std::fs;
use std::path::PathBuf;
use std::time::Instant;

fn natural_cmp(left: &str, right: &str) -> Ordering {
    let left = left.to_lowercase();
    let right = right.to_lowercase();
    let mut left_chars = left.chars().peekable();
    let mut right_chars = right.chars().peekable();

    loop {
        match (left_chars.peek(), right_chars.peek()) {
            (Some(a), Some(b)) if a.is_ascii_digit() && b.is_ascii_digit() => {
                let left_number: String = std::iter::from_fn(|| {
                    left_chars.next_if(|character| character.is_ascii_digit())
                })
                .collect();
                let right_number: String = std::iter::from_fn(|| {
                    right_chars.next_if(|character| character.is_ascii_digit())
                })
                .collect();
                let order = left_number
                    .trim_start_matches('0')
                    .len()
                    .cmp(&right_number.trim_start_matches('0').len())
                    .then_with(|| {
                        left_number
                            .trim_start_matches('0')
                            .cmp(right_number.trim_start_matches('0'))
                    })
                    .then_with(|| left_number.len().cmp(&right_number.len()));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(_), Some(_)) => {
                let order = left_chars.next().cmp(&right_chars.next());
                if order != Ordering::Equal {
                    return order;
                }
            }
            _ => return left_chars.next().cmp(&right_chars.next()),
        }
    }
}

const DDL_BASE: &str = r#"
CREATE TABLE IF NOT EXISTS media_files (
    id                 INTEGER PRIMARY KEY,
    path               TEXT    NOT NULL,
    parent_path        TEXT    NOT NULL,
    filename           TEXT    NOT NULL,
    size               INTEGER NOT NULL,
    modified_secs      INTEGER NOT NULL,
    mime_type          TEXT    NOT NULL,
    mime_family        TEXT    NOT NULL,
    duration_secs      REAL,
    title              TEXT,
    artist             TEXT,
    album              TEXT,
    genre              TEXT,
    track_number       INTEGER,
    year               INTEGER,
    album_artist       TEXT,
    disc_number        INTEGER,
    disc_total         INTEGER,
    track_total        INTEGER,
    composer           TEXT,
    comment            TEXT,
    bpm                INTEGER,
    compilation        INTEGER,
    sort_title         TEXT,
    sort_artist        TEXT,
    sort_album         TEXT,
    release_date       TEXT,
    musicbrainz_track_id  TEXT,
    musicbrainz_album_id  TEXT,
    musicbrainz_artist_id TEXT,
    codec              TEXT,
    sample_rate        INTEGER,
    channels           INTEGER,
    bits_per_sample    INTEGER,
    bit_rate           INTEGER,
    tags_version       INTEGER NOT NULL DEFAULT 0,
    subtitle_available INTEGER NOT NULL DEFAULT 0,
    created_at_secs    INTEGER NOT NULL,
    updated_at_secs    INTEGER NOT NULL,
    track_sort         INTEGER GENERATED ALWAYS AS (COALESCE(track_number, 4294967296)) STORED,
    disc_sort          INTEGER GENERATED ALWAYS AS (COALESCE(disc_number, 1)) VIRTUAL
) STRICT;

CREATE TABLE IF NOT EXISTS directories (
    path        TEXT PRIMARY KEY,
    parent_path TEXT NOT NULL,
    name        TEXT NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS directory_mime_counts (
    dir_path TEXT    NOT NULL REFERENCES directories(path) ON DELETE CASCADE,
    family   TEXT    NOT NULL,
    count    INTEGER NOT NULL,
    PRIMARY KEY (dir_path, family)
) STRICT;

CREATE TABLE IF NOT EXISTS playlists (
    id              INTEGER PRIMARY KEY,
    name            TEXT    NOT NULL,
    description     TEXT,
    source_path     TEXT,
    created_at_secs INTEGER NOT NULL,
    updated_at_secs INTEGER NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS playlist_entries (
    playlist_id   INTEGER NOT NULL REFERENCES playlists(id)   ON DELETE CASCADE,
    media_file_id INTEGER NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    position      INTEGER NOT NULL,
    PRIMARY KEY (playlist_id, position)
) STRICT;

CREATE TABLE IF NOT EXISTS root_availability (
    path                   TEXT PRIMARY KEY,
    last_seen_secs         INTEGER NOT NULL,
    unavailable_since_secs INTEGER,
    indexed_count          INTEGER NOT NULL,
    reason                 TEXT    NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS secrets (
    key   TEXT PRIMARY KEY,
    value BLOB NOT NULL
) STRICT;

CREATE TABLE IF NOT EXISTS media_tags (
    media_file_id INTEGER NOT NULL REFERENCES media_files(id) ON DELETE CASCADE,
    key           TEXT    NOT NULL,
    value         TEXT    NOT NULL,
    PRIMARY KEY (media_file_id, key, value)
) STRICT;

CREATE TABLE IF NOT EXISTS mediainfo (
    media_file_id     INTEGER PRIMARY KEY REFERENCES media_files(id) ON DELETE CASCADE,
    provider          TEXT    NOT NULL,
    remote_id         TEXT    NOT NULL,
    kind              TEXT    NOT NULL,
    title             TEXT,
    original_title    TEXT,
    overview          TEXT,
    release_date      TEXT,
    year              INTEGER,
    rating            REAL,
    genres            TEXT,
    season            INTEGER,
    episode           INTEGER,
    artwork_key       TEXT,
    payload           TEXT    NOT NULL,
    confidence        INTEGER NOT NULL,
    fetched_at        INTEGER NOT NULL,
    mediainfo_version INTEGER NOT NULL
) STRICT;
"#;

const INDEXES_SQL: &str = r#"
CREATE UNIQUE INDEX IF NOT EXISTS idx_media_path ON media_files(path);
CREATE INDEX IF NOT EXISTS idx_media_dir_order
    ON media_files(parent_path, disc_sort, track_sort, filename COLLATE natural_order);
CREATE INDEX IF NOT EXISTS idx_media_dir_family
    ON media_files(parent_path, mime_family);
CREATE INDEX IF NOT EXISTS idx_media_album
    ON media_files(album, disc_sort, track_sort, filename COLLATE natural_order);
CREATE INDEX IF NOT EXISTS idx_media_artist       ON media_files(artist);
CREATE INDEX IF NOT EXISTS idx_media_genre        ON media_files(genre);
CREATE INDEX IF NOT EXISTS idx_media_year         ON media_files(year);
CREATE INDEX IF NOT EXISTS idx_media_album_artist ON media_files(album_artist);
CREATE INDEX IF NOT EXISTS idx_media_family       ON media_files(mime_family);
CREATE INDEX IF NOT EXISTS idx_media_tags_version ON media_files(tags_version);

CREATE INDEX IF NOT EXISTS idx_directories_parent
    ON directories(parent_path, name COLLATE natural_order);
CREATE INDEX IF NOT EXISTS idx_playlists_source
    ON playlists(source_path) WHERE source_path IS NOT NULL;
CREATE INDEX IF NOT EXISTS idx_playlist_entries_file
    ON playlist_entries(media_file_id);
CREATE INDEX IF NOT EXISTS idx_media_tags_key
    ON media_tags(key, value);
CREATE INDEX IF NOT EXISTS idx_mediainfo_confidence
    ON mediainfo(confidence);
"#;

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let total_objects: usize = args
        .get(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(10_000_000);

    let base_dir = std::env::current_dir()?;
    let test_media_dir = base_dir.join("test-media");
    let config_dir = base_dir.join("target").join("release").join("config");
    let db_dir = config_dir.join("database");
    let db_path = db_dir.join("media.db");

    println!("=== VuIO Benchmark Media Generator (Rust) ===");
    println!("Target Objects: {}", total_objects);
    println!("Test Media Dir: {}", test_media_dir.display());
    println!("Database Path:  {}", db_path.display());
    println!();

    // 1. Create physical test media directory and sample files
    println!("1. Creating physical test files in {}...", test_media_dir.display());
    fs::create_dir_all(&test_media_dir)?;

    let silent_mp3 = b"ID3\x04\x00\x00\x00\x00\x00\x00\x00\xFF\xFB\x90\x44\x00\x00\x00\x00";
    let mut physical_files_created = 0;
    for artist_idx in 0..10 {
        let artist_dir = test_media_dir.join(format!("Artist_{:02}", artist_idx));
        for album_idx in 0..10 {
            let album_dir = artist_dir.join(format!("Album_{:02}", album_idx));
            fs::create_dir_all(&album_dir)?;
            for track_idx in 1..=10 {
                let track_file = album_dir.join(format!("{:02} - Track_{}.mp3", track_idx, track_idx));
                if !track_file.exists() {
                    fs::write(&track_file, silent_mp3)?;
                    physical_files_created += 1;
                }
            }
        }
    }
    println!("   Created {} physical files in test-media.\n", physical_files_created);

    // 2. Prepare SQLite Database
    println!("2. Initializing SQLite database schema at {}...", db_path.display());
    fs::create_dir_all(&db_dir)?;

    // Remove existing database files
    for ext in &["", "-wal", "-shm"] {
        let p = PathBuf::from(format!("{}{}", db_path.display(), ext));
        if p.exists() {
            let _ = fs::remove_file(p);
        }
    }

    let mut conn = rusqlite::Connection::open(&db_path)
        .with_context(|| format!("Failed to open {}", db_path.display()))?;

    // Register natural_order collation
    conn.create_collation("natural_order", |a: &str, b: &str| {
        natural_cmp(a, b)
    })?;

    // Pragmas for fast bulk load
    conn.execute_batch(
        "PRAGMA synchronous = OFF;
         PRAGMA journal_mode = OFF;
         PRAGMA page_size = 4096;
         PRAGMA cache_size = -131072;
         PRAGMA temp_store = MEMORY;"
    )?;

    // Create DDL schema
    conn.execute_batch(DDL_BASE)?;
    conn.execute_batch("PRAGMA user_version = 4;")?;

    println!("   Schema and tables initialized.\n");

    // 3. Pre-populate directories table
    println!("3. Populating directory hierarchy in database...");
    let canonical_root = test_media_dir.canonicalize().unwrap_or_else(|_| test_media_dir.clone());
    let root_str = canonical_root.to_string_lossy().to_string();
    let root_parent = canonical_root
        .parent()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_default();
    let root_name = canonical_root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_default();

    {
        let tx = conn.transaction()?;
        tx.execute(
            "INSERT OR IGNORE INTO directories (path, parent_path, name) VALUES (?, ?, ?)",
            rusqlite::params![&root_str, &root_parent, &root_name],
        )?;

        let num_artists = 10_000.min(total_objects / 100).max(10);
        {
            let mut dir_stmt = tx.prepare_cached(
                "INSERT OR IGNORE INTO directories (path, parent_path, name) VALUES (?, ?, ?)",
            )?;

            for a in 0..num_artists {
                let art_name = format!("Artist_{:04}", a);
                let art_path = format!("{}/{}", root_str, art_name);
                dir_stmt.execute(rusqlite::params![&art_path, &root_str, &art_name])?;

                for alb in 0..5 {
                    let alb_id = a * 5 + alb;
                    let alb_name = format!("Album_{:05}", alb_id);
                    let alb_path = format!("{}/{}", art_path, alb_name);
                    dir_stmt.execute(rusqlite::params![&alb_path, &art_path, &alb_name])?;
                }
            }
        }

        tx.execute(
            "INSERT OR REPLACE INTO directory_mime_counts (dir_path, family, count) VALUES (?, '*', ?)",
            rusqlite::params![&root_str, total_objects as i64],
        )?;
        tx.execute(
            "INSERT OR REPLACE INTO directory_mime_counts (dir_path, family, count) VALUES (?, 'audio', ?)",
            rusqlite::params![&root_str, total_objects as i64],
        )?;

        tx.commit()?;
    }
    println!("   Directory hierarchy populated.\n");

    // 4. Bulk insert media files in transaction chunks
    println!("4. Inserting {} media files in chunks of 50,000...", total_objects);
    let start_insert = Instant::now();
    let chunk_size = 50_000;
    let num_artists = 10_000.max(1);
    let num_albums = 100_000.max(1);
    let genres = ["Rock", "Pop", "Jazz", "Classical", "Electronic", "Metal", "Hip Hop", "Ambient"];

    let insert_sql = "
        INSERT INTO media_files (
            id, path, parent_path, filename, size, modified_secs, mime_type, mime_family,
            duration_secs, title, artist, album, genre, track_number, year, album_artist,
            disc_number, disc_total, track_total, composer, comment, bpm, compilation,
            sort_title, sort_artist, sort_album, release_date, musicbrainz_track_id,
            musicbrainz_album_id, musicbrainz_artist_id, codec, sample_rate, channels,
            bits_per_sample, bit_rate, tags_version, subtitle_available, created_at_secs, updated_at_secs
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16,
                  ?17, ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30,
                  ?31, ?32, ?33, ?34, ?35, ?36, ?37, ?38, ?39)
    ";

    for chunk_start in (0..total_objects).step_by(chunk_size) {
        let chunk_end = (chunk_start + chunk_size).min(total_objects);
        let tx = conn.transaction()?;
        {
            let mut stmt = tx.prepare_cached(insert_sql)?;
            for i in chunk_start..chunk_end {
                let id = (i + 1) as i64;
                let artist_id = i % num_artists;
                let album_id = i % num_albums;
                let track_num = ((i % 20) + 1) as i64;
                let genre = genres[i % genres.len()];
                let parent = format!("{}/Artist_{:04}/Album_{:05}", root_str, artist_id, album_id);
                let filename = format!("{:02} - Track_{}.mp3", track_num, i);
                let path = format!("{}/{}", parent, filename);
                let title = format!("Track {}", i);
                let artist = format!("Artist {}", artist_id);
                let album = format!("Album {}", album_id);
                let year = (1980 + (i % 45)) as i64;
                let release_date = format!("{}-01-01", year);
                let size = (4_194_304 + (i % 1000) * 1024) as i64;
                let duration = 215.0 + (i % 60) as f64;
                let timestamp = 1700000000 + (i % 86400) as i64;

                stmt.execute(rusqlite::params![
                    id,
                    path,
                    parent,
                    filename,
                    size,
                    timestamp,
                    "audio/mpeg",
                    "audio",
                    duration,
                    title,
                    artist,
                    album,
                    genre,
                    track_num,
                    year,
                    artist,
                    1i64,
                    1i64,
                    20i64,
                    Option::<String>::None,
                    Option::<String>::None,
                    120i64,
                    0i64,
                    Option::<String>::None,
                    Option::<String>::None,
                    Option::<String>::None,
                    release_date,
                    Option::<String>::None,
                    Option::<String>::None,
                    Option::<String>::None,
                    "mp3",
                    44100i64,
                    2i64,
                    16i64,
                    320000i64,
                    1i64,
                    0i64,
                    timestamp,
                    timestamp,
                ])?;
            }
        }
        tx.commit()?;

        if chunk_end % 1_000_000 == 0 || chunk_end == total_objects {
            let elapsed = start_insert.elapsed().as_secs_f64();
            let rate = chunk_end as f64 / elapsed;
            println!(
                "   Inserted {:>10} / {} rows ({:>5.1}s, {:>8.0} rows/s)...",
                chunk_end, total_objects, elapsed, rate
            );
        }
    }

    let insert_duration = start_insert.elapsed();
    println!("   Inserted {} rows in {:.1}s.\n", total_objects, insert_duration.as_secs_f64());

    // 5. Create B-Tree Indexes
    println!("5. Creating B-Tree indexes on 10M records in SQLite...");
    let start_idx = Instant::now();
    conn.execute_batch(INDEXES_SQL)?;
    println!("   Indexes created in {:.1}s.\n", start_idx.elapsed().as_secs_f64());

    // 6. Finalize WAL and stats
    println!("6. Finalizing SQLite database settings...");
    conn.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         PRAGMA optimize;"
    )?;

    drop(conn);

    let metadata = fs::metadata(&db_path)?;
    let db_bytes = metadata.len();
    let db_mb = db_bytes as f64 / (1024.0 * 1024.0);
    let db_gb = db_bytes as f64 / (1024.0 * 1024.0 * 1024.0);

    println!("============================================================");
    println!("BENCHMARK LIBRARY READY!");
    println!("Total Records: {}", total_objects);
    println!("Database Size: {:.2} GB ({:.1} MB)", db_gb, db_mb);
    println!("============================================================");

    Ok(())
}
