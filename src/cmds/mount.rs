use crate::{CloneController, Filesystem, FilesystemOrigin, InodeId, Objects, Storage};
use anyhow::{Context, Result};
use fuser::{MountOption, ReplyAttr, ReplyData, ReplyDirectory, ReplyEntry, Request, TimeOrNow};
use std::ffi::OsStr;
use std::path::PathBuf;
use std::time::{Duration, SystemTime};
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct MountCmd {
    /// Path to the *.ofs file
    src: PathBuf,

    /// Path to the mountpoint
    dst: PathBuf,

    /// When specified, mount given clone
    #[structopt(short, long)]
    clone: Option<String>,

    /// Force a read-only mount
    #[structopt(short, long)]
    read_only: bool,

    /// When *.ofs runs out of space, throw an I/O error instead of growing the
    /// file
    #[structopt(short, long)]
    in_place: bool,
}

impl MountCmd {
    pub fn run(self) -> Result<()> {
        tracing_subscriber::fmt::init();

        let storage = Storage::open(&self.src, !self.in_place)?;
        let mut objects = Objects::new(storage);

        let origin = if let Some(clone) = &self.clone {
            let clone = CloneController::new(&mut objects).find(clone)?;

            FilesystemOrigin::Clone {
                oid: clone.oid,
                is_writable: clone.is_writable && !self.read_only,
            }
        } else {
            FilesystemOrigin::Main {
                is_writable: !self.read_only,
            }
        };

        let fs = Filesystem::new(objects, origin)?;

        let options = vec![
            MountOption::FSName("ofs".into()),
            MountOption::AllowOther,
            MountOption::AutoUnmount,
        ];

        fuser::mount2(FsController { fs }, self.dst, &options)
            .context("Couldn't mount filesystem")?;

        Ok(())
    }
}

#[derive(Debug)]
struct FsController {
    fs: Filesystem,
}

impl FsController {
    const TTL: Duration = Duration::from_secs(0);
}

impl fuser::Filesystem for FsController {
    fn lookup(&mut self, _: &Request, parent: u64, name: &OsStr, reply: ReplyEntry) {
        match self.fs.lookup(InodeId::new(parent), name) {
            Ok(val) => reply.entry(&Self::TTL, &val, 0),
            Err(err) => reply.error(err.log_and_convert()),
        }
    }

    fn getattr(&mut self, _: &Request, ino: u64, reply: ReplyAttr) {
        match self.fs.getattr(InodeId::new(ino)) {
            Ok(val) => reply.attr(&Self::TTL, &val),
            Err(err) => reply.error(err.log_and_convert()),
        }
    }

    fn setattr(
        &mut self,
        _: &Request<'_>,
        ino: u64,
        mode: Option<u32>,
        uid: Option<u32>,
        gid: Option<u32>,
        size: Option<u64>,
        _: Option<TimeOrNow>,
        _: Option<TimeOrNow>,
        _: Option<SystemTime>,
        _: Option<u64>,
        _: Option<SystemTime>,
        _: Option<SystemTime>,
        _: Option<SystemTime>,
        _: Option<u32>,
        reply: ReplyAttr,
    ) {
        match self.fs.setattr(InodeId::new(ino), mode, uid, gid, size) {
            Ok(val) => reply.attr(&Self::TTL, &val),
            Err(err) => reply.error(err.log_and_convert()),
        }
    }

    fn mknod(
        &mut self,
        req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _: u32,
        _: u32,
        reply: ReplyEntry,
    ) {
        match self
            .fs
            .mknod(InodeId::new(parent), name, mode, req.uid(), req.gid())
        {
            Ok(val) => reply.entry(&Self::TTL, &val, 0),
            Err(err) => reply.error(err.log_and_convert()),
        }
    }

    fn mkdir(
        &mut self,
        req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        mode: u32,
        _: u32,
        reply: ReplyEntry,
    ) {
        match self
            .fs
            .mkdir(InodeId::new(parent), name, mode, req.uid(), req.gid())
        {
            Ok(val) => reply.entry(&Self::TTL, &val, 0),
            Err(err) => reply.error(err.log_and_convert()),
        }
    }

    fn unlink(&mut self, _: &Request<'_>, parent: u64, name: &OsStr, reply: fuser::ReplyEmpty) {
        match self.fs.unlink(InodeId::new(parent), name) {
            Ok(_) => reply.ok(),
            Err(err) => reply.error(err.log_and_convert()),
        }
    }

    fn rmdir(&mut self, _: &Request<'_>, parent: u64, name: &OsStr, reply: fuser::ReplyEmpty) {
        match self.fs.unlink(InodeId::new(parent), name) {
            Ok(_) => reply.ok(),
            Err(err) => reply.error(err.log_and_convert()),
        }
    }

    fn rename(
        &mut self,
        _req: &Request<'_>,
        parent: u64,
        name: &OsStr,
        newparent: u64,
        newname: &OsStr,
        _: u32,
        reply: fuser::ReplyEmpty,
    ) {
        match self
            .fs
            .rename(InodeId::new(parent), name, InodeId::new(newparent), newname)
        {
            Ok(_) => reply.ok(),
            Err(err) => reply.error(err.log_and_convert()),
        }
    }

    fn read(
        &mut self,
        _: &Request,
        ino: u64,
        _: u64,
        offset: i64,
        size: u32,
        _: i32,
        _: Option<u64>,
        reply: ReplyData,
    ) {
        match self.fs.read(InodeId::new(ino), offset, size) {
            Ok(val) => reply.data(&val),
            Err(err) => reply.error(err.log_and_convert()),
        }
    }

    fn write(
        &mut self,
        _: &Request<'_>,
        ino: u64,
        _: u64,
        offset: i64,
        data: &[u8],
        _: u32,
        _: i32,
        _: Option<u64>,
        reply: fuser::ReplyWrite,
    ) {
        match self.fs.write(InodeId::new(ino), offset, data) {
            Ok(_) => reply.written(data.len() as u32),
            Err(err) => reply.error(err.log_and_convert()),
        }
    }

    fn readdir(&mut self, _: &Request, ino: u64, _: u64, offset: i64, mut reply: ReplyDirectory) {
        match self.fs.readdir(InodeId::new(ino), offset) {
            Ok(attrs) => {
                for (attr_offset, attr_iid, attr_kind, attr_name) in attrs {
                    if reply.add(attr_iid.get(), attr_offset, attr_kind, attr_name) {
                        break;
                    }
                }

                reply.ok();
            }

            Err(err) => reply.error(err.log_and_convert()),
        }
    }
}
