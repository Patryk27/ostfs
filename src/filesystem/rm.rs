use super::{Alter, AlterResult, FsError, FsResult};
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
        if !self.is_writable {
            return Err(FsError::ReadOnly);
        }

        self.begin_tx()?;

        let (child_iid, _) = self.find(parent_iid, name)?;

        let AlterResult {
            new_root_oid,
            changeset,
            ..
        } = self.alter(Alter::delete(child_iid))?;

        self.tx.update_root(new_root_oid, changeset)?;
        self.commit_tx()?;

        Ok(())
    }
}
