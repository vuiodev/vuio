use super::{AppConfig, ConfigChangeEvent};
use std::{collections::HashSet, path::PathBuf};

pub(super) fn changes(old: &AppConfig, new: &AppConfig) -> Vec<ConfigChangeEvent> {
    let mut changes = vec![ConfigChangeEvent::Reloaded(Box::new(new.clone()))];
    let old_dirs = old
        .media
        .directories
        .iter()
        .map(|directory| PathBuf::from(&directory.path))
        .collect::<HashSet<_>>();
    let new_dirs = new
        .media
        .directories
        .iter()
        .map(|directory| PathBuf::from(&directory.path))
        .collect::<HashSet<_>>();
    let added = new_dirs.difference(&old_dirs).cloned().collect::<Vec<_>>();
    let removed = old_dirs.difference(&new_dirs).cloned().collect::<Vec<_>>();
    let modified = new
        .media
        .directories
        .iter()
        .filter_map(|new_directory| {
            old.media
                .directories
                .iter()
                .find(|old_directory| old_directory.path == new_directory.path)
                .filter(|old_directory| *old_directory != new_directory)
                .map(|_| PathBuf::from(&new_directory.path))
        })
        .collect::<Vec<_>>();
    if !added.is_empty() || !removed.is_empty() || !modified.is_empty() {
        changes.push(ConfigChangeEvent::DirectoriesChanged {
            added,
            removed,
            modified,
        });
    }
    if old.network.interface_selection != new.network.interface_selection
        || old.server.port != new.server.port
    {
        changes.push(ConfigChangeEvent::NetworkChanged {
            old_interface: old.network.interface_selection.clone(),
            new_interface: new.network.interface_selection.clone(),
            old_port: old.server.port,
            new_port: new.server.port,
        });
    }
    changes
}
