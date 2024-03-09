use crate::{Objects, Storage};
use anyhow::Result;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct InspectCmd {
    /// Path to the *.ofs file
    src: PathBuf,
}

impl InspectCmd {
    pub fn run(self) -> Result<()> {
        let storage = Storage::open(&self.src, false)?;
        let mut objects = Objects::new(storage);

        for (oid, obj) in objects.all()? {
            println!("[{}] = {:?}", oid.get(), obj);
        }

        Ok(())
    }
}
