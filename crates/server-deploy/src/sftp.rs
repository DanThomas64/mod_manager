use anyhow::{Context, Result};
use ssh2::{Session, Sftp};
use std::io::Write;
use std::net::TcpStream;
use std::path::Path;

pub struct SftpUploader {
    sftp: Sftp,
}

impl SftpUploader {
    pub fn connect(host: &str, port: u16, username: &str, password: &str) -> Result<Self> {
        let tcp = TcpStream::connect((host, port)).with_context(|| format!("connecting to {host}:{port}"))?;
        let mut session = Session::new().context("creating SSH session")?;
        session.set_tcp_stream(tcp);
        session.handshake().context("SSH handshake failed")?;
        session
            .userauth_password(username, password)
            .context("SFTP authentication failed (check username/password/totp_secret)")?;
        if !session.authenticated() {
            anyhow::bail!("SFTP authentication failed (check username/password/totp_secret)");
        }
        let sftp = session.sftp().context("opening SFTP channel")?;
        Ok(Self { sftp })
    }

    /// Recursively mirror `local_dir` into `remote_dir` over SFTP, creating
    /// subdirectories as needed and overwriting existing files. Returns the
    /// number of files uploaded.
    pub fn mirror_dir(&self, local_dir: &Path, remote_dir: &str) -> Result<usize> {
        self.ensure_remote_dir(remote_dir)?;
        let mut count = 0;
        for entry in std::fs::read_dir(local_dir).with_context(|| format!("reading {}", local_dir.display()))? {
            let entry = entry?;
            let name = entry.file_name();
            let name = name.to_string_lossy();
            let remote_path = format!("{}/{name}", remote_dir.trim_end_matches('/'));
            if entry.file_type()?.is_dir() {
                count += self.mirror_dir(&entry.path(), &remote_path)?;
            } else {
                self.upload_file(&entry.path(), &remote_path)?;
                count += 1;
            }
        }
        Ok(count)
    }

    fn ensure_remote_dir(&self, remote_dir: &str) -> Result<()> {
        let mut built = String::new();
        for part in remote_dir.trim_start_matches('/').split('/') {
            if part.is_empty() {
                continue;
            }
            built.push('/');
            built.push_str(part);
            if self.sftp.stat(Path::new(&built)).is_err() {
                self.sftp
                    .mkdir(Path::new(&built), 0o755)
                    .with_context(|| format!("creating remote directory {built}"))?;
            }
        }
        Ok(())
    }

    fn upload_file(&self, local_path: &Path, remote_path: &str) -> Result<()> {
        let data = std::fs::read(local_path).with_context(|| format!("reading {}", local_path.display()))?;
        let mut remote_file = self
            .sftp
            .create(Path::new(remote_path))
            .with_context(|| format!("creating remote file {remote_path}"))?;
        remote_file
            .write_all(&data)
            .with_context(|| format!("writing remote file {remote_path}"))?;
        Ok(())
    }
}
