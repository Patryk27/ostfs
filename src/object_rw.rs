use fuser::FileType;

use crate::{Object, ObjectId};

#[derive(Debug, Default)]
pub struct ObjectWriter {
    data: [u8; Object::SIZE],
    len: usize,
}

impl ObjectWriter {
    pub fn bool(&mut self, val: bool) {
        self.u8(val as u8);
    }

    pub fn u8(&mut self, val: u8) {
        self.data[self.len] = val;
        self.len += 1;
    }

    pub fn u16(&mut self, val: u16) {
        for b in val.to_be_bytes() {
            self.u8(b);
        }
    }

    pub fn u32(&mut self, val: u32) {
        for b in val.to_be_bytes() {
            self.u8(b);
        }
    }

    pub fn oid(&mut self, oid: ObjectId) {
        self.u32(oid.get());
    }

    pub fn oid_opt(&mut self, oid: Option<ObjectId>) {
        self.u32(oid.map(|oid| oid.get()).unwrap_or_default());
    }

    pub fn kind(&mut self, kind: FileType) {
        self.u8(if kind == FileType::Directory { 0 } else { 1 })
    }

    pub fn rest(&mut self, mut next_byte: impl FnMut() -> Option<u8>) {
        while let Some(byte) = next_byte() {
            self.u8(byte);
        }
    }

    pub fn finish(self) -> [u8; Object::SIZE] {
        self.data
    }
}

#[derive(Debug)]
pub struct ObjectReader {
    data: [u8; Object::SIZE],
    len: usize,
}

impl ObjectReader {
    pub fn new(data: [u8; Object::SIZE]) -> Self {
        Self { data, len: 0 }
    }

    pub fn bool(&mut self) -> bool {
        self.u8() == 1
    }

    pub fn u8(&mut self) -> u8 {
        let out = self.data[self.len];

        self.len += 1;

        out
    }

    pub fn u16(&mut self) -> u16 {
        let d0 = self.u8();
        let d1 = self.u8();

        u16::from_be_bytes([d0, d1])
    }

    pub fn u32(&mut self) -> u32 {
        let d0 = self.u8();
        let d1 = self.u8();
        let d2 = self.u8();
        let d3 = self.u8();

        u32::from_be_bytes([d0, d1, d2, d3])
    }

    pub fn oid(&mut self) -> ObjectId {
        ObjectId::new(self.u32())
    }

    pub fn oid_opt(&mut self) -> Option<ObjectId> {
        let oid = self.u32();

        if oid == 0 {
            None
        } else {
            Some(ObjectId::new(oid))
        }
    }

    pub fn kind(&mut self) -> FileType {
        if self.u8() == 0 {
            FileType::Directory
        } else {
            FileType::RegularFile
        }
    }

    pub fn rest<const N: usize>(&mut self) -> [u8; N] {
        self.data[self.len..]
            .try_into()
            .expect("rest of the block has unexpected size")
    }
}
