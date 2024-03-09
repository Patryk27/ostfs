use super::{FsError, FsResult};
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
        if !self.origin.is_writable() {
            return Err(FsError::ReadOnly);
        }

        self.begin_tx()?;

        let name_oid = self
            .objects
            .alloc_payload(Some(&mut self.tx), name.as_bytes())?
            .context("got an empty name")?;

        let new_parent_oid = self.clone_inode(parent_iid)?;

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

        let new_oid = self.append(new_parent_oid, Object::Entry(obj))?;

        self.commit_tx()?;

        let new_iid = self.inodes.alloc(parent_iid, new_oid)?;

        Ok(Self::attr(new_iid, obj))
    }
}
