use super::FsResult;
use crate::{Filesystem, InodeId};
use fuser::FileAttr;
use std::ffi::OsStr;
use tracing::{debug, instrument};

impl Filesystem {
    #[instrument(skip(self))]
    pub fn lookup(&mut self, parent_iid: InodeId, name: &OsStr) -> FsResult<FileAttr> {
        debug!("op: lookup()");

        self.begin_tx()?;
        let (iid, obj) = self.find(parent_iid, name)?;
        self.commit_tx()?;

        Ok(Self::attr(iid, obj))
    }
}
