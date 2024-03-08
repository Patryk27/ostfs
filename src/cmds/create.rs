use crate::{EntryObj, HeaderObj, Object, ObjectId, Objects, Storage};
use anyhow::{Context, Result};
use fuser::FileType;
use std::fs;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct CreateCmd {
    /// Path to the *.ofs file
    src: PathBuf,

    /// When set, tries to remove `src` first
    #[structopt(short, long)]
    recreate: bool,
}

impl CreateCmd {
    pub fn run(self) -> Result<()> {
        if self.recreate && self.src.exists() {
            fs::remove_file(&self.src)
                .with_context(|| format!("couldn't delete: {}", self.src.display()))?;
        }

        let preset = {
            let uid = unsafe { libc::getuid() };
            let gid = unsafe { libc::getgid() };

            [
                Object::Header(HeaderObj {
                    root: ObjectId::new(1),
                    dead: None,
                    clone: None,
                }),
                Object::Entry(EntryObj {
                    name: ObjectId::new(2),
                    body: None,
                    next: None,
                    kind: FileType::Directory,
                    size: 0,
                    mode: 0o777,
                    uid,
                    gid,
                }),
                Object::payload(b"/"),
            ]
        };

        let storage = Storage::create(&self.src)?;
        let mut objects = Objects::new(storage);

        for object in preset {
            objects.alloc(None, object)?;
        }

        println!("ok");

        Ok(())
    }
}
