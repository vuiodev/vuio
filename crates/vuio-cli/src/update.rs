use anyhow::{anyhow, Context, Result};
use serde::Deserialize;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Deserialize, Debug)]
struct GithubRelease {
    tag_name: String,
    assets: Vec<GithubAsset>,
}

#[derive(Deserialize, Debug)]
struct GithubAsset {
    name: String,
    browser_download_url: String,
}

/// Helper function to detect the correct asset name for the current platform
fn get_target_asset_name() -> Option<&'static str> {
    if cfg!(target_os = "linux") {
        if cfg!(target_arch = "x86_64") {
            Some("vuio-linux-x86_64.tar.gz")
        } else if cfg!(target_arch = "aarch64") {
            Some("vuio-linux-arm64.tar.gz")
        } else if cfg!(target_arch = "arm") {
            Some("vuio-linux-armv7.tar.gz")
        } else {
            None
        }
    } else if cfg!(target_os = "windows") {
        if cfg!(target_arch = "x86_64") {
            Some("vuio-windows-x86_64.exe")
        } else if cfg!(target_arch = "aarch64") {
            Some("vuio-windows-arm64.exe")
        } else {
            None
        }
    } else if cfg!(target_os = "macos") {
        if cfg!(target_arch = "x86_64") {
            Some("vuio-macos-x86_64.tar.gz")
        } else if cfg!(target_arch = "aarch64") {
            Some("vuio-macos-arm64.tar.gz")
        } else {
            None
        }
    } else {
        None
    }
}

/// Compare two versions. Returns true if `latest` is newer than `current`.
fn is_newer_version(current: &str, latest: &str) -> bool {
    let clean_current = current.trim_start_matches('v');
    let clean_latest = latest.trim_start_matches('v');

    let parts_cur: Vec<u32> = clean_current
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();
    let parts_lat: Vec<u32> = clean_latest
        .split('.')
        .filter_map(|s| s.parse().ok())
        .collect();

    for i in 0..std::cmp::max(parts_cur.len(), parts_lat.len()) {
        let cur = parts_cur.get(i).cloned().unwrap_or(0);
        let lat = parts_lat.get(i).cloned().unwrap_or(0);
        if lat > cur {
            return true;
        } else if cur > lat {
            return false;
        }
    }
    false
}

/// Run the update process.
pub async fn update_binary() -> Result<()> {
    let current_version = env!("CARGO_PKG_VERSION");
    println!("Current version: v{}", current_version);

    let asset_name = match get_target_asset_name() {
        Some(name) => name,
        None => return Err(anyhow!("Unsupported target platform or architecture")),
    };
    println!("Detected platform asset: {}", asset_name);

    let current_exe = env::current_exe().context("Failed to get current executable path")?;
    println!("Current executable path: {}", current_exe.display());

    // Create reqwest client with User-Agent header (required by GitHub API)
    let client = reqwest::Client::builder()
        .user_agent(format!("vuio-updater/{}", current_version))
        .build()
        .context("Failed to build HTTP client")?;

    println!("Checking for latest release on GitHub...");
    let response = client
        .get("https://api.github.com/repos/vuiodev/vuio/releases/latest")
        .send()
        .await
        .context("Failed to fetch latest release from GitHub API")?;

    if !response.status().is_success() {
        return Err(anyhow!(
            "Failed to fetch latest release (HTTP {}): {}",
            response.status(),
            response.text().await.unwrap_or_default()
        ));
    }

    let release: GithubRelease = response
        .json()
        .await
        .context("Failed to parse GitHub release JSON")?;

    println!("Latest release found: {}", release.tag_name);

    if !is_newer_version(current_version, &release.tag_name) {
        println!("VuIO is already up-to-date (v{}).", current_version);
        return Ok(());
    }

    println!("A newer version is available: {}", release.tag_name);

    let asset = release
        .assets
        .iter()
        .find(|a| a.name == asset_name)
        .ok_or_else(|| {
            anyhow!(
                "Could not find asset '{}' in the latest release",
                asset_name
            )
        })?;

    println!(
        "Downloading release from {} ...",
        asset.browser_download_url
    );
    let download_resp = client
        .get(&asset.browser_download_url)
        .send()
        .await
        .context("Failed to start download of release asset")?;

    if !download_resp.status().is_success() {
        return Err(anyhow!(
            "Failed to download asset (HTTP {}).",
            download_resp.status()
        ));
    }

    // Generate unique temporary directory in system temp
    let temp_dir_name = format!("vuio-update-{}", uuid::Uuid::new_v4());
    let temp_dir_path = env::temp_dir().join(temp_dir_name);
    fs::create_dir_all(&temp_dir_path).context("Failed to create temporary directory")?;

    let downloaded_file_path = temp_dir_path.join(&asset.name);

    // Save download
    let bytes = download_resp
        .bytes()
        .await
        .context("Failed to read downloaded bytes")?;
    fs::write(&downloaded_file_path, bytes).context("Failed to write downloaded file")?;
    println!(
        "Downloaded asset saved to {}",
        downloaded_file_path.display()
    );

    let new_binary_path = if asset_name.ends_with(".tar.gz") {
        println!("Extracting archive using tar...");
        // Decompress using tar command line tool
        let output = Command::new("tar")
            .arg("-xzf")
            .arg(&downloaded_file_path)
            .arg("-C")
            .arg(&temp_dir_path)
            .output()
            .context("Failed to run 'tar' command. Please make sure tar is installed.")?;

        if !output.status.success() {
            let _ = fs::remove_dir_all(&temp_dir_path);
            return Err(anyhow!(
                "Failed to extract tar archive: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }

        let extracted_binary = temp_dir_path.join("vuio");
        if !extracted_binary.exists() {
            let _ = fs::remove_dir_all(&temp_dir_path);
            return Err(anyhow!("Extracted binary 'vuio' not found in archive"));
        }
        extracted_binary
    } else {
        // For Windows .exe files, the downloaded file is the binary
        downloaded_file_path
    };

    println!("Replacing current binary...");

    let old_exe_path = match install_new_binary(&new_binary_path, &current_exe) {
        Ok(old_exe_path) => old_exe_path,
        Err(e) => {
            let _ = fs::remove_dir_all(&temp_dir_path);
            return Err(e);
        }
    };

    // Clean up temporary files (except the .exe.old on Windows since it is locked until process exit)
    let _ = fs::remove_dir_all(&temp_dir_path);

    // Try to clean up the old file on Unix (on Windows it will be locked until exit, so we leave it)
    #[cfg(unix)]
    {
        let _ = fs::remove_file(&old_exe_path);
    }

    println!("Successfully updated to version {}!", release.tag_name);
    if cfg!(target_os = "windows") {
        println!(
            "Note: The old binary has been renamed to '{}'. You may delete it after exiting the application.",
            old_exe_path.display()
        );
    }

    Ok(())
}

/// Put `new_binary_path` in place of `current_exe`, returning where the
/// previous executable was moved to.
///
/// The new binary is staged as a sibling of the one it replaces rather than
/// moved straight out of the download directory. `rename` cannot cross a
/// filesystem, and the two are routinely on different ones — /tmp on tmpfs, or
/// an install on another volume — so the move used to fail with EXDEV *after*
/// the running executable had already been set aside, leaving the update to
/// roll back every time. Copying first makes the rename that matters a move
/// within one directory, which is both possible and atomic.
///
/// A failure before the backup rename leaves the installation untouched; one
/// after it rolls the backup into place.
fn install_new_binary(new_binary_path: &Path, current_exe: &Path) -> Result<PathBuf> {
    let staged_path = staging_path(current_exe);
    if staged_path.exists() {
        let _ = fs::remove_file(&staged_path);
    }
    fs::copy(new_binary_path, &staged_path).with_context(|| {
        format!(
            "Failed to stage the new binary at {}. The existing installation is untouched.",
            staged_path.display()
        )
    })?;

    // Permissions and durability on the staged copy, before it is anything the
    // system might execute.
    let staged = Staged(staged_path);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&staged.0)
            .context("Failed to read the staged binary's permissions")?
            .permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&staged.0, perms).context("Failed to set executable permissions")?;
    }
    if let Ok(file) = fs::File::open(&staged.0) {
        // Best effort: a machine that loses power mid-update should not come
        // back to an executable of the right length and the wrong contents.
        let _ = file.sync_all();
    }

    // Rename current running executable to a .old backup.
    let old_exe_path = if cfg!(target_os = "windows") {
        current_exe.with_extension("exe.old")
    } else {
        current_exe.with_extension("old")
    };
    if old_exe_path.exists() {
        fs::remove_file(&old_exe_path).context("Failed to remove old backup executable")?;
    }
    fs::rename(current_exe, &old_exe_path)
        .context("Failed to rename current executable to backup path")?;

    // Same directory, so this cannot fail the way the cross-filesystem move did.
    if let Err(e) = fs::rename(&staged.0, current_exe) {
        println!("Error replacing binary: {e}. Attempting rollback...");
        if let Err(rollback_err) = fs::rename(&old_exe_path, current_exe) {
            println!("CRITICAL: Rollback failed: {rollback_err}");
        }
        return Err(e).context("Failed to move new binary to current executable path");
    }
    std::mem::forget(staged); // Renamed into place; there is nothing left to clean up.

    Ok(old_exe_path)
}

/// Removes the staged file on any path out of [`install_new_binary`] that does
/// not rename it into place, so a failed update leaves no half-written binary
/// beside the real one.
struct Staged(PathBuf);

impl Drop for Staged {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// Where the downloaded binary is assembled: a sibling of the executable it
/// replaces, so the move into place is a rename within one directory.
///
/// Named after the target with a unique suffix. Two updates running at once
/// would otherwise stage over each other, and a leftover from a crashed run
/// must not be mistaken for this run's download.
fn staging_path(current_exe: &Path) -> PathBuf {
    let name = current_exe
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "vuio".to_owned());
    current_exe.with_file_name(format!(".{name}.new-{}", uuid::Uuid::new_v4()))
}

#[cfg(test)]
mod tests {
    use super::{is_newer_version, staging_path};
    use std::path::Path;

    #[test]
    fn compares_release_versions() {
        assert!(is_newer_version("0.0.33", "0.0.34"));
        assert!(is_newer_version("0.0.33", "0.1.0"));
        assert!(is_newer_version("v0.0.33", "1.0.0"));
        assert!(!is_newer_version("0.0.33", "0.0.33"));
        assert!(!is_newer_version("1.0.0", "0.9.9"));
    }

    /// The new binary is staged beside the one it replaces, never in the system
    /// temporary directory: `rename` cannot cross a filesystem, and /tmp on
    /// tmpfs or an install on another volume made the move fail with EXDEV
    /// after the old binary had already been set aside.
    #[test]
    fn the_new_binary_is_staged_beside_the_one_it_replaces() {
        let current = Path::new("/opt/vuio/bin/vuio");
        let staged = staging_path(current);
        assert_eq!(staged.parent(), current.parent());
        assert_ne!(staged, current);
        assert!(staged
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with(".vuio.new-"));

        // Two runs never pick the same name.
        assert_ne!(staging_path(current), staged);
    }

    /// Installing from a directory that is not the executable's own has to
    /// work: that is every real update, since the download lands in the system
    /// temporary directory. The old binary is kept, the new one is in place and
    /// executable, and nothing is left staged beside it.
    #[test]
    fn a_binary_is_installed_from_another_directory() {
        let install = tempfile::TempDir::new().expect("install dir");
        let download = tempfile::TempDir::new().expect("download dir");

        let current_exe = install.path().join("vuio");
        std::fs::write(&current_exe, b"old binary").expect("write current");
        let downloaded = download.path().join("vuio");
        std::fs::write(&downloaded, b"new binary").expect("write downloaded");

        let old_exe_path = super::install_new_binary(&downloaded, &current_exe).expect("install");

        assert_eq!(
            std::fs::read(&current_exe).expect("read installed"),
            b"new binary"
        );
        assert_eq!(
            std::fs::read(&old_exe_path).expect("read backup"),
            b"old binary"
        );
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = std::fs::metadata(&current_exe)
                .expect("stat")
                .permissions()
                .mode();
            assert_eq!(
                mode & 0o777,
                0o755,
                "the installed binary must be executable"
            );
        }
        assert!(
            leftovers(install.path()).is_empty(),
            "nothing staged should survive a successful install: {:?}",
            leftovers(install.path())
        );
        // The discriminating assertion, and the reason this bug existed: the
        // step that crosses a directory boundary must be a copy. A rename there
        // consumes the download — which is what this checks for — and fails
        // outright with EXDEV when the boundary is also a filesystem boundary,
        // as it is whenever /tmp is on tmpfs or the install is on another
        // volume. Those two conditions cannot be arranged portably in a test;
        // "the download survives" can, and it holds exactly when the move does.
        assert!(
            downloaded.exists(),
            "the download must be copied into place, not renamed across filesystems"
        );
    }

    /// A download that has gone missing must leave the installation alone —
    /// not moved aside and not replaced.
    #[test]
    fn a_failed_install_leaves_the_existing_binary_in_place() {
        let install = tempfile::TempDir::new().expect("install dir");
        let current_exe = install.path().join("vuio");
        std::fs::write(&current_exe, b"old binary").expect("write current");

        let missing = install.path().join("nowhere").join("vuio");
        assert!(super::install_new_binary(&missing, &current_exe).is_err());

        assert_eq!(
            std::fs::read(&current_exe).expect("read current"),
            b"old binary"
        );
        assert!(leftovers(install.path()).is_empty());
    }

    /// Files beside the executable that the install did not mean to leave.
    fn leftovers(directory: &Path) -> Vec<String> {
        std::fs::read_dir(directory)
            .expect("read dir")
            .filter_map(|entry| {
                let name = entry.ok()?.file_name().to_string_lossy().into_owned();
                (name != "vuio" && name != "vuio.old" && name != "vuio.exe.old").then_some(name)
            })
            .collect()
    }
}
