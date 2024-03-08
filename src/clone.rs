use crate::{CloneObj, HeaderObj, Object, ObjectId, Objects};
use anyhow::{anyhow, Context, Result};

#[derive(Clone, Debug)]
pub struct Clone {
    pub oid: ObjectId,
    pub name: String,
    pub root: ObjectId,
    pub is_writable: bool,
}

#[derive(Debug)]
pub struct CloneController<'a> {
    objects: &'a mut Objects,
}

impl<'a> CloneController<'a> {
    pub fn new(objects: &'a mut Objects) -> Self {
        Self { objects }
    }

    pub fn create(&mut self, name: &str, is_writable: bool) -> Result<()> {
        let name = name.trim();

        for clone in self.all()? {
            if clone.name == name {
                return Err(anyhow!("clone named `{}` already exists", name));
            }
        }

        let header = self.objects.get_header()?;

        let name = self
            .objects
            .alloc_payload(None, name.as_bytes())?
            .context("name cannot be empty")?;

        let clone = self.objects.alloc(
            None,
            Object::Clone(CloneObj {
                name,
                root: header.root,
                is_writable,
                next: header.clone,
            }),
        )?;

        self.objects.set_header(HeaderObj {
            clone: Some(clone),
            ..header
        })?;

        Ok(())
    }

    pub fn delete(&mut self, name: &str) -> Result<()> {
        let header = self.objects.get_header()?;
        let oid_to_delete = self.find(name)?.oid;

        let mut clones = Vec::new();
        let mut cursor = header.clone;

        while let Some(oid) = cursor {
            let obj = self.objects.get(oid)?.into_clone(oid)?;

            if oid != oid_to_delete {
                let new_oid = self.objects.alloc(None, Object::Clone(obj))?;

                clones.push((obj, new_oid));
            }

            cursor = obj.next;
        }

        for i in 0..clones.len() {
            let next = clones.get(i + 1).map(|(_, new_oid)| *new_oid);
            let (curr, oid) = &mut clones[i];

            curr.next = next;

            self.objects.set(*oid, Object::Clone(*curr))?;
        }

        self.objects.set_header(HeaderObj {
            clone: clones.first().map(|(_, oid)| *oid),
            ..header
        })?;

        Ok(())
    }

    pub fn find(&mut self, name: &str) -> Result<Clone> {
        let name = name.trim();

        self.all()?
            .into_iter()
            .find(|clone| clone.name == name)
            .ok_or_else(|| anyhow!("no clone named `{}` found", name))
    }

    pub fn all(&mut self) -> Result<Vec<Clone>> {
        let mut clones = Vec::new();
        let mut cursor = self.objects.get_header()?.clone;

        while let Some(oid) = cursor {
            let obj = self.objects.get(oid)?.into_clone(oid)?;

            clones.push(Clone {
                oid,
                name: self.objects.get_string(obj.name)?,
                root: obj.root,
                is_writable: obj.is_writable,
            });

            cursor = obj.next;
        }

        clones.reverse();

        Ok(clones)
    }
}
