use super::Inode;

pub struct File {
    ftype: FileType,
    readable: bool,
    writable: bool,
}

enum FileType {
    None,
    Pipe,
    Inode(Inode, u32),
    Device(Inode, u16),
}
