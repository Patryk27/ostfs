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
    source: FilesystemSource,
    tx: Transaction,
}

impl Filesystem {
    pub fn new(mut objects: Objects, source: FilesystemSource) -> Result<Self> {
        let inodes = Inodes::new(source.root_oid(&mut objects)?)?;

        Ok(Self {
            objects,
            inodes,
            source,
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
            .commit(&mut self.objects, Some(&mut self.inodes), self.source)?;

        Ok(())
    }

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

    #[instrument(skip(self))]
    fn add_child(&mut self, parent_oid: ObjectId, child: Object) -> Result<ObjectId> {
        let parent = self.objects.get(parent_oid)?.into_entry(parent_oid)?;
        let child_oid = self.objects.alloc(Some(&mut self.tx), child)?;

        if let Some(mut oid) = parent.body {
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
}

#[derive(Clone, Copy, Debug)]
pub enum FilesystemSource {
    Original { is_writable: bool },
    Clone { oid: ObjectId, is_writable: bool },
}

impl FilesystemSource {
    fn root_oid(self, objects: &mut Objects) -> Result<ObjectId> {
        match self {
            FilesystemSource::Original { .. } => Ok(objects.get_header()?.root),
            FilesystemSource::Clone { oid, .. } => Ok(objects.get(oid)?.into_clone(oid)?.root),
        }
    }

    fn is_writable(self) -> bool {
        match self {
            FilesystemSource::Original { is_writable } => is_writable,
            FilesystemSource::Clone { is_writable, .. } => is_writable,
        }
    }
}
