use std::env;
use std::fs;
use std::process::Command;
use anyhow::{anyhow, Result};
use serde::Deserialize;

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

    let client = crate::http_clients::updater()?;

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
        .ok_or_else(|| anyhow!("Could not find asset '{}' in the latest release", asset_name))?;

    println!("Downloading release from {} ...", asset.browser_download_url);
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
    let bytes = download_resp.bytes().await.context("Failed to read downloaded bytes")?;
    fs::write(&downloaded_file_path, bytes).context("Failed to write downloaded file")?;
    println!("Downloaded asset saved to {}", downloaded_file_path.display());

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

    // Rename current running executable to a .old backup
    let old_exe_path = if cfg!(target_os = "windows") {
        current_exe.with_extension("exe.old")
    } else {
        current_exe.with_extension("old")
    };

    if old_exe_path.exists() {
        fs::remove_file(&old_exe_path).context("Failed to remove old backup executable")?;
    }

    fs::rename(&current_exe, &old_exe_path)
        .context("Failed to rename current executable to backup path")?;

    // Copy/rename new binary to the original executable path
    if let Err(e) = fs::rename(&new_binary_path, &current_exe) {
        // Rollback on failure
        println!("Error replacing binary: {}. Attempting rollback...", e);
        if let Err(rollback_err) = fs::rename(&old_exe_path, &current_exe) {
            println!("CRITICAL: Rollback failed: {}", rollback_err);
        }
        let _ = fs::remove_dir_all(&temp_dir_path);
        return Err(e).context("Failed to move new binary to current executable path");
    }

    // On Unix, ensure the new binary is executable
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = fs::metadata(&current_exe)?.permissions();
        perms.set_mode(0o755);
        fs::set_permissions(&current_exe, perms).context("Failed to set executable permissions")?;
    }

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
