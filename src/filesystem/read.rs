use super::{FsError, FsResult};
use crate::{Filesystem, InodeId};
use tracing::{debug, instrument};

impl Filesystem {
    #[instrument(skip(self))]
    pub fn read(&mut self, iid: InodeId, offset: i64, size: u32) -> FsResult<Vec<u8>> {
        debug!("op: read()");

        let oid = self
            .inodes
            .resolve_object(iid)
            .ok()
            .ok_or(FsError::NotFound)?;

        let obj = self.objects.get(oid)?.into_entry(oid)?;

        if let Some(body) = obj.body {
            // TODO excruciatingly inefficient & wasteful
            let data = self.objects.get_payload(body)?;
            let data = data[offset as usize..][..size as usize].to_vec();

            Ok(data)
        } else {
            Ok(Default::default())
        }
    }
}
