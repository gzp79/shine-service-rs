use std::error::Error as StdError;

/// Attempt to downcast `err` into a `T` and if that fails recursively try and downcast `err`'s source
pub fn find_error_source<'a, T>(err: &'a (dyn StdError + 'static)) -> Option<&'a T>
where
    T: StdError + 'static,
{
    if let Some(err) = err.downcast_ref::<T>() {
        Some(err)
    } else if let Some(source) = err.source() {
        find_error_source(source)
    } else {
        None
    }
}
