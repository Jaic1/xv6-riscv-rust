//! Inode-relevant operations

use crate::spinlock::SpinLock;

use super::Inode;
use super::NINODE;

static mut ICACHE: Icache = Icache::new();

struct Icache {
    lock: SpinLock<()>,
    inodes: [Inode; NINODE],
}

impl Icache {
    const fn new() -> Self {
        Self {
            lock: SpinLock::new((), "icache"),
            inodes: [Inode::new(); NINODE],
        }
    }
}

/// Find the inode with number inum on device dev
/// and return the in-memory copy. Does not lock
/// the inode and does not read it from disk.
pub fn iget(dev: u32, inum: u32) -> &'static Inode {
    let icache = unsafe {ICACHE.lock.lock()};

    // Is the inode we are looking for already cached?
    let mut empty: Option<&mut Inode> = None;
    for ip in unsafe {ICACHE.inodes.iter_mut()} {
        if ip.iref > 0 && ip.dev == dev && ip.inum == inum {
            ip.iref += 1;
            drop(icache);
            return ip;
        }
        if empty.is_none() && ip.iref == 0 {
            empty = Some(ip);
        }
    }

    // Recycle an inode cacahe entry
    if empty.is_none() {
        panic!("iget: no enough space in inode cache");
    }
    let ip: &mut Inode = empty.take().unwrap();
    ip.dev = dev;
    ip.inum = inum;
    ip.iref = 1;
    ip.valid = false;
    drop(icache);
    ip
}

/// Lock the given inode.
/// Reads the inode from disk if necessary.
pub fn ilock(ip: &mut Inode) {
    if ip.iref < 1 {
        panic!("ilock: iref smaller than 1");
    }

    // acquire sleep lock

    if !ip.valid {
        // bp = bread(ip.dev, )
    }
}

#[cfg(feature = "unit_test")]
pub mod tests {
    pub fn inode_test() {
        println!("inode test ...pass!");
    }
}
