use crate::lockfile::Lockfile;
use semver::Version;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChangeSet {
    /// Mod ids present in `new` but not `old` (loader version bumps are
    /// tracked separately via `loader_changed`, not through this list).
    pub added: Vec<(String, String)>,
    /// Mod ids present in `old` but not `new`.
    pub removed: Vec<(String, String)>,
    /// Mod ids present in both, with a different version.
    pub changed: Vec<(String, String, String)>,
    /// Loader version change, if any: (old, new).
    pub loader_changed: Option<(Option<String>, String)>,
}

impl ChangeSet {
    pub fn is_empty(&self) -> bool {
        self.added.is_empty() && self.removed.is_empty() && self.changed.is_empty() && self.loader_changed.is_none()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BumpKind {
    Minor,
    Patch,
}

/// Compare two lockfiles and report what changed. Pure function, no I/O.
///
/// Per-mod/loader versions are compared as opaque strings (`!=` only, never
/// ordered) — that's deliberate: not every mod source has a real semver
/// (e.g. a Steam Workshop item's "version" is a synthesized update
/// timestamp), and equality is all this needs. Only the release's own
/// top-level version (`apply_bump` below) needs real semver.
pub fn diff_lockfiles(old: &Lockfile, new: &Lockfile) -> ChangeSet {
    let mut added = Vec::new();
    let mut removed = Vec::new();
    let mut changed = Vec::new();

    for (id, new_entry) in &new.mods {
        match old.mods.get(id) {
            None => added.push((id.clone(), new_entry.version.clone())),
            Some(old_entry) if old_entry.version != new_entry.version => {
                changed.push((id.clone(), old_entry.version.clone(), new_entry.version.clone()))
            }
            Some(_) => {}
        }
    }
    for (id, old_entry) in &old.mods {
        if !new.mods.contains_key(id) {
            removed.push((id.clone(), old_entry.version.clone()));
        }
    }
    added.sort_by(|a, b| a.0.cmp(&b.0));
    removed.sort_by(|a, b| a.0.cmp(&b.0));
    changed.sort_by(|a, b| a.0.cmp(&b.0));

    let loader_changed = match (&old.loader, &new.loader) {
        (None, Some(new_entry)) => Some((None, new_entry.version.clone())),
        (Some(old_entry), Some(new_entry)) if old_entry.version != new_entry.version => {
            Some((Some(old_entry.version.clone()), new_entry.version.clone()))
        }
        _ => None,
    };

    ChangeSet {
        added,
        removed,
        changed,
        loader_changed,
    }
}

/// Added/removed mods => minor bump. Only version changes (mods or loader)
/// => patch bump. No changes => no release.
pub fn bump_kind(changes: &ChangeSet) -> Option<BumpKind> {
    if changes.is_empty() {
        None
    } else if !changes.added.is_empty() || !changes.removed.is_empty() {
        Some(BumpKind::Minor)
    } else {
        Some(BumpKind::Patch)
    }
}

/// Apply a bump to the release's own version, following standard semver
/// reset rules: a minor bump resets patch to 0, a patch bump only
/// increments patch. This is the one place real semver ordering matters.
pub fn apply_bump(version: &Version, bump: BumpKind) -> Version {
    let mut v = version.clone();
    match bump {
        BumpKind::Minor => {
            v.minor += 1;
            v.patch = 0;
        }
        BumpKind::Patch => {
            v.patch += 1;
        }
    }
    v.pre = semver::Prerelease::EMPTY;
    v.build = semver::BuildMetadata::EMPTY;
    v
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lockfile::LockedEntry;
    use std::collections::BTreeMap;

    fn entry(version: &str) -> LockedEntry {
        LockedEntry {
            version: version.to_string(),
            source_url: "https://example.com".into(),
            sha256: "deadbeef".into(),
        }
    }

    fn lockfile(loader: Option<&str>, mods: &[(&str, &str)]) -> Lockfile {
        let mut map = BTreeMap::new();
        for (id, version) in mods {
            map.insert(id.to_string(), entry(version));
        }
        Lockfile {
            loader: loader.map(entry),
            mods: map,
        }
    }

    #[test]
    fn no_changes_means_no_bump() {
        let old = lockfile(Some("5.4.0"), &[("mod-a", "1.0.0")]);
        let new = lockfile(Some("5.4.0"), &[("mod-a", "1.0.0")]);
        let changes = diff_lockfiles(&old, &new);
        assert!(changes.is_empty());
        assert_eq!(bump_kind(&changes), None);
    }

    #[test]
    fn added_mod_means_minor_bump() {
        let old = lockfile(Some("5.4.0"), &[("mod-a", "1.0.0")]);
        let new = lockfile(Some("5.4.0"), &[("mod-a", "1.0.0"), ("mod-b", "2.0.0")]);
        let changes = diff_lockfiles(&old, &new);
        assert_eq!(changes.added, vec![("mod-b".to_string(), "2.0.0".to_string())]);
        assert_eq!(bump_kind(&changes), Some(BumpKind::Minor));
    }

    #[test]
    fn removed_mod_means_minor_bump() {
        let old = lockfile(Some("5.4.0"), &[("mod-a", "1.0.0"), ("mod-b", "2.0.0")]);
        let new = lockfile(Some("5.4.0"), &[("mod-a", "1.0.0")]);
        let changes = diff_lockfiles(&old, &new);
        assert_eq!(changes.removed, vec![("mod-b".to_string(), "2.0.0".to_string())]);
        assert_eq!(bump_kind(&changes), Some(BumpKind::Minor));
    }

    #[test]
    fn version_change_means_patch_bump() {
        let old = lockfile(Some("5.4.0"), &[("mod-a", "1.0.0")]);
        let new = lockfile(Some("5.4.0"), &[("mod-a", "1.0.1")]);
        let changes = diff_lockfiles(&old, &new);
        assert_eq!(
            changes.changed,
            vec![("mod-a".to_string(), "1.0.0".to_string(), "1.0.1".to_string())]
        );
        assert_eq!(bump_kind(&changes), Some(BumpKind::Patch));
    }

    #[test]
    fn loader_version_change_means_patch_bump() {
        let old = lockfile(Some("5.4.0"), &[]);
        let new = lockfile(Some("5.4.1"), &[]);
        let changes = diff_lockfiles(&old, &new);
        assert!(changes.loader_changed.is_some());
        assert_eq!(bump_kind(&changes), Some(BumpKind::Patch));
    }

    #[test]
    fn non_semver_versions_still_diff_by_equality() {
        // e.g. a Steam Workshop item's synthesized "updated-<timestamp>" version.
        let old = lockfile(None, &[("workshop-mod", "updated-1000")]);
        let new = lockfile(None, &[("workshop-mod", "updated-2000")]);
        let changes = diff_lockfiles(&old, &new);
        assert_eq!(
            changes.changed,
            vec![("workshop-mod".to_string(), "updated-1000".to_string(), "updated-2000".to_string())]
        );
    }

    #[test]
    fn apply_bump_resets_patch_on_minor() {
        let v = Version::parse("1.4.7").unwrap();
        assert_eq!(apply_bump(&v, BumpKind::Minor), Version::parse("1.5.0").unwrap());
        assert_eq!(apply_bump(&v, BumpKind::Patch), Version::parse("1.4.8").unwrap());
    }
}
