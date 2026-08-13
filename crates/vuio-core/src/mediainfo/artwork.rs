//! The poster cache.
//!
//! VuIO has never stored artwork: `serve_cover` re-reads a sidecar file or the
//! embedded tag on every request, which is fine when the bytes are already on
//! local disk and impossible when they are on someone else's server. Downloaded
//! posters therefore need somewhere to live, and it is not the database — a few
//! thousand JPEGs would multiply the size of a file that gets vacuumed, backed up
//! and copied around.
//!
//! Files are addressed by a hash of their source URL and sharded a byte deep, so a
//! large library does not produce a single directory with one entry per item.

use super::client::Fetcher;
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

/// Posters are a few hundred KiB; anything an order of magnitude past that is not
/// a poster and is not worth the disk.
const MAX_ARTWORK_BYTES: usize = 8 * 1024 * 1024;

#[derive(Clone)]
pub struct ArtworkCache {
    root: PathBuf,
}

impl ArtworkCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The cache key for a source URL: a hex digest, so it is a valid filename on
    /// every platform regardless of what the URL contained.
    pub fn key_for(url: &str) -> String {
        // FNV-1a, the same hash `ui.rs` uses for asset ETags. This is a cache
        // address, not a security boundary — the only cost of a collision is one
        // wrong thumbnail, and a 64-bit space makes that vanishingly unlikely.
        let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in url.as_bytes() {
            hash ^= *byte as u64;
            hash = hash.wrapping_mul(0x1000_0000_01b3);
        }
        format!("{hash:016x}")
    }

    /// Where a key's file lives, given the extension implied by its content type.
    fn path_for(&self, key: &str, extension: &str) -> PathBuf {
        self.root.join(&key[..2]).join(format!("{key}.{extension}"))
    }

    /// Find a cached file for `key`, whatever image type it was stored as.
    pub fn lookup(&self, key: &str) -> Option<PathBuf> {
        if key.len() < 2 {
            return None;
        }
        ["jpg", "png", "webp"]
            .iter()
            .map(|extension| self.path_for(key, extension))
            .find(|path| path.is_file())
    }

    /// Download `url` into the cache and return its key.
    ///
    /// Already-cached URLs are not re-fetched, which is what makes a second run of
    /// the library fetch cheap.
    pub async fn store(&self, http: &Fetcher, provider: &'static str, url: &str) -> Result<String> {
        let key = Self::key_for(url);
        if self.lookup(&key).is_some() {
            return Ok(key);
        }

        let (content_type, bytes) = http.get_image(provider, url, MAX_ARTWORK_BYTES).await?;
        let extension = match content_type.as_str() {
            "image/jpeg" | "image/jpg" => "jpg",
            "image/png" => "png",
            "image/webp" => "webp",
            // Refuse anything that is not an image we would serve back. A provider
            // handing us an HTML error page should not become a cached "poster".
            other => bail!("{provider}: artwork had unexpected content type {other:?}"),
        };
        if bytes.is_empty() {
            bail!("{provider}: artwork was empty");
        }

        let path = self.path_for(&key, extension);
        let parent = path
            .parent()
            .context("artwork cache path has no parent directory")?
            .to_path_buf();
        let bytes_to_write = bytes;
        let write_path = path.clone();
        tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            std::fs::create_dir_all(&parent)?;
            // Write beside the target and rename, so an interrupted download cannot
            // leave a truncated image that later reads would serve as valid.
            let temporary = write_path.with_extension("part");
            std::fs::write(&temporary, &bytes_to_write)?;
            std::fs::rename(&temporary, &write_path)
        })
        .await
        .context("artwork cache write task failed")?
        .context("Failed to write artwork to the cache")?;

        Ok(key)
    }
}

/// The content type to serve a cached file as, from its extension.
pub fn content_type_for(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
    {
        "png" => "image/png",
        "webp" => "image/webp",
        _ => "image/jpeg",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_key_is_stable_and_url_specific() {
        let one = ArtworkCache::key_for("https://example.test/a.jpg");
        assert_eq!(one, ArtworkCache::key_for("https://example.test/a.jpg"));
        assert_ne!(one, ArtworkCache::key_for("https://example.test/b.jpg"));
        assert_eq!(one.len(), 16);
        assert!(one.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn files_are_sharded_by_the_first_two_characters() {
        let cache = ArtworkCache::new("/tmp/artwork");
        let key = ArtworkCache::key_for("https://example.test/poster.jpg");
        let path = cache.path_for(&key, "jpg");
        assert_eq!(
            path,
            Path::new("/tmp/artwork").join(&key[..2]).join(format!("{key}.jpg"))
        );
    }

    #[test]
    fn lookup_finds_a_stored_file_of_any_supported_type() {
        let temp = tempfile::tempdir().unwrap();
        let cache = ArtworkCache::new(temp.path());
        let key = ArtworkCache::key_for("https://example.test/poster.png");
        assert!(cache.lookup(&key).is_none());

        let path = cache.path_for(&key, "png");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"bytes").unwrap();
        assert_eq!(cache.lookup(&key), Some(path));
    }

    #[test]
    fn content_type_follows_the_extension() {
        assert_eq!(content_type_for(Path::new("a/b.png")), "image/png");
        assert_eq!(content_type_for(Path::new("a/b.webp")), "image/webp");
        assert_eq!(content_type_for(Path::new("a/b.jpg")), "image/jpeg");
    }
}
