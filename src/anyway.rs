use std::error::Error;

pub type AnyError = Box<dyn Error>;

pub type AnyResult<T> = Result<T, AnyError>;

#[macro_export]
macro_rules! ok {
    () => {
        Ok::<(), $crate::anyway::AnyError>(())
    };
}
