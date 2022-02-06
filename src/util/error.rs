use std::io;

pub type AppResult = io::Result<()>;

pub trait ErrorCode {
    fn code(&self) -> i32;
}

impl ErrorCode for io::Error {
    fn code(&self) -> i32 { 1 }
}