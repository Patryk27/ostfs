use crate::{HeaderObj, Object, ObjectId, PayloadObj, Storage, Transaction};
use anyhow::{anyhow, Context, Result};
use std::ffi::OsString;
use tracing::{instrument, trace, warn};

#[derive(Debug)]
pub struct Objects {
    storage: Storage,
}

impl Objects {
    pub fn new(storage: Storage) -> Self {
        Self { storage }
    }

    #[instrument(skip(self))]
    pub fn all(&mut self) -> Result<Vec<(ObjectId, Object)>> {
        let mut objs = Vec::new();
        let mut oidx = 0;

        while let Ok(object) = self.storage.read(oidx) {
            let oid = ObjectId::new(oidx);

            let obj =
                Object::decode(object).with_context(|| format!("couldn't decode {:?}", oid))?;

            objs.push((oid, obj));
            oidx += 1;
        }

        Ok(objs)
    }

    #[instrument(skip(self))]
    pub fn get(&mut self, oid: ObjectId) -> Result<Object> {
        trace!("reading object");

        if oid == ObjectId::HEADER {
            return Err(anyhow!("tried to get() the header object"));
        }

        let obj = self
            .storage
            .read(oid.get())
            .with_context(|| format!("couldn't read object {:?}", oid))?;

        Object::decode(obj).with_context(|| format!("couldn't decode {:?}", oid))
    }

    #[instrument(skip(self))]
    pub fn get_header(&mut self) -> Result<HeaderObj> {
        let obj = self
            .storage
            .read(ObjectId::HEADER.get())
            .context("couldn't read header")?;

        Object::decode(obj)
            .context("couldn't decode header")?
            .into_header(ObjectId::HEADER)
    }

    pub fn get_payload(&mut self, mut oid: ObjectId) -> Result<Vec<u8>> {
        let mut buf = Vec::new();

        loop {
            let obj = self.get(oid)?.into_payload(oid)?;

            buf.extend_from_slice(&obj.data[0..(obj.size as usize)]);

            if let Some(next) = obj.next {
                oid = next;
            } else {
                break;
            }
        }

        Ok(buf)
    }

    pub fn get_string(&mut self, oid: ObjectId) -> Result<String> {
        let buf = self.get_payload(oid)?;

        Ok(String::from_utf8_lossy(&buf).into())
    }

    pub fn get_os_string(&mut self, oid: ObjectId) -> Result<OsString> {
        Ok(self.get_string(oid)?.into())
    }

    #[instrument(skip(self, obj))]
    pub fn set(&mut self, oid: ObjectId, obj: Object) -> Result<()> {
        trace!("writing object [{:?}] = {:?}", oid, obj);

        if oid == ObjectId::HEADER {
            return Err(anyhow!("tried to set() the header object"));
        }

        self.storage
            .write(oid.get(), obj.encode())
            .with_context(|| format!("couldn't write [{:?}] = {:?}", oid, obj))?;

        Ok(())
    }

    #[instrument(skip(self))]
    pub fn set_header(&mut self, obj: HeaderObj) -> Result<()> {
        self.storage
            .write(ObjectId::HEADER.get(), Object::Header(obj).encode())
            .context("couldn't write header")?;

        Ok(())
    }

    #[instrument(skip(self, tx, obj))]
    pub fn alloc(&mut self, tx: Option<&mut Transaction>, obj: Object) -> Result<ObjectId> {
        if let Some(tx) = tx {
            let tx = tx.get_mut()?;

            if let Some(oid) = tx.header.dead {
                let dead_obj = self.get(oid)?;

                if let Ok(dead_obj) = dead_obj.into_dead(oid) {
                    trace!("reusing {:?} = {:?}", oid, obj);

                    self.storage
                        .write(oid.get(), obj.encode())
                        .with_context(|| format!("couldn't write [{:?}] = {:?}", oid, obj))?;

                    tx.dirty = true;
                    tx.header.dead = dead_obj.next;

                    return Ok(oid);
                } else {
                    warn!("can't reuse {:?} - GC required", oid);
                }
            }
        }

        let oid = self.storage.append(obj.encode())?;
        let oid = ObjectId::new(oid);

        trace!("creating {:?} = {:?}", oid, obj);

        Ok(oid)
    }

    pub fn alloc_payload(
        &mut self,
        mut tx: Option<&mut Transaction>,
        payload: &[u8],
    ) -> Result<Option<ObjectId>> {
        let mut next = None;

        for chunk in payload.chunks(Object::PAYLOAD_LEN).rev() {
            let curr = self.alloc(
                tx.as_deref_mut(),
                Object::Payload(PayloadObj {
                    next,
                    ..PayloadObj::new(chunk)
                }),
            )?;

            next = Some(curr);
        }

        Ok(next)
    }
}
