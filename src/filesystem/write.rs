use super::{FsError, FsResult};
use crate::filesystem::{Alter, AlterResult};
use crate::{Filesystem, InodeId, Object};
use tracing::{debug, instrument};

impl Filesystem {
    #[instrument(skip(self, incoming))]
    pub fn write(&mut self, iid: InodeId, offset: i64, incoming: &[u8]) -> FsResult<()> {
        debug!("op: write()");

        if !self.is_writable {
            return Err(FsError::ReadOnly);
        }

        self.begin_tx()?;

        let AlterResult {
            new_root_oid,
            new_oid,
            changeset,
        } = self.alter(Alter::clone(iid))?;

        let new_oid = new_oid.unwrap();
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
        self.tx.update_root(new_root_oid, changeset)?;
        self.commit_tx()?;

        Ok(())
    }
}
