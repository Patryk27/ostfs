use crate::{ObjectId, ObjectReader, ObjectWriter};
use anyhow::{anyhow, Result};
use fuser::FileType;
use std::fmt::{self, Debug};

#[derive(Clone, Copy, Debug)]
pub enum Object {
    Empty,
    Header(HeaderObj),
    Clone(CloneObj),
    Entry(EntryObj),
    Payload(PayloadObj),
    Dead(DeadObj),
}

impl Object {
    pub const SIZE: usize = 32;

    pub const TY_EMPTY: u8 = 0;
    pub const TY_HEADER: u8 = 1;
    pub const TY_CLONE: u8 = 2;
    pub const TY_ENTRY: u8 = 3;
    pub const TY_FILE: u8 = 4;
    pub const TY_PAYLOAD: u8 = 5;
    pub const TY_DEAD: u8 = 6;

    pub const PAYLOAD_LEN: usize = 26;

    pub fn decode(obj: [u8; Self::SIZE]) -> Result<Self> {
        let mut reader = ObjectReader::new(obj);

        match reader.u8() {
            Self::TY_EMPTY => Ok(Object::Empty),

            Self::TY_HEADER => Ok(Object::Header(HeaderObj {
                root: reader.oid(),
                dead: reader.oid_opt(),
                clone: reader.oid_opt(),
            })),

            Self::TY_CLONE => Ok(Object::Clone(CloneObj {
                name: reader.oid(),
                root: reader.oid(),
                is_writable: reader.bool(),
                next: reader.oid_opt(),
            })),

            Self::TY_ENTRY => Ok(Object::Entry(EntryObj {
                name: reader.oid(),
                body: reader.oid_opt(),
                next: reader.oid_opt(),
                kind: reader.kind(),
                size: reader.u32(),
                mode: reader.u16(),
                uid: reader.u32(),
                gid: reader.u32(),
            })),

            Self::TY_PAYLOAD => Ok(Object::Payload(PayloadObj {
                size: reader.u8(),
                next: reader.oid_opt(),
                data: reader.rest(),
            })),

            Self::TY_DEAD => Ok(Object::Dead(DeadObj {
                next: reader.oid_opt(),
            })),

            ty => Err(anyhow!("unknown object type: {}", ty)),
        }
    }

    pub fn encode(self) -> [u8; Self::SIZE] {
        let mut writer = ObjectWriter::default();

        match self {
            Object::Empty => {
                //
            }

            Object::Header(HeaderObj { root, dead, clone }) => {
                writer.u8(Self::TY_HEADER);
                writer.oid(root);
                writer.oid_opt(dead);
                writer.oid_opt(clone);
            }

            Object::Clone(CloneObj {
                root,
                name,
                is_writable: is_snapshot,
                next,
            }) => {
                writer.u8(Self::TY_CLONE);
                writer.oid(name);
                writer.oid(root);
                writer.bool(is_snapshot);
                writer.oid_opt(next);
            }

            Object::Entry(EntryObj {
                name,
                body,
                next,
                kind,
                size,
                mode,
                uid,
                gid,
            }) => {
                writer.u8(Self::TY_ENTRY);
                writer.oid(name);
                writer.oid_opt(body);
                writer.oid_opt(next);
                writer.kind(kind);
                writer.u32(size);
                writer.u16(mode);
                writer.u32(uid);
                writer.u32(gid);
            }

            Object::Payload(PayloadObj { size, next, data }) => {
                let mut data = data.iter().copied();

                writer.u8(Self::TY_PAYLOAD);
                writer.u8(size);
                writer.oid_opt(next);
                writer.rest(|| data.next());
            }

            Object::Dead(DeadObj { next }) => {
                writer.u8(Self::TY_DEAD);
                writer.oid_opt(next);
            }
        }

        writer.finish()
    }

    pub fn into_header(self, oid: ObjectId) -> Result<HeaderObj> {
        if let Object::Header(obj) = self {
            Ok(obj)
        } else {
            Err(anyhow!("expected header object at {:?}", oid))
        }
    }

    pub fn into_clone(self, oid: ObjectId) -> Result<CloneObj> {
        if let Object::Clone(obj) = self {
            Ok(obj)
        } else {
            Err(anyhow!("expected clone object at {:?}", oid))
        }
    }

    pub fn into_entry(self, oid: ObjectId) -> Result<EntryObj> {
        if let Object::Entry(obj) = self {
            Ok(obj)
        } else {
            Err(anyhow!("expected entry object at {:?}", oid))
        }
    }

    pub fn into_payload(self, oid: ObjectId) -> Result<PayloadObj> {
        if let Object::Payload(obj) = self {
            Ok(obj)
        } else {
            Err(anyhow!("expected payload object at {:?}", oid))
        }
    }

    pub fn into_dead(self, oid: ObjectId) -> Result<DeadObj> {
        if let Object::Dead(obj) = self {
            Ok(obj)
        } else {
            Err(anyhow!("expected dead object at {:?}", oid))
        }
    }

    pub fn payload(data: &[u8]) -> Self {
        Object::Payload(PayloadObj::new(data))
    }
}

#[derive(Clone, Copy, Debug)]
pub struct HeaderObj {
    /// Points at [`EntryObj`] containing the root directory
    pub root: ObjectId,

    /// Points at first [`CloneObj`]
    pub clone: Option<ObjectId>,

    /// Points at first [`DeadObj`]
    pub dead: Option<ObjectId>,
}

#[derive(Clone, Copy, Debug)]
pub struct CloneObj {
    /// Points at [`PayloadObj`] containing the clone's name
    pub name: ObjectId,

    /// Points at [`EntryObj`] containing the clone's root directory
    pub root: ObjectId,

    /// Whether it's a "clone-clone" or rather a snapshot
    pub is_writable: bool,

    /// Points at the sibling [`CloneObj`] (it's a linked list)
    pub next: Option<ObjectId>,
}

/// File or directory
#[derive(Clone, Copy, Debug)]
pub struct EntryObj {
    /// Points at [`PayloadObj`] containing the entry's name
    pub name: ObjectId,

    /// Points either at enother [`EntryObj`] (if this is a directory) or at
    /// [`PayloadObj`] (if this is a file)
    pub body: Option<ObjectId>,

    /// Points at the sibling [`EntryObj`] (intuitively, it points at the next
    /// item in the directory which this entry is contained within)
    pub next: Option<ObjectId>,

    pub kind: FileType,
    pub size: u32,
    pub mode: u16,
    pub uid: u32,
    pub gid: u32,
}

/// String or binary data
#[derive(Clone, Copy)]
pub struct PayloadObj {
    pub size: u8,
    pub next: Option<ObjectId>,
    pub data: [u8; Object::PAYLOAD_LEN],
}

impl PayloadObj {
    pub fn new(data: &[u8]) -> Self {
        assert!(data.len() <= Object::PAYLOAD_LEN);

        let mut buf = [0; Object::PAYLOAD_LEN];

        buf[0..data.len()].copy_from_slice(data);

        Self {
            size: data.len() as u8,
            next: None,
            data: buf,
        }
    }
}

impl Debug for PayloadObj {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let data = &self.data[0..(self.size as usize)];

        let data = if let Ok(data) = String::from_utf8(data.to_vec()) {
            data
        } else {
            data.iter()
                .map(|b| format!("{:#02x}", b))
                .collect::<Vec<_>>()
                .join(" ")
        };

        f.debug_struct("Payload")
            .field("size", &self.size)
            .field("next", &self.next)
            .field("data", &data)
            .finish()
    }
}

/// Object that's been garbage collected; used by the object allocator to reuse
/// space instead of always allocating a new object
#[derive(Clone, Copy, Debug)]
pub struct DeadObj {
    pub next: Option<ObjectId>,
}
