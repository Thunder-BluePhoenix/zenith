/// ZenithVm — a single KVM virtual machine.
///
/// Responsibilities:
///   - Load a kernel image into guest memory
///   - Setup initial CPU state (x86_64 protected mode → long mode)
///   - Run the VCPU event loop until the guest signals completion
///   - Snapshot and restore the full VCPU+memory state for warm-pool reuse

use anyhow::{bail, Context, Result};
use std::os::unix::io::{AsRawFd, OwnedFd};
use nix::{ioctl_read, ioctl_write_ptr};
use serde::{Deserialize, Serialize};

// ─── KVM data structures ──────────────────────────────────────────────────────

const KVMIO: u8 = 0xAE;

/// Guest memory slot descriptor passed to KVM_SET_USER_MEMORY_REGION
#[repr(C)]
pub struct KvmUserspaceMemoryRegion {
    pub slot:            u32,
    pub flags:           u32,
    pub guest_phys_addr: u64,
    pub memory_size:     u64,
    pub userspace_addr:  u64,
}

/// x86_64 general-purpose + special registers (KVM_GET_REGS / KVM_SET_REGS)
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct KvmRegs {
    pub rax: u64, pub rbx: u64, pub rcx: u64, pub rdx: u64,
    pub rsi: u64, pub rdi: u64, pub rsp: u64, pub rbp: u64,
    pub r8:  u64, pub r9:  u64, pub r10: u64, pub r11: u64,
    pub r12: u64, pub r13: u64, pub r14: u64, pub r15: u64,
    pub rip: u64, pub rflags: u64,
}

/// Segment descriptor (part of KvmSregs)
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct KvmSegment {
    pub base:     u64,
    pub limit:    u32,
    pub selector: u16,
    pub type_:    u8,
    pub present:  u8,
    pub dpl:      u8,
    pub db:       u8,
    pub s:        u8,
    pub l:        u8,
    pub g:        u8,
    pub avl:      u8,
    pub unusable: u8,
    pub padding:  u8,
}

/// x86_64 special/system registers (KVM_GET_SREGS / KVM_SET_SREGS)
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct KvmSregs {
    pub cs:   KvmSegment, pub ds: KvmSegment,
    pub es:   KvmSegment, pub fs: KvmSegment,
    pub gs:   KvmSegment, pub ss: KvmSegment,
    pub tr:   KvmSegment, pub ldt: KvmSegment,
    pub gdt:  [u64; 2],   pub idt: [u64; 2],
    pub cr0:  u64, pub cr2: u64, pub cr3: u64, pub cr4: u64, pub cr8: u64,
    pub efer: u64, pub apic_base: u64,
    pub interrupt_bitmap: [u64; 4],
}

// KVM register ioctls (on VCPU fd)
ioctl_read! (kvm_get_regs,  KVMIO, 0x81, KvmRegs);
ioctl_write_ptr!(kvm_set_regs,  KVMIO, 0x82, KvmRegs);
ioctl_read! (kvm_get_sregs, KVMIO, 0x83, KvmSregs);
ioctl_write_ptr!(kvm_set_sregs, KVMIO, 0x84, KvmSregs);

// KVM_RUN — run the VCPU until it exits back to userspace
nix::ioctl_none!(kvm_run, KVMIO, 0x80);

// ─── VM snapshot (serialisable CPU + memory state) ───────────────────────────

/// Full VM state snapshot — persisted for warm-pool restore.
/// The guest memory image is stored separately as a raw byte blob on disk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VmSnapshot {
    pub id:         String,
    /// Serialised KvmRegs (general-purpose registers)
    pub regs_bytes: Vec<u8>,
    /// Serialised KvmSregs (system registers)
    pub sregs_bytes: Vec<u8>,
    /// Path to the memory image file (`snapshot/<id>/mem.raw`)
    pub mem_image:  std::path::PathBuf,
    /// Guest memory size in bytes
    pub mem_size:   u64,
}

impl VmSnapshot {
    /// Save this snapshot to disk under `dir/<id>/`.
    pub fn save(&self, dir: &std::path::Path) -> Result<()> {
        let dest = dir.join(&self.id);
        std::fs::create_dir_all(&dest)?;
        let meta_path = dest.join("snapshot.json");
        let meta_json = serde_json::to_string_pretty(self)?;
        std::fs::write(meta_path, meta_json)?;
        Ok(())
    }

    /// Load a snapshot from `dir/<id>/snapshot.json`.
    pub fn load(dir: &std::path::Path, id: &str) -> Result<Self> {
        let meta_path = dir.join(id).join("snapshot.json");
        let json = std::fs::read_to_string(&meta_path)
            .with_context(|| format!("Cannot read snapshot metadata {:?}", meta_path))?;
        let snap: VmSnapshot = serde_json::from_str(&json)?;
        Ok(snap)
    }
}

// ─── ZenithVm ─────────────────────────────────────────────────────────────────

/// A single Zenith microVM instance.
pub struct ZenithVm {
    vm_fd:        OwnedFd,
    vcpu_fd:      OwnedFd,
    guest_mem:    *mut libc::c_void,
    guest_mem_sz: u64,
    kvm_run_ptr:  *mut libc::c_void,
    kvm_run_size: usize,
    pub id:       String,
}

// SAFETY: ZenithVm owns all raw pointers; we never alias them across threads.
unsafe impl Send for ZenithVm {}
unsafe impl Sync for ZenithVm {}

impl ZenithVm {
    /// Called by `ZenithVmm::create_vm()` after all KVM setup is complete.
    pub fn new(
        vm_fd:        OwnedFd,
        vcpu_fd:      OwnedFd,
        guest_mem:    *mut libc::c_void,
        guest_mem_sz: u64,
        kvm_run_ptr:  *mut libc::c_void,
        kvm_run_size: usize,
    ) -> Self {
        Self {
            vm_fd, vcpu_fd, guest_mem, guest_mem_sz,
            kvm_run_ptr, kvm_run_size,
            id: uuid::Uuid::new_v4().to_string(),
        }
    }

    /// Read the current VCPU general-purpose registers.
    pub fn get_regs(&self) -> Result<KvmRegs> {
        let mut regs = KvmRegs::default();
        unsafe { kvm_get_regs(self.vcpu_fd.as_raw_fd(), &mut regs) }
            .context("KVM_GET_REGS failed")?;
        Ok(regs)
    }

    /// Write VCPU general-purpose registers.
    pub fn set_regs(&self, regs: &KvmRegs) -> Result<()> {
        unsafe { kvm_set_regs(self.vcpu_fd.as_raw_fd(), regs) }
            .context("KVM_SET_REGS failed")?;
        Ok(())
    }

    /// Read VCPU system/special registers.
    pub fn get_sregs(&self) -> Result<KvmSregs> {
        let mut sregs = KvmSregs::default();
        unsafe { kvm_get_sregs(self.vcpu_fd.as_raw_fd(), &mut sregs) }
            .context("KVM_GET_SREGS failed")?;
        Ok(sregs)
    }

    /// Write VCPU system/special registers.
    pub fn set_sregs(&self, sregs: &KvmSregs) -> Result<()> {
        unsafe { kvm_set_sregs(self.vcpu_fd.as_raw_fd(), sregs) }
            .context("KVM_SET_SREGS failed")?;
        Ok(())
    }

    /// Run the VCPU until it exits back to userspace (I/O, halt, shutdown, etc.)
    /// Returns the exit reason code from the `kvm_run` struct.
    pub fn run_vcpu(&self) -> Result<u32> {
        unsafe { kvm_run(self.vcpu_fd.as_raw_fd()) }
            .context("KVM_RUN failed")?;

        // Read exit_reason from the shared kvm_run struct.
        // kvm_run.exit_reason is at offset 8 bytes (after request + padding).
        let exit_reason = unsafe {
            let ptr = (self.kvm_run_ptr as *const u8).add(8) as *const u32;
            ptr.read_volatile()
        };
        Ok(exit_reason)
    }

    // ─── Snapshot / Restore ───────────────────────────────────────────────────

    /// Capture the full VM state (CPU registers + guest memory image).
    /// The memory image is written to `snap_dir/<id>/mem.raw`.
    pub fn snapshot(&self, snap_dir: &std::path::Path) -> Result<VmSnapshot> {
        let snap_id = uuid::Uuid::new_v4().to_string();
        let dest_dir = snap_dir.join(&snap_id);
        std::fs::create_dir_all(&dest_dir)?;

        // Save CPU register state
        let regs  = self.get_regs()?;
        let sregs = self.get_sregs()?;

        let regs_bytes: Vec<u8> = unsafe {
            std::slice::from_raw_parts(
                &regs as *const KvmRegs as *const u8,
                std::mem::size_of::<KvmRegs>(),
            ).to_vec()
        };
        let sregs_bytes: Vec<u8> = unsafe {
            std::slice::from_raw_parts(
                &sregs as *const KvmSregs as *const u8,
                std::mem::size_of::<KvmSregs>(),
            ).to_vec()
        };

        // Write memory image
        let mem_path = dest_dir.join("mem.raw");
        let mem_slice = unsafe {
            std::slice::from_raw_parts(self.guest_mem as *const u8, self.guest_mem_sz as usize)
        };
        std::fs::write(&mem_path, mem_slice)?;

        let snap = VmSnapshot {
            id:          snap_id,
            regs_bytes,
            sregs_bytes,
            mem_image:   mem_path,
            mem_size:    self.guest_mem_sz,
        };
        snap.save(snap_dir)?;

        tracing::debug!(id = %snap.id, mem_bytes = snap.mem_size, "VM snapshot saved");
        Ok(snap)
    }

    /// Restore CPU and memory state from a snapshot.
    /// The VM fd and VCPU fd must already exist (created fresh by `ZenithVmm::create_vm()`).
    pub fn restore(&self, snap: &VmSnapshot) -> Result<()> {
        if snap.mem_size != self.guest_mem_sz {
            bail!(
                "Snapshot memory size {} != VM memory size {}",
                snap.mem_size, self.guest_mem_sz
            );
        }

        // Restore guest memory
        let mem_data = std::fs::read(&snap.mem_image)
            .with_context(|| format!("Cannot read memory image {:?}", snap.mem_image))?;
        unsafe {
            std::ptr::copy_nonoverlapping(
                mem_data.as_ptr(),
                self.guest_mem as *mut u8,
                mem_data.len(),
            );
        }

        // Restore registers
        let regs = unsafe {
            assert_eq!(snap.regs_bytes.len(), std::mem::size_of::<KvmRegs>());
            let mut r = KvmRegs::default();
            std::ptr::copy_nonoverlapping(snap.regs_bytes.as_ptr(), &mut r as *mut KvmRegs as *mut u8, snap.regs_bytes.len());
            r
        };
        let sregs = unsafe {
            assert_eq!(snap.sregs_bytes.len(), std::mem::size_of::<KvmSregs>());
            let mut s = KvmSregs::default();
            std::ptr::copy_nonoverlapping(snap.sregs_bytes.as_ptr(), &mut s as *mut KvmSregs as *mut u8, snap.sregs_bytes.len());
            s
        };
        self.set_regs(&regs)?;
        self.set_sregs(&sregs)?;

        tracing::debug!(id = %snap.id, "VM state restored from snapshot");
        Ok(())
    }
}

impl Drop for ZenithVm {
    fn drop(&mut self) {
        unsafe {
            if !self.kvm_run_ptr.is_null() {
                libc::munmap(self.kvm_run_ptr, self.kvm_run_size);
            }
            if !self.guest_mem.is_null() {
                libc::munmap(self.guest_mem, self.guest_mem_sz as usize);
            }
        }
        // vm_fd and vcpu_fd are OwnedFd — drop automatically closes them
    }
}
