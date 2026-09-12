use crate::FOOTER_MAGIC;
use anyhow::{Context, Result, bail};
use std::fs::File;
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

/// Read the zstd-compressed tar payload appended after this executable's own
/// code by `mod-packager`. Footer layout (from EOF backwards):
/// `[payload bytes][8-byte LE payload length][9-byte magic "VMODPACK1"]`.
pub fn read_own_payload() -> Result<Vec<u8>> {
    let exe_path = std::env::current_exe().context("locating own executable path")?;
    let mut file = File::open(&exe_path).context("opening own executable")?;
    let file_len = file.metadata()?.len();

    let footer_len = FOOTER_MAGIC.len() as u64 + 8;
    if file_len < footer_len {
        bail!("executable is too small to contain an embedded payload");
    }

    file.seek(SeekFrom::End(-(footer_len as i64)))?;
    let mut footer = vec![0u8; footer_len as usize];
    file.read_exact(&mut footer)?;

    let (len_bytes, magic) = footer.split_at(8);
    if magic != FOOTER_MAGIC {
        bail!(
            "no embedded mod payload found (this binary wasn't packaged with mod-packager, \
             or the payload footer is corrupt)"
        );
    }
    let payload_len = u64::from_le_bytes(len_bytes.try_into().unwrap());

    let payload_start = file_len
        .checked_sub(footer_len)
        .and_then(|n| n.checked_sub(payload_len))
        .context("embedded payload length is inconsistent with executable size")?;

    file.seek(SeekFrom::Start(payload_start))?;
    let mut payload = vec![0u8; payload_len as usize];
    file.read_exact(&mut payload)?;
    Ok(payload)
}

/// Decompress (zstd) and unpack (tar) the payload into `dest_dir`.
pub fn unpack_payload(payload: &[u8], dest_dir: &Path) -> Result<()> {
    std::fs::create_dir_all(dest_dir).context("creating extraction temp dir")?;
    let decoder = zstd::Decoder::new(payload).context("initializing zstd decoder")?;
    let mut archive = tar::Archive::new(decoder);
    archive.unpack(dest_dir).context("unpacking mod tar archive")?;
    Ok(())
}
