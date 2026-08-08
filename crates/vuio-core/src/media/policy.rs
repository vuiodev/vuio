use super::*;

#[derive(Debug, Clone)]
pub struct ScanPolicy {
    pub root: PathBuf,
    pub recursive: bool,
    pub case_sensitive: bool,
    extensions: HashSet<String>,
    exclude_patterns: Vec<String>,
    pub scan_playlists: bool,
}

impl ScanPolicy {
    pub fn from_config(config: &AppConfig, directory: &MonitoredDirectoryConfig) -> Self {
        let extensions = directory
            .extensions
            .as_ref()
            .unwrap_or(&config.media.supported_extensions)
            .iter()
            .map(|extension| extension.trim_start_matches('.').to_ascii_lowercase())
            .filter(|extension| !extension.is_empty())
            .collect();
        Self {
            root: PathBuf::from(&directory.path),
            recursive: directory.recursive,
            case_sensitive: directory.case_sensitive.unwrap_or_else(|| {
                detect_case_sensitivity(Path::new(&directory.path)).unwrap_or_else(|| {
                    let fallback = !cfg!(target_os = "windows");
                    warn!(
                        "Could not detect case behavior for {}; using {} fallback",
                        directory.path,
                        if fallback {
                            "case-sensitive"
                        } else {
                            "case-insensitive"
                        }
                    );
                    fallback
                })
            }),
            extensions,
            exclude_patterns: directory.exclude_patterns.clone().unwrap_or_default(),
            scan_playlists: config.media.scan_playlists,
        }
    }

    pub fn platform_default(root: &Path, recursive: bool) -> Self {
        Self {
            root: root.to_path_buf(),
            recursive,
            case_sensitive: detect_case_sensitivity(root).unwrap_or(!cfg!(target_os = "windows")),
            extensions: crate::platform::filesystem::get_supported_extensions()
                .iter()
                .map(|extension| extension.to_ascii_lowercase())
                .collect(),
            exclude_patterns: Vec::new(),
            scan_playlists: false,
        }
    }

    pub fn policies(config: &AppConfig) -> Vec<Self> {
        config
            .media
            .directories
            .iter()
            .map(|directory| Self::from_config(config, directory))
            .collect()
    }

    pub fn for_subtree(&self, root: &Path) -> Self {
        let mut policy = self.clone();
        policy.root = root.to_path_buf();
        policy
    }

    pub fn for_path<'a>(policies: &'a [Self], path: &Path) -> Option<&'a Self> {
        policies
            .iter()
            .filter(|policy| policy.path_starts_with(path, &policy.root))
            .max_by_key(|policy| policy.root.components().count())
    }

    pub fn contains(&self, path: &Path) -> bool {
        if !self.path_starts_with(path, &self.root) {
            return false;
        }
        self.recursive
            || path
                .parent()
                .is_some_and(|parent| self.paths_equal(parent, &self.root))
    }

    pub fn allows_media(&self, path: &Path) -> bool {
        self.contains(path)
            && !self.is_excluded(path)
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| self.extensions.contains(&extension.to_ascii_lowercase()))
    }

    pub fn allows_playlist(&self, path: &Path) -> bool {
        self.scan_playlists
            && self.contains(path)
            && !self.is_excluded(path)
            && path
                .extension()
                .and_then(|extension| extension.to_str())
                .is_some_and(|extension| {
                    matches!(
                        extension.to_ascii_lowercase().as_str(),
                        "m3u" | "m3u8" | "pls"
                    )
                })
    }

    pub fn allows_watched_path(&self, path: &Path) -> bool {
        if path.is_dir() {
            return self.paths_equal(path, &self.root)
                || (self.recursive && self.path_starts_with(path, &self.root));
        }
        self.allows_media(path)
            || self.allows_playlist(path)
            || (self.contains(path)
                && !self.is_excluded(path)
                && path
                    .extension()
                    .and_then(|extension| extension.to_str())
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("srt")))
    }

    fn is_excluded(&self, path: &Path) -> bool {
        let skip = if self.path_starts_with(path, &self.root) {
            self.root.components().count()
        } else {
            0
        };
        path.components().skip(skip).any(|component| {
            let value = component.as_os_str().to_string_lossy();
            self.exclude_patterns
                .iter()
                .any(|pattern| wildcard_match(pattern, &value, self.case_sensitive))
        })
    }

    fn paths_equal(&self, left: &Path, right: &Path) -> bool {
        path_components_equal(left, right, self.case_sensitive)
    }

    fn path_starts_with(&self, path: &Path, root: &Path) -> bool {
        let mut path_components = path.components();
        root.components().all(|root_component| {
            path_components.next().is_some_and(|path_component| {
                component_equal(path_component, root_component, self.case_sensitive)
            })
        })
    }
}

pub(super) fn wildcard_match(pattern: &str, value: &str, case_sensitive: bool) -> bool {
    let (pattern, value) = if !case_sensitive {
        (pattern.to_ascii_lowercase(), value.to_ascii_lowercase())
    } else {
        (pattern.to_owned(), value.to_owned())
    };
    let pattern = pattern.as_bytes();
    let value = value.as_bytes();
    let (mut p, mut v, mut star, mut checkpoint) = (0, 0, None, 0);
    while v < value.len() {
        if p < pattern.len() && (pattern[p] == b'?' || pattern[p] == value[v]) {
            p += 1;
            v += 1;
        } else if p < pattern.len() && pattern[p] == b'*' {
            star = Some(p);
            p += 1;
            checkpoint = v;
        } else if let Some(star_position) = star {
            p = star_position + 1;
            checkpoint += 1;
            v = checkpoint;
        } else {
            return false;
        }
    }
    while p < pattern.len() && pattern[p] == b'*' {
        p += 1;
    }
    p == pattern.len()
}

pub(super) fn component_equal(
    left: std::path::Component<'_>,
    right: std::path::Component<'_>,
    case_sensitive: bool,
) -> bool {
    if case_sensitive {
        left == right
    } else {
        left.as_os_str()
            .to_string_lossy()
            .eq_ignore_ascii_case(&right.as_os_str().to_string_lossy())
    }
}

pub(super) fn path_components_equal(left: &Path, right: &Path, case_sensitive: bool) -> bool {
    let mut left = left.components();
    let mut right = right.components();
    loop {
        match (left.next(), right.next()) {
            (None, None) => return true,
            (Some(a), Some(b)) if component_equal(a, b, case_sensitive) => {}
            _ => return false,
        }
    }
}

/// Detect case behavior without writing to a monitored root. We change the
/// ASCII case of one existing path component and ask the filesystem whether it
/// resolves to the same canonical object.
pub(super) fn detect_case_sensitivity(root: &Path) -> Option<bool> {
    let canonical = std::fs::canonicalize(root).ok()?;
    for current in canonical.ancestors() {
        let Some(name) = current.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        let Some(swapped) = swap_one_ascii_case(name) else {
            continue;
        };
        let mut candidate = current.to_path_buf();
        candidate.set_file_name(swapped);
        match std::fs::canonicalize(candidate) {
            Ok(other) => return Some(other != current),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Some(true),
            Err(_) => continue,
        }
    }
    None
}

pub(super) fn swap_one_ascii_case(value: &str) -> Option<String> {
    let mut bytes = value.as_bytes().to_vec();
    let byte = bytes.iter_mut().find(|byte| byte.is_ascii_alphabetic())?;
    *byte = if byte.is_ascii_lowercase() {
        byte.to_ascii_uppercase()
    } else {
        byte.to_ascii_lowercase()
    };
    String::from_utf8(bytes).ok()
}

#[derive(Debug)]
pub(super) struct TraversalReport {
    pub(super) file_paths: Vec<PathBuf>,
    pub(super) uncertain_prefixes: Vec<PathBuf>,
    pub(super) errors: Vec<ScanError>,
    pub(super) root_complete: bool,
}
