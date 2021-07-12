use crate::consts::fs::{DIRSIZE, ROOTDEV, ROOTINUM};
use super::Inode;

// TODO
pub fn namei(path: &[u8]) -> &Inode {
    let mut name: [u8; DIRSIZE] = [0; DIRSIZE];
    namex(path, false, &mut name)
}

// TODO
fn namex(_path: &[u8], _nameparent: bool, _name: &mut [u8]) -> &'static Inode {
    todo!()
}
