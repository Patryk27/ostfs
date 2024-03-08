use anyhow::Error;
use libc::{EIO, ENOENT, ENOSYS, EROFS};
use std::ffi::c_int;
use tracing::{debug, error};

pub type FsResult<T> = Result<T, FsError>;

#[derive(Debug)]
pub enum FsError {
    ReadOnly,
    NotFound,
    NotImplemented,
    Other(Error),
}

impl FsError {
    pub fn log_and_convert(self) -> c_int {
        match self {
            FsError::ReadOnly => {
                debug!("... read-only file system");
                EROFS
            }

            FsError::NotFound => {
                debug!("... not found");
                ENOENT
            }

            FsError::NotImplemented => {
                debug!("... not implemented");
                ENOSYS
            }

            FsError::Other(err) => {
                error!("... {:?}", err);
                EIO
            }
        }
    }
}

impl From<Error> for FsError {
    fn from(err: Error) -> Self {
        FsError::Other(err)
    }
}
