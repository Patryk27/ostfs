use super::{FsError, FsResult};
use crate::{Filesystem, InodeId};
use fuser::FileAttr;
use tracing::{debug, instrument};

impl Filesystem {
    #[instrument(skip(self))]
    pub fn getattr(&mut self, iid: InodeId) -> FsResult<FileAttr> {
        debug!("op: getattr()");

        let oid = self
            .inodes
            .resolve_object(iid)
            .ok()
            .ok_or(FsError::NotFound)?;

        let obj = self.objects.get(oid)?.into_entry(oid)?;

        Ok(Self::attr(iid, obj))
    }
}
