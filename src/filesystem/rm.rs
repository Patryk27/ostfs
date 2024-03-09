use super::{FsError, FsResult};
use crate::{Filesystem, InodeId};
use std::ffi::OsStr;
use tracing::{debug, instrument};

impl Filesystem {
    #[instrument(skip(self))]
    pub fn unlink(&mut self, parent_iid: InodeId, name: &OsStr) -> FsResult<()> {
        debug!("op: unlink()");

        self.rm(parent_iid, name)
    }

    #[instrument(skip(self))]
    pub fn rmdir(&mut self, parent_iid: InodeId, name: &OsStr) -> FsResult<()> {
        debug!("op: rmdir()");

        self.rm(parent_iid, name)
    }

    fn rm(&mut self, parent_iid: InodeId, name: &OsStr) -> FsResult<()> {
        if !self.source.is_writable() {
            return Err(FsError::ReadOnly);
        }

        // Removing entries is pretty simple - just clone the entire tree, but
        // without given child:

        self.begin_tx()?;
        let iid = self.find(parent_iid, name)?.0;
        self.delete_inode(iid)?;
        self.commit_tx()?;

        Ok(())
    }
}
