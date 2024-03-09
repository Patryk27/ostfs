use super::{FsError, FsResult};
use crate::{Filesystem, InodeId, Object};
use anyhow::Context;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use tracing::{debug, instrument};

impl Filesystem {
    #[instrument(skip(self))]
    pub fn rename(
        &mut self,
        old_parent_iid: InodeId,
        old_name: &OsStr,
        new_parent_iid: InodeId,
        new_name: &OsStr,
    ) -> FsResult<()> {
        debug!("op: rename()");

        if !self.source.is_writable() {
            return Err(FsError::ReadOnly);
        }

        self.begin_tx()?;
        let (iid, _) = self.find(old_parent_iid, old_name)?;

        if old_parent_iid == new_parent_iid {
            if new_name == old_name {
                return Ok(());
            }

            let new_oid = self.clone_inode(iid)?;
            let mut obj = self.objects.get(new_oid)?.into_entry(new_oid)?;

            obj.name = self
                .objects
                .alloc_payload(Some(&mut self.tx), new_name.as_bytes())?
                .context("got an empty name")?;

            self.objects.set(new_oid, Object::Entry(obj))?;
            self.commit_tx()?;

            Ok(())
        } else {
            Err(FsError::NotImplemented)
        }
    }
}
