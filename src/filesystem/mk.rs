use super::{Alter, AlterResult, FsError, FsResult};
use crate::{EntryObj, Filesystem, InodeId, Object};
use anyhow::Context;
use fuser::{FileAttr, FileType};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use tracing::{debug, instrument};

impl Filesystem {
    #[instrument(skip(self))]
    pub fn mknod(
        &mut self,
        parent_iid: InodeId,
        name: &OsStr,
        mode: u32,
        uid: u32,
        gid: u32,
    ) -> FsResult<FileAttr> {
        debug!("op: mknod()");

        self.mk(parent_iid, name, mode, uid, gid, FileType::RegularFile)
    }

    #[instrument(skip(self))]
    pub fn mkdir(
        &mut self,
        parent_iid: InodeId,
        name: &OsStr,
        mode: u32,
        uid: u32,
        gid: u32,
    ) -> FsResult<FileAttr> {
        debug!("op: mkdir()");

        self.mk(parent_iid, name, mode, uid, gid, FileType::Directory)
    }

    fn mk(
        &mut self,
        parent_iid: InodeId,
        name: &OsStr,
        mode: u32,
        uid: u32,
        gid: u32,
        kind: FileType,
    ) -> FsResult<FileAttr> {
        if !self.is_writable {
            return Err(FsError::ReadOnly);
        }

        self.begin_tx()?;

        let name_oid = self
            .objects
            .alloc_payload(Some(&mut self.tx), name.as_bytes())?
            .context("got an empty name")?;

        let AlterResult {
            new_root_oid,
            new_oid: new_parent_oid,
            changeset,
        } = self.alter(Alter::clone(parent_iid))?;

        let new_parent_oid = new_parent_oid.unwrap();

        let obj = EntryObj {
            name: name_oid,
            body: None,
            next: None,
            kind,
            size: 0,
            mode: mode as u16,
            uid,
            gid,
        };

        let new_oid = self.add_child(new_parent_oid, Object::Entry(obj))?;

        self.tx.update_root(new_root_oid, changeset)?;
        self.commit_tx()?;

        let new_iid = self.inodes.alloc(parent_iid, new_oid)?;

        Ok(Self::attr(new_iid, obj))
    }
}
