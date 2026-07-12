//! SSH — remote file copy over SSH/SFTP via libssh2.
//!
//! Uses the `ssh2` crate (libssh2 bindings) to provide
//! password-based and key-based SFTP file transfer.

use anyhow::{Context, Result};
use std::path::Path;

/// SSH connection parameters.
#[derive(Debug, Clone)]
pub struct SshConfig {
    pub host: String,
    pub port: u16,
    pub username: String,
    /// Optional password (or use key-based auth).
    pub password: Option<String>,
    /// Optional path to private key.
    pub key_path: Option<String>,
}

impl SshConfig {
    pub fn new(host: &str, username: &str) -> Self {
        Self {
            host: host.into(),
            port: 22,
            username: username.into(),
            password: None,
            key_path: None,
        }
    }
}

/// An SSH session that can perform SFTP transfers.
pub struct SshSession {
    session: ssh2::Session,
}

impl SshSession {
    /// Connect and authenticate to an SSH server.
    pub fn connect(config: &SshConfig) -> Result<Self> {
        let tcp = std::net::TcpStream::connect(format!("{}:{}", config.host, config.port))
            .with_context(|| format!("Failed to connect to {}:{}", config.host, config.port))?;
        tcp.set_read_timeout(Some(std::time::Duration::from_secs(30)))?;
        tcp.set_write_timeout(Some(std::time::Duration::from_secs(30)))?;

        let mut session = ssh2::Session::new()
            .context("Failed to create SSH session")?;
        session.set_tcp_stream(tcp);
        session.handshake()
            .context("SSH handshake failed")?;

        // Authenticate
        if let Some(key) = &config.key_path {
            session.userauth_pubkey_file(
                &config.username,
                None,
                Path::new(key),
                None,
            ).context("SSH public key auth failed")?;
        } else if let Some(pw) = &config.password {
            session.userauth_password(&config.username, pw)
                .context("SSH password auth failed")?;
        } else {
            session.userauth_agent(&config.username)
                .context("SSH agent auth failed")?;
        }

        Ok(Self { session })
    }

    /// Copy a local file to a remote path via SFTP.
    pub fn upload_file(&self, local: &Path, remote: &Path) -> Result<()> {
        let sftp = self.session.sftp().context("Failed to open SFTP session")?;
        let mut remote_file = sftp.create(remote).context("Failed to create remote file")?;
        let mut local_file = std::fs::File::open(local)
            .with_context(|| format!("Failed to open local file: {}", local.display()))?;
        std::io::copy(&mut local_file, &mut remote_file)
            .context("Failed to upload file")?;
        Ok(())
    }

    /// Copy a remote file to a local path via SFTP.
    pub fn download_file(&self, remote: &Path, local: &Path) -> Result<()> {
        let sftp = self.session.sftp().context("Failed to open SFTP session")?;
        let mut remote_file = sftp.open(remote).context("Failed to open remote file")?;
        if let Some(parent) = local.parent() {
            std::fs::create_dir_all(parent)
                .context("Failed to create local directory")?;
        }
        let mut local_file = std::fs::File::create(local)
            .with_context(|| format!("Failed to create local file: {}", local.display()))?;
        std::io::copy(&mut remote_file, &mut local_file)
            .context("Failed to download file")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ssh_config_new() {
        let config = SshConfig::new("192.168.1.100", "user");
        assert_eq!(config.host, "192.168.1.100");
        assert_eq!(config.username, "user");
        assert_eq!(config.port, 22);
    }

    #[test]
    fn test_ssh_config_with_password() {
        let mut config = SshConfig::new("example.com", "admin");
        config.password = Some("secret".into());
        assert_eq!(config.password.as_deref(), Some("secret"));
    }

    #[test]
    fn test_ssh_config_with_key() {
        let mut config = SshConfig::new("example.com", "admin");
        config.key_path = Some("~/.ssh/id_rsa".into());
        assert!(config.key_path.as_deref().unwrap().contains("id_rsa"));
    }

    #[test]
    fn test_ssh_config_default_port() {
        let config = SshConfig::new("10.0.0.1", "test");
        assert_eq!(config.port, 22);
    }
}