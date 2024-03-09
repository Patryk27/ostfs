use super::{FsError, FsResult};
use crate::{Filesystem, InodeId, Object};
use fuser::FileAttr;
use tracing::{debug, instrument};

impl Filesystem {
    #[instrument(skip(self))]
    pub fn setattr(
        &mut self,
        iid: InodeId,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        size: Option<u64>,
    ) -> FsResult<FileAttr> {
        debug!("op: setattr()");

        if !self.source.is_writable() {
            return Err(FsError::ReadOnly);
        }

        self.begin_tx()?;

        let new_oid = self.clone_inode(iid)?;
        let mut obj = self.objects.get(new_oid)?.into_entry(new_oid)?;

        if let Some(mode) = mode {
            obj.mode = mode as u16;
        }

        if let Some(uid) = uid {
            obj.uid = uid;
        }

        if let Some(gid) = gid {
            obj.gid = gid;
        }

        let truncate = match size {
            Some(0) => true,
            Some(_) => return Err(FsError::NotImplemented),
            None => false,
        };

        if truncate {
            obj.size = 0;
            obj.body = None;
        }

        self.objects.set(new_oid, Object::Entry(obj))?;
        self.commit_tx()?;

        Ok(Self::attr(iid, obj))
    }
}
