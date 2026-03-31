/// Zenith Tool Manager — "You install Zenith. Zenith installs everything else."
///
/// This module handles downloading, caching, and managing all external tool
/// binaries that Zenith needs: Firecracker, QEMU user-mode, wasmtime, etc.
/// Users never need to `apt install` or `brew install` anything manually.
///
/// All binaries are stored in `~/.zenith/bin/` and are downloaded only once.

use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use tracing::info;

// ─── Versioned tool coordinates ──────────────────────────────────────────────

/// Firecracker VMM — AWS open source MicroVM manager (Linux x86_64 only)
pub const FIRECRACKER_VERSION: &str = "1.7.0";

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub const FIRECRACKER_URL: &str =
    "https://github.com/firecracker-microvm/firecracker/releases/download/v1.7.0/firecracker-v1.7.0-x86_64.tgz";

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub const FIRECRACKER_URL: &str =
    "https://github.com/firecracker-microvm/firecracker/releases/download/v1.7.0/firecracker-v1.7.0-aarch64.tgz";

/// QEMU user-mode static binaries — cross-arch emulation (Linux only)
pub const QEMU_VERSION: &str = "8.2.0";

#[cfg(target_os = "linux")]
pub const QEMU_AARCH64_URL: &str =
    "https://github.com/multiarch/qemu-user-static/releases/download/v7.2.0-1/qemu-aarch64-static";

#[cfg(target_os = "linux")]
pub const QEMU_ARM_URL: &str =
    "https://github.com/multiarch/qemu-user-static/releases/download/v7.2.0-1/qemu-arm-static";

#[cfg(target_os = "linux")]
pub const QEMU_RISCV64_URL: &str =
    "https://github.com/multiarch/qemu-user-static/releases/download/v7.2.0-1/qemu-riscv64-static";

/// wasmtime CLI — WebAssembly + WASI runtime (cross-platform)
pub const WASMTIME_VERSION: &str = "18.0.3";

#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub const WASMTIME_URL: &str =
    "https://github.com/bytecodealliance/wasmtime/releases/download/v18.0.3/wasmtime-v18.0.3-x86_64-linux.tar.xz";

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub const WASMTIME_URL: &str =
    "https://github.com/bytecodealliance/wasmtime/releases/download/v18.0.3/wasmtime-v18.0.3-aarch64-linux.tar.xz";

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
pub const WASMTIME_URL: &str =
    "https://github.com/bytecodealliance/wasmtime/releases/download/v18.0.3/wasmtime-v18.0.3-x86_64-macos.tar.xz";

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
pub const WASMTIME_URL: &str =
    "https://github.com/bytecodealliance/wasmtime/releases/download/v18.0.3/wasmtime-v18.0.3-aarch64-macos.tar.xz";

#[cfg(target_os = "windows")]
pub const WASMTIME_URL: &str =
    "https://github.com/bytecodealliance/wasmtime/releases/download/v18.0.3/wasmtime-v18.0.3-x86_64-windows.zip";

/// Wine portable build — Windows exe on Linux (Phase 5)
pub const WINE_VERSION: &str = "9.0";

#[cfg(target_os = "linux")]
pub const WINE_URL: &str =
    "https://github.com/Kron4ek/Wine-Builds/releases/download/9.0/wine-9.0-amd64.tar.xz";

// ─── Firecracker kernel + rootfs images ───────────────────────────────────────

/// Pre-built Linux kernel (vmlinux ELF) tuned for Firecracker / KVM.
/// Source: Firecracker's own CI quickstart assets (maintained by AWS Firecracker team).
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub const FC_KERNEL_URL: &str =
    "https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/v1.7/x86_64/vmlinux-5.10.217";

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub const FC_KERNEL_URL: &str =
    "https://s3.amazonaws.com/spec.ccfc.min/firecracker-ci/v1.7/aarch64/vmlinux-4.14.210";

/// Pre-built Alpine Linux ext4 rootfs disk image for Firecracker.
/// Zenith downloads and caches this under ~/.zenith/rootfs-fc/alpine.ext4
/// This is the Firecracker quickstart Ubuntu image (Alpine-compatible workflows).
#[cfg(all(target_os = "linux", target_arch = "x86_64"))]
pub const FC_ROOTFS_URL: &str =
    "https://s3.amazonaws.com/spec.ccfc.min/img/quickstart_guide/x86_64/rootfs/bionic.rootfs.ext4";

#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
pub const FC_ROOTFS_URL: &str =
    "https://s3.amazonaws.com/spec.ccfc.min/img/quickstart_guide/aarch64/rootfs/bionic.rootfs.ext4";

// ─── bin directory ────────────────────────────────────────────────────────────

pub fn bin_dir() -> PathBuf {
    crate::sandbox::zenith_home().join("bin")
}

pub fn tool_path(name: &str) -> PathBuf {
    #[cfg(target_os = "windows")]
    return bin_dir().join(format!("{}.exe", name));
    #[cfg(not(target_os = "windows"))]
    bin_dir().join(name)
}

// ─── Public API ───────────────────────────────────────────────────────────────

/// Ensure a tool binary exists at `~/.zenith/bin/<name>`.
/// Downloads and extracts if not already present.
/// Returns the full path to the ready-to-use binary.
pub async fn ensure_tool(name: &str, url: &str) -> Result<PathBuf> {
    let path = tool_path(name);

    if path.exists() {
        return Ok(path);
    }

    info!("Zenith is downloading {} (first-time setup, cached afterward)...", name);
    std::fs::create_dir_all(bin_dir()).context("Failed to create ~/.zenith/bin")?;

    let url_lower = url.to_lowercase();

    if url_lower.ends_with(".tar.xz") {
        download_tar_xz(name, url, &path).await?;
    } else if url_lower.ends_with(".tgz") || url_lower.ends_with(".tar.gz") {
        download_tar_gz_binary(name, url, &path).await?;
    } else if url_lower.ends_with(".zip") {
        download_zip_binary(name, url, &path).await?;
    } else {
        // Plain binary — direct download
        download_plain_binary(url, &path).await?;
    }

    // Mark executable on Unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&path, perms)?;
    }

    info!("{} is ready at {:?}", name, path);
    Ok(path)
}

/// Ensure Firecracker VMM binary is available.
#[cfg(target_os = "linux")]
pub async fn ensure_firecracker() -> Result<PathBuf> {
    ensure_tool("firecracker", FIRECRACKER_URL).await
}

/// Ensure a QEMU user-mode static binary is available for the target arch.
#[cfg(target_os = "linux")]
pub async fn ensure_qemu_for_arch(target_arch: &str) -> Result<PathBuf> {
    let (name, url) = match target_arch {
        "aarch64" | "arm64" => ("qemu-aarch64-static", QEMU_AARCH64_URL),
        "arm" | "armv7" => ("qemu-arm-static", QEMU_ARM_URL),
        "riscv64" => ("qemu-riscv64-static", QEMU_RISCV64_URL),
        other => return Err(anyhow::anyhow!(
            "No QEMU user-mode binary available for arch '{}'. Supported: aarch64, arm, riscv64", other
        )),
    };
    ensure_tool(name, url).await
}

/// Ensure wasmtime CLI binary is available (cross-platform).
pub async fn ensure_wasmtime() -> Result<PathBuf> {
    ensure_tool("wasmtime", WASMTIME_URL).await
}

/// Ensure the Firecracker-compatible Linux kernel (vmlinux) is cached.
/// Stored at ~/.zenith/kernel/vmlinux — downloaded once, reused forever.
#[cfg(target_os = "linux")]
pub async fn ensure_fc_kernel() -> Result<PathBuf> {
    let kernel_dir = crate::sandbox::zenith_home().join("kernel");
    std::fs::create_dir_all(&kernel_dir).context("Failed to create kernel cache dir")?;
    let kernel_path = kernel_dir.join("vmlinux");
    if kernel_path.exists() {
        return Ok(kernel_path);
    }
    info!("Zenith is downloading a Firecracker-compatible Linux kernel (first-time setup)...");
    let bytes = fetch_bytes(FC_KERNEL_URL).await?;
    std::fs::write(&kernel_path, &bytes).context("Failed to write vmlinux")?;
    // Mark executable so Firecracker can use it
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&kernel_path)?.permissions();
        perms.set_mode(0o644);
        std::fs::set_permissions(&kernel_path, perms)?;
    }
    info!("Firecracker kernel cached at {:?}", kernel_path);
    Ok(kernel_path)
}

/// Ensure an ext4 rootfs disk image is available for Firecracker.
/// Stored at ~/.zenith/rootfs-fc/<os>.ext4
/// On first use Zenith downloads a pre-built image — no mkfs.ext4 required.
#[cfg(target_os = "linux")]
pub async fn ensure_fc_rootfs(os: &str) -> Result<PathBuf> {
    let fc_rootfs_dir = crate::sandbox::zenith_home().join("rootfs-fc");
    std::fs::create_dir_all(&fc_rootfs_dir).context("Failed to create FC rootfs dir")?;
    let img_path = fc_rootfs_dir.join(format!("{}.ext4", os));
    if img_path.exists() {
        return Ok(img_path);
    }
    info!("Zenith is downloading Firecracker rootfs for '{}' (first-time setup)...", os);
    let bytes = fetch_bytes(FC_ROOTFS_URL).await?;
    std::fs::write(&img_path, &bytes).context("Failed to write ext4 rootfs image")?;
    info!("Firecracker rootfs cached at {:?}", img_path);
    Ok(img_path)
}

/// Ensure Wine portable build is available (Linux only — Windows exe runner).
#[cfg(target_os = "linux")]
pub async fn ensure_wine() -> Result<PathBuf> {
    // Wine extracts to a directory, not a single binary.
    let wine_dir = crate::sandbox::zenith_home().join("wine").join(WINE_VERSION);
    let wine_bin = wine_dir.join("bin").join("wine");
    if wine_bin.exists() {
        return Ok(wine_bin);
    }
    info!("Zenith is downloading Wine {} (first-time setup for Windows exe support)...", WINE_VERSION);
    std::fs::create_dir_all(&wine_dir).context("Failed to create wine dir")?;
    let parent = wine_dir.parent().unwrap();
    download_tar_xz_dir("wine", WINE_URL, parent).await?;
    if wine_bin.exists() {
        info!("Wine ready at {:?}", wine_bin);
        Ok(wine_bin)
    } else {
        Err(anyhow::anyhow!("Wine binary not found after extraction at {:?}", wine_bin))
    }
}

// ─── Download helpers ─────────────────────────────────────────────────────────

async fn download_plain_binary(url: &str, dest: &Path) -> Result<()> {
    let bytes = fetch_bytes(url).await?;
    std::fs::write(dest, &bytes).context("Failed to write binary")?;
    Ok(())
}

/// Download a .tar.xz archive and extract the first executable file found.
async fn download_tar_xz(name: &str, url: &str, dest: &Path) -> Result<()> {
    let bytes = fetch_bytes(url).await?;
    let tmp = bin_dir().join(format!("_tmp_{}.tar.xz", name));
    std::fs::write(&tmp, &bytes)?;

    // Extract — find the binary matching `name` anywhere inside the archive
    let file = std::fs::File::open(&tmp)?;
    let decomp = xz2::read::XzDecoder::new(file);
    let mut archive = tar::Archive::new(decomp);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        let file_name = path.file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");

        // Match the binary by name (ignore extensions like .exe on the archive side)
        let base = dest.file_name().and_then(|f| f.to_str()).unwrap_or(name);
        let base_no_ext = base.trim_end_matches(".exe");

        if file_name == base_no_ext || file_name == base || file_name == name {
            entry.unpack(dest).context("Failed to extract binary from tar.xz")?;
            let _ = std::fs::remove_file(&tmp);
            return Ok(());
        }
    }

    let _ = std::fs::remove_file(&tmp);
    Err(anyhow::anyhow!("Binary '{}' not found inside archive from {}", name, url))
}

/// Download a .tar.gz or .tgz archive and extract the first executable.
async fn download_tar_gz_binary(name: &str, url: &str, dest: &Path) -> Result<()> {
    let bytes = fetch_bytes(url).await?;
    let tmp = bin_dir().join(format!("_tmp_{}.tar.gz", name));
    std::fs::write(&tmp, &bytes)?;

    let file = std::fs::File::open(&tmp)?;
    let decomp = flate2::read::GzDecoder::new(file);
    let mut archive = tar::Archive::new(decomp);

    for entry in archive.entries()? {
        let mut entry = entry?;
        let path = entry.path()?;
        let file_name = path.file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");

        let base = dest.file_name().and_then(|f| f.to_str()).unwrap_or(name);
        let base_no_ext = base.trim_end_matches(".exe");

        if file_name == base_no_ext || file_name == base || file_name == name {
            entry.unpack(dest).context("Failed to extract binary from tar.gz")?;
            let _ = std::fs::remove_file(&tmp);
            return Ok(());
        }
    }

    let _ = std::fs::remove_file(&tmp);
    Err(anyhow::anyhow!("Binary '{}' not found inside archive from {}", name, url))
}

/// Download a .zip archive and extract the binary (Windows / wasmtime on Windows).
async fn download_zip_binary(name: &str, url: &str, dest: &Path) -> Result<()> {
    let bytes = fetch_bytes(url).await?;
    let tmp = bin_dir().join(format!("_tmp_{}.zip", name));
    std::fs::write(&tmp, &bytes)?;

    let file = std::fs::File::open(&tmp)?;
    let mut zip = zip::ZipArchive::new(file).context("Failed to open zip archive")?;

    let base = dest.file_name().and_then(|f| f.to_str()).unwrap_or(name);

    for i in 0..zip.len() {
        let mut zf = zip.by_index(i)?;
        let file_name = zf.name().to_string();
        let stem = std::path::Path::new(&file_name)
            .file_name()
            .and_then(|f| f.to_str())
            .unwrap_or("");

        if stem == base || stem.trim_end_matches(".exe") == name {
            let mut out = std::fs::File::create(dest)?;
            std::io::copy(&mut zf, &mut out)?;
            let _ = std::fs::remove_file(&tmp);
            return Ok(());
        }
    }

    let _ = std::fs::remove_file(&tmp);
    Err(anyhow::anyhow!("Binary '{}' not found in zip from {}", name, url))
}

/// Download a .tar.xz and extract the entire archive into `dest_parent/`.
/// Used for Wine which ships as a multi-file directory tarball.
#[allow(dead_code)]
async fn download_tar_xz_dir(name: &str, url: &str, dest_parent: &Path) -> Result<()> {
    let bytes = fetch_bytes(url).await?;
    let tmp = dest_parent.join(format!("_tmp_{}.tar.xz", name));
    std::fs::write(&tmp, &bytes)?;

    let file = std::fs::File::open(&tmp)?;
    let decomp = xz2::read::XzDecoder::new(file);
    let mut archive = tar::Archive::new(decomp);
    archive.unpack(dest_parent).context("Failed to extract tar.xz directory")?;

    let _ = std::fs::remove_file(&tmp);
    Ok(())
}

/// Public alias so toolchain modules can reuse the same HTTP client.
pub async fn fetch_url(url: &str) -> Result<bytes::Bytes> {
    fetch_bytes(url).await
}

// ─── Phase 12: Zenith custom kernel + minimal rootfs ─────────────────────────

/// CDN base for Zenith-built artefacts (kernel, rootfs, zenith-init).
const ZENITH_CDN: &str = "https://cdn.zenith.run/v0.1.0";

/// Ensure the Zenith custom kernel is at `~/.zenith/kernel/vmlinux-zenith`.
///
/// This is a stripped-down Linux build optimised for sub-50ms Firecracker
/// VM boot times, built from `kernel/zenith.config` in the repo.
pub async fn ensure_zenith_kernel() -> Result<std::path::PathBuf> {
    let kernel_dir  = crate::sandbox::zenith_home().join("kernel");
    let kernel_path = kernel_dir.join("vmlinux-zenith");

    if kernel_path.exists() {
        tracing::debug!("Zenith kernel already present: {:?}", kernel_path);
        return Ok(kernel_path);
    }

    let url = format!("{}/kernel/vmlinux-zenith", ZENITH_CDN);
    tracing::info!("Downloading Zenith custom kernel...");

    std::fs::create_dir_all(&kernel_dir)
        .context("Cannot create kernel directory")?;

    let bytes = fetch_url(&url).await?;
    std::fs::write(&kernel_path, &bytes)
        .context("Failed to write kernel to disk")?;

    tracing::info!("Zenith kernel ready: {:?}", kernel_path);
    Ok(kernel_path)
}

/// Ensure the Zenith minimal rootfs is at `~/.zenith/rootfs/zenith-minimal.tar.gz`.
///
/// A < 5MB BusyBox + musl rootfs with only what CI actually needs:
/// sh, curl, git, make, tar. Smaller and faster than Alpine.
pub async fn ensure_zenith_rootfs() -> Result<std::path::PathBuf> {
    let rootfs_dir  = crate::sandbox::zenith_home().join("rootfs");
    let rootfs_path = rootfs_dir.join("zenith-minimal.tar.gz");

    if rootfs_path.exists() {
        tracing::debug!("Zenith rootfs already present: {:?}", rootfs_path);
        return Ok(rootfs_path);
    }

    let url = format!("{}/rootfs/zenith-minimal.tar.gz", ZENITH_CDN);
    tracing::info!("Downloading Zenith minimal rootfs...");

    std::fs::create_dir_all(&rootfs_dir)
        .context("Cannot create rootfs directory")?;

    let bytes = fetch_url(&url).await?;
    std::fs::write(&rootfs_path, &bytes)
        .context("Failed to write rootfs to disk")?;

    tracing::info!("Zenith rootfs ready: {:?}", rootfs_path);
    Ok(rootfs_path)
}

async fn fetch_bytes(url: &str) -> Result<bytes::Bytes> {
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .header("User-Agent", "zenith-runtime/1.0")
        .send()
        .await
        .context("Network request failed")?;

    if !resp.status().is_success() {
        return Err(anyhow::anyhow!(
            "Download failed (HTTP {}): {}", resp.status(), url
        ));
    }

    resp.bytes().await.context("Failed to read response body")
}
