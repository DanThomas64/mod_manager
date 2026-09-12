mod amp_api;
mod sftp;

use anyhow::{Context, Result};
use clap::Parser;
use serde::Deserialize;
use std::path::PathBuf;

/// Pushes a release's server-plugins/ folder to an AMP-managed dedicated
/// server over SFTP, then triggers a restart via AMP's JSON API.
#[derive(Parser)]
struct Args {
    #[arg(long, default_value = "amp.toml")]
    config: PathBuf,
    /// Release directory to deploy (e.g. releases/v0.2.0). Defaults to
    /// whatever releases/latest.txt points at.
    #[arg(long)]
    release_dir: Option<PathBuf>,
    #[arg(long, default_value = "releases")]
    releases_dir: PathBuf,
}

#[derive(Deserialize)]
struct AmpConfig {
    sftp: SftpConfig,
    api: ApiConfig,
}

#[derive(Deserialize)]
struct SftpConfig {
    host: String,
    #[serde(default = "default_sftp_port")]
    port: u16,
    username: String,
    password: String,
    #[serde(default)]
    totp_secret: String,
    remote_plugins_path: String,
}

fn default_sftp_port() -> u16 {
    2223
}

#[derive(Deserialize)]
struct ApiConfig {
    base_url: String,
    username: String,
    password: String,
    #[serde(default = "default_restart_method")]
    restart_method: String,
}

fn default_restart_method() -> String {
    "Core/Restart".to_string()
}

fn main() -> Result<()> {
    let args = Args::parse();

    let config_text = std::fs::read_to_string(&args.config).with_context(|| {
        format!(
            "reading {} (copy amp.toml.example to amp.toml and fill it in)",
            args.config.display()
        )
    })?;
    let config: AmpConfig =
        toml::from_str(&config_text).with_context(|| format!("parsing {}", args.config.display()))?;

    let release_dir = match args.release_dir {
        Some(dir) => dir,
        None => common::latest_release_dir(&args.releases_dir)
            .context("resolving latest release (pass --release-dir to override)")?,
    };
    let server_plugins_dir = release_dir.join("server-plugins");
    if !server_plugins_dir.is_dir() {
        anyhow::bail!(
            "{} not found — run mod-packager first",
            server_plugins_dir.display()
        );
    }

    println!(
        "Deploying {} to {}",
        server_plugins_dir.display(),
        config.sftp.host
    );

    let password = if config.sftp.totp_secret.trim().is_empty() {
        config.sftp.password.clone()
    } else {
        let code = totp_code(&config.sftp.totp_secret)?;
        format!("{}{code}", config.sftp.password)
    };

    let uploader = sftp::SftpUploader::connect(&config.sftp.host, config.sftp.port, &config.sftp.username, &password)
        .context("connecting to AMP SFTP")?;
    let uploaded = uploader
        .mirror_dir(&server_plugins_dir, &config.sftp.remote_plugins_path)
        .context("uploading server-plugins over SFTP")?;
    println!("Uploaded {uploaded} file(s) to {}", config.sftp.remote_plugins_path);

    match amp_api::AmpClient::login(&config.api.base_url, &config.api.username, &config.api.password) {
        Ok(client) => match client.call(&config.api.restart_method, serde_json::json!({})) {
            Ok(_) => println!("Restart triggered via AMP API ({}).", config.api.restart_method),
            Err(e) => eprintln!(
                "Warning: upload succeeded but the restart call failed: {e:#}\n\
                 Restart the server manually from the AMP panel, and check `restart_method` in {} \
                 against <base_url>/API on your instance.",
                args.config.display()
            ),
        },
        Err(e) => eprintln!(
            "Warning: upload succeeded but AMP API login failed: {e:#}\n\
             Restart the server manually from the AMP panel."
        ),
    }

    Ok(())
}

fn totp_code(secret: &str) -> Result<String> {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .context("reading system time")?
        .as_secs();
    totp_code_at(secret, now)
}

fn totp_code_at(secret: &str, unix_time: u64) -> Result<String> {
    use totp_rs::{Algorithm, Secret, TOTP};
    let bytes = Secret::Encoded(secret.to_string())
        .to_bytes()
        .map_err(|e| anyhow::anyhow!("invalid totp_secret: {e:?}"))?;
    let totp = TOTP::new(Algorithm::SHA1, 6, 1, 30, bytes).context("building TOTP generator")?;
    Ok(totp.generate(unix_time))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_known_rfc6238_style_vectors() {
        // Verified against a standalone HMAC-SHA1 TOTP implementation (RFC
        // 6238), 6 digits, 30s step. Secret must decode to >=128 bits (16
        // bytes) — totp-rs rejects shorter ones.
        let secret = "JBSWY3DPEHPK3PXPJBSWY3DPEHPK3PXP";
        assert_eq!(totp_code_at(secret, 1_000_000_000).unwrap(), "832818");
        assert_eq!(totp_code_at(secret, 1_000_000_030).unwrap(), "027653");
    }

    #[test]
    fn rejects_invalid_secret() {
        assert!(totp_code_at("not valid base32!!", 0).is_err());
    }
}
