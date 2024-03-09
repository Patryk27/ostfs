mod alter;
mod getattr;
mod lookup;
mod mk;
mod read;
mod readdir;
mod rename;
mod result;
mod rm;
mod setattr;
mod write;

use self::result::*;
use crate::{EntryObj, InodeId, Inodes, Object, ObjectId, Objects, Transaction};
use anyhow::Result;
use fuser::{FileAttr, FileType};
use std::ffi::OsStr;
use std::time::UNIX_EPOCH;
use tracing::instrument;

#[derive(Debug)]
pub struct Filesystem {
    objects: Objects,
    inodes: Inodes,
    origin: FilesystemOrigin,
    tx: Transaction,
}

impl Filesystem {
    pub fn new(mut objects: Objects, origin: FilesystemOrigin) -> Result<Self> {
        let inodes = Inodes::new(origin.root_oid(&mut objects)?)?;

        Ok(Self {
            objects,
            inodes,
            origin,
            tx: Default::default(),
        })
    }

    fn attr(iid: InodeId, obj: EntryObj) -> FileAttr {
        const BASE_ATTR: FileAttr = FileAttr {
            ino: 0,
            size: 0,
            blocks: 0,
            atime: UNIX_EPOCH,
            mtime: UNIX_EPOCH,
            ctime: UNIX_EPOCH,
            crtime: UNIX_EPOCH,
            kind: FileType::RegularFile,
            perm: 0,
            nlink: 1,
            uid: 0,
            gid: 0,
            rdev: 0,
            flags: 0,
            blksize: 512,
        };

        FileAttr {
            ino: iid.get(),
            kind: obj.kind,
            size: obj.size as u64,
            perm: obj.mode,
            uid: obj.uid,
            gid: obj.gid,
            ..BASE_ATTR
        }
    }

    fn begin_tx(&mut self) -> Result<()> {
        self.tx.begin(&mut self.objects)?;

        Ok(())
    }

    fn commit_tx(&mut self) -> Result<()> {
        self.tx
            .commit(&mut self.objects, Some(&mut self.inodes), self.origin)?;

        Ok(())
    }

    /// Appends object to parent (i.e. adds a file/directory into a directory).
    ///
    /// Note that this function works in-place, i.e. it assumes that the parent
    /// has been already cloned.
    #[instrument(skip(self))]
    fn append(&mut self, parent_oid: ObjectId, child: Object) -> Result<ObjectId> {
        let parent = self.objects.get(parent_oid)?.into_entry(parent_oid)?;
        let child_oid = self.objects.alloc(Some(&mut self.tx), child)?;

        if let Some(mut oid) = parent.body {
            // If the parent already has a child, find the linked list's tail
            // and append our object there

            loop {
                let obj = self.objects.get(oid)?.into_entry(oid)?;

                if let Some(next) = obj.next {
                    oid = next;
                } else {
                    self.objects.set(
                        oid,
                        Object::Entry(EntryObj {
                            next: Some(child_oid),
                            ..obj
                        }),
                    )?;

                    break;
                }
            }
        } else {
            // If our parent has no children (it's an empty directory), then
            // easy peasy - just modify the parent

            self.objects.set(
                parent_oid,
                Object::Entry(EntryObj {
                    body: Some(child_oid),
                    ..parent
                }),
            )?;
        }

        Ok(child_oid)
    }

    /// Goes through inode's children and looks for the one with given name.
    ///
    /// If no such child exists, bails out with [`FsError::NotFound`].
    #[instrument(skip(self))]
    fn find(&mut self, parent_iid: InodeId, name: &OsStr) -> FsResult<(InodeId, EntryObj)> {
        let children = self
            .inodes
            .resolve_children(&mut self.objects, parent_iid)
            .ok()
            .ok_or(FsError::NotFound)?;

        for iid in children {
            let oid = self.inodes.resolve_object(iid)?;
            let obj = self.objects.get(oid)?.into_entry(oid)?;

            if self.objects.get_os_string(obj.name)? == name {
                return Ok((iid, obj));
            }
        }

        Err(FsError::NotFound)
    }
}

#[derive(Clone, Copy, Debug)]
pub enum FilesystemOrigin {
    Main { is_writable: bool },
    Clone { oid: ObjectId, is_writable: bool },
}

impl FilesystemOrigin {
    fn root_oid(self, objects: &mut Objects) -> Result<ObjectId> {
        match self {
            FilesystemOrigin::Main { .. } => Ok(objects.get_header()?.root),
            FilesystemOrigin::Clone { oid, .. } => Ok(objects.get(oid)?.into_clone(oid)?.root),
        }
    }

    fn is_writable(self) -> bool {
        match self {
            FilesystemOrigin::Main { is_writable } => is_writable,
            FilesystemOrigin::Clone { is_writable, .. } => is_writable,
        }
    }
}
