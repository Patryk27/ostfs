use crate::{ObjectId, Objects, Storage};
use anyhow::Result;
use std::path::PathBuf;
use structopt::StructOpt;

#[derive(Debug, StructOpt)]
pub struct InspectCmd {
    /// Path to the *.ofs file
    src: PathBuf,

    /// When set, shows just this particular object
    #[structopt(short, long)]
    oid: Option<u32>,
}

impl InspectCmd {
    pub fn run(self) -> Result<()> {
        let storage = Storage::open(&self.src, false)?;
        let mut objects = Objects::new(storage);

        if let Some(oid) = self.oid {
            println!("[{}] = {:?}", oid, objects.get(ObjectId::new(oid)));
        } else {
            for (oid, obj) in objects.all()? {
                println!("[{}] = {:?}", oid.get(), obj);
            }
        }

        Ok(())
    }
}
