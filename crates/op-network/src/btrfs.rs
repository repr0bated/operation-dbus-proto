//! Native btrfs device operations via ioctl — no CLI subprocesses.
//!
//! The identity-sled persistence model attaches a per-container writable
//! block device to a mounted (seed-image) btrfs filesystem with
//! `BTRFS_IOC_ADD_DEV` — the same ioctl `btrfs device add` issues — so no
//! overlay layers and no subvolume snapshots are involved.

use std::ffi::CString;
use std::fs::File;
use std::os::fd::AsRawFd;
use std::path::Path;

use anyhow::{bail, Context, Result};

/// `BTRFS_PATH_NAME_MAX` from `linux/btrfs.h`.
const BTRFS_PATH_NAME_MAX: usize = 4087;

/// `struct btrfs_ioctl_vol_args` from `linux/btrfs.h`.
#[repr(C)]
struct BtrfsIoctlVolArgs {
    fd: i64,
    name: [u8; BTRFS_PATH_NAME_MAX + 1],
}

// _IOW(BTRFS_IOCTL_MAGIC=0x94, 10, struct btrfs_ioctl_vol_args)
// = 0x40000000 | (sizeof(vol_args)=4096 << 16) | (0x94 << 8) | 10
const BTRFS_IOC_ADD_DEV: libc::c_ulong = 0x5000_940a;
const _: () = assert!(std::mem::size_of::<BtrfsIoctlVolArgs>() == 4096);

/// Attach `device` to the mounted btrfs filesystem at `mount_point`
/// (equivalent to `btrfs device add <device> <mount_point>`, but issued
/// natively). The caller is responsible for the device being a real block
/// device (or loop-backed image) it intends to hand to this filesystem —
/// the kernel wipes and claims it.
pub fn device_add(device: &Path, mount_point: &Path) -> Result<()> {
    let dev_str = device.to_str().context("device path is not valid UTF-8")?;
    if dev_str.len() > BTRFS_PATH_NAME_MAX {
        bail!("device path exceeds BTRFS_PATH_NAME_MAX");
    }
    let dev_c = CString::new(dev_str).context("device path contains NUL")?;

    let dir = File::open(mount_point)
        .with_context(|| format!("open btrfs mount point {}", mount_point.display()))?;

    let mut args = BtrfsIoctlVolArgs {
        fd: 0,
        name: [0u8; BTRFS_PATH_NAME_MAX + 1],
    };
    let bytes = dev_c.as_bytes_with_nul();
    args.name[..bytes.len()].copy_from_slice(bytes);

    // SAFETY: `dir` is a valid open fd and `args` is a properly initialized,
    // NUL-terminated btrfs_ioctl_vol_args matching the kernel ABI.
    let rc = unsafe { libc::ioctl(dir.as_raw_fd(), BTRFS_IOC_ADD_DEV, &mut args) };
    if rc != 0 {
        let err = std::io::Error::last_os_error();
        bail!(
            "BTRFS_IOC_ADD_DEV({} -> {}) failed: {err}",
            device.display(),
            mount_point.display()
        );
    }
    Ok(())
}
