use crate::consts::fs::{DIRSIZ, ROOTDEV, ROOTINO};
use super::Inode;

// TODO
pub fn namei(path: &[u8]) -> &Inode {
    let mut name: [u8; DIRSIZ] = [0; DIRSIZ];
    namex(path, false, &mut name)
}

// TODO
fn namex(_path: &[u8], _nameparent: bool, _name: &mut [u8]) -> &'static Inode {
    todo!()
}
