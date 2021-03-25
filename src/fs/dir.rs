use crate::consts::fs::{DIRSIZ, ROOTDEV, ROOTINO};

use super::Inode;
use super::inode::iget;

pub fn namei(path: &[u8]) -> &Inode {
    let mut name: [u8; DIRSIZ] = [0; DIRSIZ];
    namex(path, false, &mut name)
}

// TODO
fn namex(path: &[u8], nameparent: bool, _name: &mut [u8]) -> &'static Inode {
    if path[0] != b'/' {
        panic!("namex: path={:?}, not start as root", path);
    }
    if nameparent {
        panic!("namex: nameparent not supported yet");
    }

    let ip = iget(ROOTDEV, ROOTINO);
    
    ip
}