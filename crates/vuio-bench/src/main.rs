//! Generates a large VuIO library, for performance work.
//!
//! The point of this tool is a database that is the right *shape*, not merely the
//! right size. An earlier version wrote its own copy of the schema and its own
//! directory tree, and the result measured very little: the full-text tables were
//! never populated, the `directories` rows did not correspond to any file's
//! `parent_path`, and the paths on disk did not match the paths in the rows. A
//! server pointed at that database browsed an empty tree and searched an empty
//! index, so the numbers described a system nobody runs.
//!
//! So the schema comes from `vuio-core` itself, and every piece of derived state —
//! the directory tree, the recursive per-family counters, both FTS indexes — is
//! built by the same `rebuild_derived_indexes` the server uses to repair itself.
//! What is left here is only the part that has to be fast: the rows.

use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result};
use clap::Parser;
use vuio_core::config::generator::ConfigGenerator;
use vuio_core::config::{AppConfig, MonitoredDirectoryConfig, ValidationMode};
use vuio_core::database::sqlite::SqliteDatabase;
use vuio_core::database::{DatabaseManager, HealthRepository};

/// How many rows go in one transaction. Large enough that commit overhead
/// disappears, small enough that the rollback journal stays bounded.
const CHUNK: usize = 50_000;

const GENRES: [&str; 8] = [
    "Rock",
    "Pop",
    "Jazz",
    "Classical",
    "Electronic",
    "Metal",
    "Hip Hop",
    "Ambient",
];

/// Shape of the generated tree. Real libraries are wide and shallow rather than
/// uniformly deep, and the directory counters are maintained per ancestor, so
/// these two numbers decide how much work a scan does per file.
const TRACKS_PER_ALBUM: usize = 12;
const ALBUMS_PER_ARTIST: usize = 10;

#[derive(Parser, Debug)]
#[command(about = "Generate a large VuIO library for performance testing")]
struct Args {
    /// How many media rows to index
    #[arg(long, default_value_t = 100_000)]
    objects: usize,

    /// Where to put the library. The database goes in `<out>/.vuio/media.db`.
    #[arg(long, default_value = "./bench-library")]
    out: PathBuf,

    /// How many stub files to write to disk. Defaults to one per row.
    ///
    /// Every row needs a file, or the first scan deletes it: a library where
    /// most rows are missing measures a deletion storm, not steady state. Lower
    /// this only when the deletion path is deliberately what you want to time.
    #[arg(long)]
    files: Option<usize>,
}

/// The 18 bytes that make a file a plausible MP3 to a tag reader.
const STUB_MP3: &[u8] = b"ID3\x04\x00\x00\x00\x00\x00\x00\xFF\xFB\x90\x44\x00\x00\x00";

#[tokio::main]
async fn main() -> Result<()> {
    let args = Args::parse();
    let files_to_write = args.files.unwrap_or(args.objects).min(args.objects);

    let root = args.out.clone();
    std::fs::create_dir_all(&root)
        .with_context(|| format!("could not create {}", root.display()))?;
    // Canonical, because that is the form the scanner stores and compares against.
    let root = root
        .canonicalize()
        .with_context(|| format!("could not canonicalize {}", root.display()))?;
    let db_path = root.join(".vuio").join("media.db");

    println!("VuIO benchmark library");
    println!("  objects:  {}", args.objects);
    println!("  files:    {files_to_write} written to disk");
    println!("  library:  {}", root.display());
    println!("  database: {}", db_path.display());
    println!();

    // 1. Stub files, at the paths the rows will carry. Their modification times
    //    come back with them: a row whose `modified_secs` does not match its
    //    file reads as changed, and the first scan would re-read and rewrite the
    //    entire library from stubs that carry no tags.
    let started = Instant::now();
    let mtimes = write_stub_files(&root, files_to_write)?;
    println!(
        "  wrote {files_to_write} files in {:.1}s",
        started.elapsed().as_secs_f64()
    );

    // 2. The real schema, from the real code. Creating the database through
    //    vuio-core is what guarantees this tool cannot drift from `schema.rs`.
    if db_path.exists() {
        for suffix in ["", "-wal", "-shm"] {
            let _ = std::fs::remove_file(format!("{}{suffix}", db_path.display()));
        }
    }
    std::fs::create_dir_all(db_path.parent().expect("the database has a parent"))?;
    {
        let database = SqliteDatabase::new(db_path.clone())
            .await
            .context("could not create the database")?;
        database
            .initialize()
            .await
            .context("could not initialize the schema")?;
    }
    println!("  schema created by vuio-core");

    // 3. The rows, as fast as SQLite will take them.
    let started = Instant::now();
    insert_rows(&db_path, &root, args.objects, &mtimes)?;
    let inserted = started.elapsed();
    println!(
        "  inserted {} rows in {:.1}s ({:.0} rows/s)",
        args.objects,
        inserted.as_secs_f64(),
        args.objects as f64 / inserted.as_secs_f64().max(0.001)
    );

    // 4. Every piece of derived state, built by the server's own repair path:
    //    the directory tree, the recursive counters, and both FTS indexes.
    let started = Instant::now();
    {
        let database = SqliteDatabase::new(db_path.clone())
            .await
            .context("could not reopen the database")?;
        let health = database
            .rebuild_derived_indexes()
            .await
            .context("could not build the derived indexes")?;
        anyhow::ensure!(
            health.is_healthy,
            "derived index rebuild reported problems: {:?}",
            health.issues
        );
    }
    println!(
        "  built directory tree, counters and search index in {:.1}s",
        started.elapsed().as_secs_f64()
    );

    // 5. A config that points the server at this library *and* this database.
    //    Without it the server would use its own default database path, scan the
    //    tree from scratch, and never open the file we just built — which is
    //    a silent way to measure nothing.
    let config_path = write_config(&root, &db_path)?;

    report(&db_path)?;

    println!();
    println!("Run the server against it:");
    println!("  ./target/debug/vuio --config {}", config_path.display());
    Ok(())
}

/// Write a config naming this library and this database.
fn write_config(root: &Path, db_path: &Path) -> Result<PathBuf> {
    let mut config = AppConfig::default_for_platform();
    config.server.port = 18080;
    config.media.directories = vec![MonitoredDirectoryConfig {
        path: root.to_string_lossy().into_owned(),
        recursive: true,
        case_sensitive: None,
        extensions: None,
        exclude_patterns: None,
        validation_mode: ValidationMode::Warn,
    }];
    config.database.path = Some(db_path.to_string_lossy().into_owned());
    // The web interface would bind a second port and add noise to the profile.
    config.web_ui.enabled = false;

    let rendered = ConfigGenerator::new()
        .context("could not build the config generator")?
        .generate_config(&config)
        .context("could not render the config")?;
    let config_path = root.join("vuio.toml");
    std::fs::write(&config_path, rendered)
        .with_context(|| format!("could not write {}", config_path.display()))?;
    Ok(config_path)
}

/// The path a row with this index carries, relative to the library root.
///
/// One function so the rows and the files on disk cannot disagree — which is
/// exactly how the previous generator ended up indexing paths that did not exist.
fn relative_path(index: usize) -> (String, String) {
    let album = index / TRACKS_PER_ALBUM;
    let artist = album / ALBUMS_PER_ARTIST;
    let track = index % TRACKS_PER_ALBUM + 1;
    (
        format!("Artist_{artist:05}/Album_{album:06}"),
        format!("{track:02} - Track_{index}.mp3"),
    )
}

/// Write the stubs and return each one's modification time, in row order.
fn write_stub_files(root: &Path, count: usize) -> Result<Vec<i64>> {
    let mut mtimes = Vec::with_capacity(count);
    let mut last_dir = String::new();
    for index in 0..count {
        let (directory, filename) = relative_path(index);
        if directory != last_dir {
            std::fs::create_dir_all(root.join(&directory))?;
            last_dir = directory.clone();
        }
        let path = root.join(&directory).join(&filename);
        std::fs::write(&path, STUB_MP3)?;
        mtimes.push(
            std::fs::metadata(&path)?
                .modified()?
                .duration_since(std::time::UNIX_EPOCH)
                .map(|since| since.as_secs() as i64)
                .unwrap_or(0),
        );
    }
    Ok(mtimes)
}

fn insert_rows(db_path: &Path, root: &Path, total: usize, mtimes: &[i64]) -> Result<()> {
    let mut connection = rusqlite::Connection::open(db_path)?;
    // Two of the browse indexes are declared `COLLATE natural_order`, so this
    // connection has to know the collation before it can maintain them.
    vuio_core::database::sqlite::register_collations(&connection)?;
    // Bulk-load settings, reverted below. Safe here because the file is
    // disposable: if this crashes, you regenerate it.
    connection.execute_batch(
        "PRAGMA synchronous = OFF;
         PRAGMA journal_mode = MEMORY;
         PRAGMA cache_size = -131072;
         PRAGMA temp_store = MEMORY;",
    )?;

    let root = root.to_string_lossy().into_owned();
    let mut done = 0usize;
    while done < total {
        let end = (done + CHUNK).min(total);
        let transaction = connection.transaction()?;
        {
            let mut statement = transaction.prepare_cached(INSERT_MEDIA)?;
            for index in done..end {
                let (directory, filename) = relative_path(index);
                let parent = format!("{root}/{directory}");
                let path = format!("{parent}/{filename}");
                let album_id = index / TRACKS_PER_ALBUM;
                let artist_id = album_id / ALBUMS_PER_ARTIST;
                let track = (index % TRACKS_PER_ALBUM + 1) as i64;
                let year = 1980 + (index % 45) as i64;
                // The file's own mtime where one exists, so the scanner sees an
                // unchanged record; a fixed past value otherwise.
                let stamp = mtimes.get(index).copied().unwrap_or(1_700_000_000);
                statement.execute(rusqlite::params![
                    (index + 1) as i64,
                    path,
                    parent,
                    filename,
                    STUB_MP3.len() as i64,
                    stamp,
                    "audio/mpeg",
                    "audio",
                    215.0 + (index % 60) as f64,
                    format!("Track {index}"),
                    format!("Artist {artist_id}"),
                    format!("Album {album_id}"),
                    GENRES[index % GENRES.len()],
                    track,
                    year,
                    format!("Artist {artist_id}"),
                    1i64,
                    1i64,
                    TRACKS_PER_ALBUM as i64,
                    format!("Composer {}", artist_id % 97),
                    Option::<String>::None,
                    120i64,
                    0i64,
                    Option::<String>::None,
                    Option::<String>::None,
                    Option::<String>::None,
                    format!("{year}-01-01"),
                    Option::<String>::None,
                    Option::<String>::None,
                    Option::<String>::None,
                    "mp3",
                    44_100i64,
                    2i64,
                    16i64,
                    320_000i64,
                    1i64,
                    0i64,
                    stamp,
                    stamp,
                ])?;
            }
        }
        transaction.commit()?;
        done = end;
    }

    // Back to the settings the server runs with, so what gets measured next is
    // the real thing.
    connection.execute_batch(
        "PRAGMA journal_mode = WAL;
         PRAGMA synchronous = NORMAL;
         ANALYZE;",
    )?;
    Ok(())
}

/// Columns in the order `bind_media_file` uses, so the parameter list here reads
/// the same as the one in `media_repo/bulk.rs`.
const INSERT_MEDIA: &str = "\
INSERT INTO media_files (
    id, path, parent_path, filename, size, modified_secs, mime_type, mime_family,
    duration_secs, title, artist, album, genre, track_number, year, album_artist,
    disc_number, disc_total, track_total, composer, comment, bpm, compilation,
    sort_title, sort_artist, sort_album, release_date,
    musicbrainz_track_id, musicbrainz_album_id, musicbrainz_artist_id,
    codec, sample_rate, channels, bits_per_sample, bit_rate, tags_version,
    subtitle_available, created_at_secs, updated_at_secs
) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17,
          ?18, ?19, ?20, ?21, ?22, ?23, ?24, ?25, ?26, ?27, ?28, ?29, ?30, ?31, ?32,
          ?33, ?34, ?35, ?36, ?37, ?38, ?39)";

/// Prove the database is usable, not merely large.
///
/// Every count here was zero or wrong in the previous generator's output, which
/// is why they are printed rather than assumed.
fn report(db_path: &Path) -> Result<()> {
    let connection = rusqlite::Connection::open(db_path)?;
    let count = |sql: &str| -> Result<i64> { Ok(connection.query_row(sql, [], |row| row.get(0))?) };

    let files = count("SELECT COUNT(*) FROM media_files")?;
    let directories = count("SELECT COUNT(*) FROM directories")?;
    let counters = count("SELECT COUNT(*) FROM directory_mime_counts")?;
    let searchable = count("SELECT COUNT(*) FROM media_fts")?;
    let bytes = std::fs::metadata(db_path).map(|m| m.len()).unwrap_or(0);

    println!();
    println!("  media_files            {files}");
    println!("  directories            {directories}");
    println!("  directory_mime_counts  {counters}");
    println!("  media_fts              {searchable}");
    println!(
        "  database               {:.1} MB",
        bytes as f64 / 1_048_576.0
    );

    anyhow::ensure!(directories > 0, "the directory tree is empty");
    anyhow::ensure!(
        searchable == files,
        "the search index covers {searchable} of {files} rows"
    );
    Ok(())
}
