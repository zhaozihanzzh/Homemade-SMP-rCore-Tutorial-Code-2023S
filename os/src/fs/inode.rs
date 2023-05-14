use super::File;
use crate::drivers::BLOCK_DEVICE;
use crate::mm::UserBuffer;
use crate::sync::UPSafeCell;
use alloc::sync::Arc;
use alloc::vec::Vec;
use bitflags::*;
use easy_fs::{EasyFileSystem, Inode};
use lazy_static::*;

/// inode in memory
pub struct OSInode {
    readable: bool,
    writable: bool,
    inner: UPSafeCell<OSInodeInner>,
}
/// inner of inode in memory
pub struct OSInodeInner {
    offset: usize,
    inode: Arc<Inode>,
}

impl OSInode {
    /// create a new inode in memory
    pub fn new(readable: bool, writable: bool, inode: Arc<Inode>) -> Self {
        trace!("kernel: OSInode::new");
        Self {
            readable,
            writable,
            inner: unsafe { UPSafeCell::new(OSInodeInner { offset: 0, inode }) },
        }
    }
    /// read all data from the inode in memory
    pub fn read_all(&self) -> Vec<u8> {
        trace!("kernel: OSInode::read_all");
        let mut inner = self.inner.exclusive_access();
        let mut buffer = [0u8; 512];
        let mut v: Vec<u8> = Vec::new();
        loop {
            let len = inner.inode.read_at(inner.offset, &mut buffer);
            if len == 0 {
                break;
            }
            inner.offset += len;
            v.extend_from_slice(&buffer[..len]);
        }
        v
    }
    /// Get Inode id
    pub fn get_inode_id(&self) -> u32 {
        self.inner.exclusive_access().inode.get_inode_id()
    }
    /// Get whether is dir
    pub fn get_inode_is_dir(&self) -> bool {
        self.inner.exclusive_access().inode.get_inode_is_dir()
    }
}

lazy_static! {
    pub static ref ROOT_INODE: Arc<Inode> = {
        let efs = EasyFileSystem::open(BLOCK_DEVICE.clone());
        Arc::new(EasyFileSystem::root_inode(&efs))
    };
}

/// Create hard link in root dir
pub fn create_link(old_name: &str, new_name: &str) -> isize {
    // let current_files = ROOT_INODE.ls();
    // println!("Before linking: {}", cu)
    if let Some(inode) = ROOT_INODE.insert_link_entry(old_name, new_name) {
        println!("create link at inode {}", inode.get_inode_id());
        0
    } else {
        println!("create_link returns -1, {} not exist", old_name);
        -1
    }
}

/// Remove hard link in root dir
pub fn delete_link(name: &str) -> isize {
    let ret = ROOT_INODE.remove_link_entry(name);
    if ret == 0 {
        println!("delete link at {} succeeded", name);
    } else {
        println!("delete link at {} failed, return {}", name, ret);
    }
    ret
}

/// Get number of hard link in root
pub fn get_link_count(inode_id: u32) -> u32 {
    let count = ROOT_INODE.get_link_count_from_root(inode_id);
    println!("inode_id {} link {} count", inode_id, count);
    count
}

/// List all apps in the root directory
pub fn list_apps() {
    println!("/**** APPS ****");
    for app in ROOT_INODE.ls() {
        println!("{}", app);
    }
    println!("**************/");
}

bitflags! {
    ///  The flags argument to the open() system call is constructed by ORing together zero or more of the following values:
    pub struct OpenFlags: u32 {
        /// readyonly
        const RDONLY = 0;
        /// writeonly
        const WRONLY = 1 << 0;
        /// read and write
        const RDWR = 1 << 1;
        /// create new file
        const CREATE = 1 << 9;
        /// truncate file size to 0
        const TRUNC = 1 << 10;
    }
}

impl OpenFlags {
    /// Do not check validity for simplicity
    /// Return (readable, writable)
    pub fn read_write(&self) -> (bool, bool) {
        if self.is_empty() {
            (true, false)
        } else if self.contains(Self::WRONLY) {
            (false, true)
        } else {
            (true, true)
        }
    }
}

/// Open a file
pub fn open_file(name: &str, flags: OpenFlags) -> Option<Arc<OSInode>> {
    trace!("kernel: open_file: name = {}, flags = {:?}", name, flags);
    let (readable, writable) = flags.read_write();
    if flags.contains(OpenFlags::CREATE) {
        if let Some(inode) = ROOT_INODE.find(name) {
            // clear size
            inode.clear();
            Some(Arc::new(OSInode::new(readable, writable, inode)))
        } else {
            // create file
            ROOT_INODE
                .create(name)
                .map(|inode| Arc::new(OSInode::new(readable, writable, inode)))
        }
    } else {
        ROOT_INODE.find(name).map(|inode| {
            if flags.contains(OpenFlags::TRUNC) {
                inode.clear();
            }
            Arc::new(OSInode::new(readable, writable, inode))
        })
    }
}

impl File for OSInode {
    /// file readable?
    fn readable(&self) -> bool {
        self.readable
    }
    /// file writable?
    fn writable(&self) -> bool {
        self.writable
    }
    /// read file data into buffer
    fn read(&self, mut buf: UserBuffer) -> usize {
        trace!("kernel: OSInode::read");
        let mut inner = self.inner.exclusive_access();
        let mut total_read_size = 0usize;
        for slice in buf.buffers.iter_mut() {
            let read_size = inner.inode.read_at(inner.offset, *slice);
            if read_size == 0 {
                break;
            }
            inner.offset += read_size;
            total_read_size += read_size;
        }
        total_read_size
    }
    /// write buffer data into file
    fn write(&self, buf: UserBuffer) -> usize {
        trace!("kernel: OSInode::write");
        let mut inner = self.inner.exclusive_access();
        let mut total_write_size = 0usize;
        for slice in buf.buffers.iter() {
            let write_size = inner.inode.write_at(inner.offset, *slice);
            assert_eq!(write_size, slice.len());
            inner.offset += write_size;
            total_write_size += write_size;
        }
        total_write_size
    }
    fn as_any(&self) -> &dyn _core::any::Any {
        self
    }
}
