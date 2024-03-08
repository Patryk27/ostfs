#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ObjectId(u32);

impl ObjectId {
    pub const HEADER: Self = Self(0);

    pub fn new(oid: u32) -> Self {
        Self(oid)
    }

    pub fn get(self) -> u32 {
        self.0
    }
}
