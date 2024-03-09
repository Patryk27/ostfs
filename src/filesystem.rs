mod getattr;
mod lookup;
mod mk;
mod ops;
mod read;
mod readdir;
mod rename;
mod result;
mod rm;
mod setattr;
mod write;

use self::result::*;
use crate::{Collector, EntryObj, InodeId, Inodes, ObjectId, Objects, Transaction};
use anyhow::{Context, Result};
use fuser::{FileAttr, FileType};
use std::time::UNIX_EPOCH;

#[derive(Debug)]
pub struct Filesystem {
    objects: Objects,
    inodes: Inodes,
    origin: FilesystemOrigin,
    tx: Transaction,
    tx_since_last_gc: u32,
}

impl Filesystem {
    pub fn new(mut objects: Objects, origin: FilesystemOrigin) -> Result<Self> {
        Collector::new(&mut objects)
            .run()
            .context("garbage collection failed")?;

        let inodes = Inodes::new(origin.root_oid(&mut objects)?)?;

        Ok(Self {
            objects,
            inodes,
            origin,
            tx: Default::default(),
            tx_since_last_gc: 0,
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
        let got_changes = self
            .tx
            .commit(&mut self.objects, Some(&mut self.inodes), self.origin)?;

        if got_changes {
            self.tx_since_last_gc += 1;
        }

        if self.tx_since_last_gc >= 250 {
            Collector::new(&mut self.objects)
                .run()
                .context("garbage collection failed")?;

            self.tx_since_last_gc = 0;
        }

        Ok(())
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
