use crate::{InodeId, Inodes, ObjectId};

#[derive(Debug, Default)]
pub struct InodeChangeset {
    to_remap: Vec<(InodeId, ObjectId)>,
    to_free: Vec<InodeId>,
}

impl InodeChangeset {
    pub fn remap(&mut self, src: InodeId, dst: ObjectId) {
        self.to_remap.push((src, dst));
    }

    pub fn free(&mut self, iid: InodeId) {
        self.to_free.push(iid);
    }

    pub fn is_empty(&self) -> bool {
        self.to_remap.is_empty() && self.to_free.is_empty()
    }

    pub fn apply_to(self, inodes: &mut Inodes) {
        for (iid, oid) in self.to_remap {
            inodes.remap(iid, oid);
        }

        for iid in self.to_free {
            inodes.free(iid);
        }
    }
}
