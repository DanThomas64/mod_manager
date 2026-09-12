use std::path::{Path, PathBuf};

/// Executable names that indicate a folder is a Valheim install, one per OS.
const WINDOWS_MARKER: &str = "valheim.exe";
const LINUX_MARKER: &str = "valheim.x86_64";

/// True if `root` contains the Valheim executable for this OS. `exists` is
/// injected so this stays unit-testable without touching the real filesystem.
pub fn is_valid_valheim_root(root: &Path, exists: impl Fn(&Path) -> bool) -> bool {
    exists(&root.join(WINDOWS_MARKER)) || exists(&root.join(LINUX_MARKER))
}

/// Candidate Steam library `steamapps` roots to check for a `common/Valheim`
/// subfolder, before falling back to parsing `libraryfolders.vdf`.
#[cfg(windows)]
fn default_steamapps_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Ok(program_files_x86) = std::env::var("ProgramFiles(x86)") {
        roots.push(PathBuf::from(program_files_x86).join("Steam").join("steamapps"));
    }
    if let Ok(program_files) = std::env::var("ProgramFiles") {
        roots.push(PathBuf::from(program_files).join("Steam").join("steamapps"));
    }
    roots
}

#[cfg(unix)]
fn default_steamapps_roots() -> Vec<PathBuf> {
    let mut roots = Vec::new();
    if let Some(home) = dirs::home_dir() {
        roots.push(home.join(".local/share/Steam/steamapps"));
        roots.push(home.join(".steam/steam/steamapps"));
        roots.push(home.join(".steam/debian-installation/steamapps"));
    }
    roots
}

/// Parse a Steam `libraryfolders.vdf` file's contents for additional Steam
/// library root paths. The format is simple nested `"key"  "value"` pairs;
/// we only need the `"path"` entries, so a line scan is enough — no need for
/// a full VDF parser crate.
pub fn parse_library_paths(vdf_contents: &str) -> Vec<PathBuf> {
    vdf_contents
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            if !line.starts_with("\"path\"") {
                return None;
            }
            let mut parts = line.splitn(3, '"').skip(2);
            let rest = parts.next()?;
            let value = rest.trim().trim_matches('"');
            // vdf files sometimes contain double-backslash Windows paths;
            // normalize before turning them into a PathBuf.
            Some(PathBuf::from(value.replace("\\\\", "\\")))
        })
        .collect()
}

/// Build the full list of `steamapps/common/Valheim` candidates: the default
/// Steam roots, plus any extra library folders found in each root's
/// `libraryfolders.vdf`.
fn valheim_candidates() -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    for steamapps in default_steamapps_roots() {
        candidates.push(steamapps.join("common").join("Valheim"));

        let vdf_path = steamapps.join("libraryfolders.vdf");
        if let Ok(contents) = std::fs::read_to_string(&vdf_path) {
            for lib_root in parse_library_paths(&contents) {
                candidates.push(lib_root.join("steamapps").join("common").join("Valheim"));
            }
        }
    }
    candidates
}

/// Pick the first candidate that actually looks like a Valheim install.
pub fn find_first_valid(candidates: Vec<PathBuf>, exists: impl Fn(&Path) -> bool + Copy) -> Option<PathBuf> {
    candidates.into_iter().find(|c| is_valid_valheim_root(c, exists))
}

/// Auto-detect the Valheim install directory on this machine, or `None` if
/// nothing was found (caller should fall back to prompting the user).
pub fn find_valheim_install() -> Option<PathBuf> {
    find_first_valid(valheim_candidates(), |p| p.exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn parses_path_entries_from_vdf() {
        let vdf = r#"
            "libraryfolders"
            {
                "0"
                {
                    "path"		"C:\\Program Files (x86)\\Steam"
                    "label"		""
                }
                "1"
                {
                    "path"		"D:\\SteamLibrary"
                }
            }
        "#;
        let paths = parse_library_paths(vdf);
        assert_eq!(
            paths,
            vec![
                PathBuf::from("C:\\Program Files (x86)\\Steam"),
                PathBuf::from("D:\\SteamLibrary"),
            ]
        );
    }

    #[test]
    fn find_first_valid_picks_matching_candidate() {
        let real: HashSet<PathBuf> = [
            PathBuf::from("/lib2/steamapps/common/Valheim/valheim.x86_64"),
        ]
        .into_iter()
        .collect();
        let exists = |p: &Path| real.contains(&p.to_path_buf());

        let candidates = vec![
            PathBuf::from("/lib1/steamapps/common/Valheim"),
            PathBuf::from("/lib2/steamapps/common/Valheim"),
        ];
        let found = find_first_valid(candidates, exists);
        assert_eq!(found, Some(PathBuf::from("/lib2/steamapps/common/Valheim")));
    }

    #[test]
    fn find_first_valid_returns_none_when_nothing_matches() {
        let exists = |_: &Path| false;
        let candidates = vec![PathBuf::from("/nowhere/Valheim")];
        assert_eq!(find_first_valid(candidates, exists), None);
    }
}
