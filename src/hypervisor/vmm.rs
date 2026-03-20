/// ZenithVmm — the core KVM Virtual Machine Monitor.
///
/// Owns the system-level /dev/kvm file descriptor and is the factory for
/// all ZenithVm instances. One VMM per process.
///
/// KVM ioctl reference: https://www.kernel.org/doc/html/latest/virt/kvm/api.html

use anyhow::{bail, Context, Result};
use std::fs::OpenOptions;
use std::os::unix::io::{AsRawFd, FromRawFd, OwnedFd};
use nix::ioctl_none;
use nix::ioctl_write_int;
use nix::ioctl_write_ptr;
use nix::ioctl_read;

use super::vm::{ZenithVm, KvmUserspaceMemoryRegion};

// ─── KVM ioctl definitions ────────────────────────────────────────────────────

// KVMIO magic number — all KVM ioctls use type 0xAE
const KVMIO: u8 = 0xAE;

// System-level ioctls (on /dev/kvm fd)
ioctl_none!(kvm_get_api_version,     KVMIO, 0x00);
ioctl_none!(kvm_create_vm_ioctl,     KVMIO, 0x01);
ioctl_none!(kvm_get_vcpu_mmap_size,  KVMIO, 0x04);

// VM-level ioctls (on VM fd returned by KVM_CREATE_VM)
ioctl_write_ptr!(kvm_set_user_memory_region, KVMIO, 0x46, KvmUserspaceMemoryRegion);
ioctl_write_int!(kvm_create_vcpu,    KVMIO, 0x41);

// ─── ZenithVmm ───────────────────────────────────────────────────────────────

/// The top-level KVM VMM.  Create once per process; use it to create VMs.
pub struct ZenithVmm {
    /// File descriptor for `/dev/kvm`
    kvm_fd: std::fs::File,
    /// Size of the `kvm_run` struct (needed to mmap per-VCPU shared memory)
    pub vcpu_mmap_size: usize,
}

impl ZenithVmm {
    /// Open `/dev/kvm` and verify KVM API version 12 (the stable API).
    pub fn new() -> Result<Self> {
        let kvm_fd = OpenOptions::new()
            .read(true)
            .write(true)
            .open("/dev/kvm")
            .context("Cannot open /dev/kvm — is KVM enabled in the kernel?")?;

        // KVM_GET_API_VERSION must return 12
        let version = unsafe { kvm_get_api_version(kvm_fd.as_raw_fd()) }
            .context("KVM_GET_API_VERSION ioctl failed")?;

        if version != 12 {
            bail!("Unexpected KVM API version {} (expected 12)", version);
        }

        // KVM_GET_VCPU_MMAP_SIZE — fixed per host kernel; used when creating VCPUs
        let vcpu_mmap_size = unsafe { kvm_get_vcpu_mmap_size(kvm_fd.as_raw_fd()) }
            .context("KVM_GET_VCPU_MMAP_SIZE ioctl failed")? as usize;

        tracing::debug!(version, vcpu_mmap_size, "KVM VMM initialised");

        Ok(Self { kvm_fd, vcpu_mmap_size })
    }

    /// Create a new isolated VM.  Returns a `ZenithVm` ready for memory
    /// allocation and VCPU setup.
    ///
    /// KVM_CREATE_VM returns a new file descriptor representing the VM.
    pub fn create_vm(&self, guest_mem_bytes: u64) -> Result<ZenithVm> {
        let vm_fd_raw = unsafe { kvm_create_vm_ioctl(self.kvm_fd.as_raw_fd()) }
            .context("KVM_CREATE_VM ioctl failed — out of KVM file descriptors?")?;

        // Safety: KVM gives us a brand-new fd we now own.
        let vm_fd = unsafe { OwnedFd::from_raw_fd(vm_fd_raw) };

        // Allocate guest RAM with mmap(ANON | SHARED) so the kernel can manage it
        let guest_mem = Self::alloc_guest_mem(guest_mem_bytes)?;

        // KVM_SET_USER_MEMORY_REGION — tell KVM where guest RAM lives in host VA
        let region = KvmUserspaceMemoryRegion {
            slot:             0,
            flags:            0,
            guest_phys_addr:  0,
            memory_size:      guest_mem_bytes,
            userspace_addr:   guest_mem as u64,
        };
        unsafe {
            kvm_set_user_memory_region(vm_fd.as_raw_fd(), &region)
        }.context("KVM_SET_USER_MEMORY_REGION failed")?;

        // Create one VCPU
        let vcpu_fd_raw = unsafe { kvm_create_vcpu(vm_fd.as_raw_fd(), 0) }
            .context("KVM_CREATE_VCPU failed")?;
        let vcpu_fd = unsafe { OwnedFd::from_raw_fd(vcpu_fd_raw) };

        // mmap kvm_run struct for the VCPU (shared memory between user/kernel space)
        let kvm_run_ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                self.vcpu_mmap_size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_SHARED,
                vcpu_fd.as_raw_fd(),
                0,
            )
        };
        if kvm_run_ptr == libc::MAP_FAILED {
            bail!("mmap(kvm_run) failed: {}", std::io::Error::last_os_error());
        }

        tracing::debug!(
            vm_fd = vm_fd.as_raw_fd(),
            vcpu_fd = vcpu_fd.as_raw_fd(),
            guest_mem_bytes,
            "VM created"
        );

        Ok(ZenithVm::new(vm_fd, vcpu_fd, guest_mem, guest_mem_bytes, kvm_run_ptr, self.vcpu_mmap_size))
    }

    /// Allocate anonymous guest memory using mmap.
    fn alloc_guest_mem(size_bytes: u64) -> Result<*mut libc::c_void> {
        let ptr = unsafe {
            libc::mmap(
                std::ptr::null_mut(),
                size_bytes as usize,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_NORESERVE,
                -1,
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            bail!("mmap(guest_mem, {} bytes) failed: {}", size_bytes, std::io::Error::last_os_error());
        }
        Ok(ptr)
    }
}

impl Drop for ZenithVmm {
    fn drop(&mut self) {
        tracing::debug!("ZenithVmm dropped — closing /dev/kvm fd");
        // kvm_fd is a File, so it drops automatically
    }
}
