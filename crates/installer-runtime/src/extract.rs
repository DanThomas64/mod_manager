use crate::FOOTER_MAGIC;
use anyhow::{Context, Result, bail};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// The embedded mod payload plus the changelog entry for the release it
/// belongs to, both read back from the executable's own trailing footer.
pub struct OwnPayload {
    pub payload: Vec<u8>,
    pub changelog: String,
}

/// Read the zstd-compressed tar payload and changelog text appended after
/// this executable's own code by `mod-packager`. Footer layout (from EOF
/// backwards): `[payload bytes][changelog bytes][8-byte LE payload length]
/// [8-byte LE changelog length][9-byte magic "VMODPACK1"]`.
pub fn read_own_payload() -> Result<OwnPayload> {
    let exe_path = std::env::current_exe().context("locating own executable path")?;
    let mut file = File::open(&exe_path).context("opening own executable")?;
    let file_len = file.metadata()?.len();

    let footer_len = FOOTER_MAGIC.len() as u64 + 16;
    if file_len < footer_len {
        bail!("executable is too small to contain an embedded payload");
    }

    file.seek(SeekFrom::End(-(footer_len as i64)))?;
    let mut footer = vec![0u8; footer_len as usize];
    file.read_exact(&mut footer)?;

    let (lens, magic) = footer.split_at(16);
    if magic != FOOTER_MAGIC {
        bail!(
            "no embedded mod payload found (this binary wasn't packaged with mod-packager, \
             or the payload footer is corrupt)"
        );
    }
    let (payload_len_bytes, changelog_len_bytes) = lens.split_at(8);
    let payload_len = u64::from_le_bytes(payload_len_bytes.try_into().unwrap());
    let changelog_len = u64::from_le_bytes(changelog_len_bytes.try_into().unwrap());

    let changelog_end = file_len
        .checked_sub(footer_len)
        .context("embedded payload length is inconsistent with executable size")?;
    let changelog_start = changelog_end
        .checked_sub(changelog_len)
        .context("embedded changelog length is inconsistent with executable size")?;
    let payload_start = changelog_start
        .checked_sub(payload_len)
        .context("embedded payload length is inconsistent with executable size")?;

    file.seek(SeekFrom::Start(payload_start))?;
    let mut payload = vec![0u8; payload_len as usize];
    file.read_exact(&mut payload)?;

    file.seek(SeekFrom::Start(changelog_start))?;
    let mut changelog_bytes = vec![0u8; changelog_len as usize];
    file.read_exact(&mut changelog_bytes)?;
    let changelog = String::from_utf8_lossy(&changelog_bytes).into_owned();

    Ok(OwnPayload { payload, changelog })
}

/// Decompress (zstd) and unpack (tar) the payload into `dest_dir`.
pub fn unpack_payload(payload: &[u8], dest_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dest_dir).context("creating extraction temp dir")?;
    let decoder = zstd::Decoder::new(payload).context("initializing zstd decoder")?;
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(dest_dir).context("unpacking mod tar archive")?;
    Ok(())
}
