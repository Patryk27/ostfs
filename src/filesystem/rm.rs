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
        if !self.origin.is_writable() {
            return Err(FsError::ReadOnly);
        }

        self.begin_tx()?;
        let iid = self.find_node(parent_iid, name)?.0;
        self.delete_node(iid)?;
        self.commit_tx()?;

        Ok(())
    }
}
