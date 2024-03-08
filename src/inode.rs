use crate::{InodeId, ObjectId};

#[derive(Debug)]
pub struct Inode {
    pub oid: ObjectId,
    pub parent: InodeId,
    pub children: Option<Vec<InodeId>>,
}
