use anyhow::{Context, Result};
use std::fs::File;
use std::io::Write;
use std::path::Path;

const FOOTER_MAGIC: &[u8] = b"VMODPACK1";

/// Append the compressed payload and the release's changelog text after a
/// prebuilt `installer-shell` binary, with a trailing footer
/// (`[payload][changelog][8-byte LE payload len][8-byte LE changelog len]
/// ["VMODPACK1"]`) that `installer-runtime::extract::read_own_payload` reads
/// back at runtime. This lets the shell binary be built once per target and
/// reused across releases — only the payload and changelog change between
/// packaging runs.
pub fn embed_payload(shell_binary: &Path, payload: &[u8], changelog: &str, output_path: &Path) -> Result<()> {
    let shell_bytes = std::fs::read(shell_binary)
        .with_context(|| format!("reading prebuilt installer shell at {}", shell_binary.display()))?;
    let changelog_bytes = changelog.as_bytes();

    let mut out = File::create(output_path)
        .with_context(|| format!("creating installer output {}", output_path.display()))?;
    out.write_all(&shell_bytes)?;
    out.write_all(payload)?;
    out.write_all(changelog_bytes)?;
    out.write_all(&(payload.len() as u64).to_le_bytes())?;
    out.write_all(&(changelog_bytes.len() as u64).to_le_bytes())?;
    out.write_all(FOOTER_MAGIC)?;
    drop(out);

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(output_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(output_path, perms)?;
    }

    Ok(())
}
