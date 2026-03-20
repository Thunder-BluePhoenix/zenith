/// Rust toolchain — uses rustup-init to bootstrap an isolated toolchain.
/// Cache: ~/.zenith/toolchains/rust/<version>/bin/rustc
///
/// rustup-init is downloaded once by Zenith, then invoked with:
///   RUSTUP_HOME=~/.zenith/toolchains/rust/<version>/rustup
///   CARGO_HOME=~/.zenith/toolchains/rust/<version>
///   rustup-init -y --no-modify-path --default-toolchain <version>
/// This installs cargo + rustc into the isolated CARGO_HOME without touching
/// the system's ~/.cargo or ~/.rustup.

use anyhow::{Context, Result};
use std::path::PathBuf;
use tracing::info;
use super::toolchain_dir;

pub struct RustToolchain { version: String }

impl RustToolchain {
    pub fn new(version: &str) -> Self {
        Self { version: version.to_string() }
    }
}

impl RustToolchain {
    pub async fn ensure_installed(&self) -> Result<PathBuf> {
        let install_dir = toolchain_dir("rust", &self.version);
        let bin_dir = install_dir.join("bin");
        let cargo_bin = bin_dir.join(if cfg!(target_os = "windows") { "cargo.exe" } else { "cargo" });

        if cargo_bin.exists() {
            return Ok(bin_dir);
        }

        info!("Zenith is installing Rust {} ...", self.version);

        std::fs::create_dir_all(&install_dir)?;
        std::fs::create_dir_all(&bin_dir)?;

        // Download rustup-init into ~/.zenith/bin/
        let rustup_init = download_rustup_init().await?;

        // Run rustup-init in fully isolated mode
        let rustup_home = install_dir.join("rustup");
        let cargo_home  = install_dir.clone();

        let status = std::process::Command::new(&rustup_init)
            .args([
                "-y",
                "--no-modify-path",
                "--default-toolchain", &self.version,
            ])
            .env("RUSTUP_HOME", &rustup_home)
            .env("CARGO_HOME",  &cargo_home)
            .status()
            .context("Failed to run rustup-init")?;

        if !status.success() {
            return Err(anyhow::anyhow!(
                "rustup-init exited with {} while installing Rust {}",
                status, self.version
            ));
        }

        if !cargo_bin.exists() {
            return Err(anyhow::anyhow!(
                "Rust {} installation failed — cargo not found at {:?}", self.version, cargo_bin
            ));
        }

        info!("Rust {} ready at {:?}", self.version, bin_dir);
        Ok(bin_dir)
    }
}

async fn download_rustup_init() -> Result<PathBuf> {
    let url = rustup_init_url();
    let dest = crate::tools::bin_dir().join(
        if cfg!(target_os = "windows") { "rustup-init.exe" } else { "rustup-init" }
    );

    if dest.exists() {
        return Ok(dest);
    }

    info!("Zenith is downloading rustup-init...");
    let bytes = crate::tools::fetch_url(url).await?;
    std::fs::write(&dest, &bytes).context("Failed to write rustup-init")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&dest)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dest, perms)?;
    }

    Ok(dest)
}

fn rustup_init_url() -> &'static str {
    if cfg!(all(target_os = "linux", target_arch = "x86_64")) {
        "https://static.rust-lang.org/rustup/dist/x86_64-unknown-linux-musl/rustup-init"
    } else if cfg!(all(target_os = "linux", target_arch = "aarch64")) {
        "https://static.rust-lang.org/rustup/dist/aarch64-unknown-linux-musl/rustup-init"
    } else if cfg!(all(target_os = "macos", target_arch = "x86_64")) {
        "https://static.rust-lang.org/rustup/dist/x86_64-apple-darwin/rustup-init"
    } else if cfg!(all(target_os = "macos", target_arch = "aarch64")) {
        "https://static.rust-lang.org/rustup/dist/aarch64-apple-darwin/rustup-init"
    } else {
        "https://static.rust-lang.org/rustup/dist/x86_64-pc-windows-msvc/rustup-init.exe"
    }
}
