use super::{FsError, FsResult};
use crate::{Filesystem, InodeId, Object};
use tracing::{debug, instrument};

impl Filesystem {
    #[instrument(skip(self, incoming))]
    pub fn write(&mut self, iid: InodeId, offset: i64, incoming: &[u8]) -> FsResult<()> {
        debug!("op: write()");

        if !self.origin.is_writable() {
            return Err(FsError::ReadOnly);
        }

        self.begin_tx()?;

        let new_oid = self.clone_inode(iid)?;
        let mut obj = self.objects.get(new_oid)?.into_entry(new_oid)?;

        obj.body = {
            // TODO excruciatingly inefficient & wasteful
            let mut data = if let Some(body) = obj.body {
                self.objects.get_payload(body)?
            } else {
                Default::default()
            };

            if (offset as usize) + incoming.len() > (obj.size as usize) {
                obj.size = (offset as u32) + (incoming.len() as u32);
            }

            data.resize(obj.size as usize, 0);
            data[offset as usize..][..incoming.len()].copy_from_slice(incoming);

            self.objects.alloc_payload(Some(&mut self.tx), &data)?
        };

        self.objects.set(new_oid, Object::Entry(obj))?;
        self.commit_tx()?;

        Ok(())
    }
}
