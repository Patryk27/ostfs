use super::FsResult;
use crate::{Filesystem, InodeId};
use fuser::FileType;
use std::ffi::OsString;
use tracing::{debug, instrument};

impl Filesystem {
    #[instrument(skip(self))]
    pub fn readdir(
        &mut self,
        iid: InodeId,
        mut offset: i64,
    ) -> FsResult<Vec<(i64, InodeId, FileType, OsString)>> {
        debug!("op: readdir()");

        let mut nth = 0;
        let mut entries = Vec::new();

        // ---

        nth += 1;

        if offset == 0 {
            entries.push((nth, iid, FileType::Directory, ".".into()));
        } else {
            offset -= 1;
        }

        // ---

        if let Ok(parent_iid) = self.inodes.resolve_parent(iid) {
            nth += 1;

            if offset == 0 {
                entries.push((nth, parent_iid, FileType::Directory, "..".into()));
            } else {
                offset -= 1;
            }
        }

        // ---

        nth += offset;

        let children = self
            .inodes
            .resolve_children(&mut self.objects, iid)?
            .into_iter()
            .skip(offset as usize);

        for iid in children {
            let oid = self.inodes.resolve_object(iid)?;
            let obj = self.objects.get(oid)?.into_entry(oid)?;
            let kind = obj.kind;
            let name = self.objects.get_os_string(obj.name)?;

            nth += 1;

            entries.push((nth, iid, kind, name));
        }

        Ok(entries)
    }
}
