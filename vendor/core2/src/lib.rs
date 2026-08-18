//! Vendored stub for core2 v0.4.0 (yanked on crates.io).
//! Provides the API surface used by glass_pumpkin 1.7/1.9.

pub mod error {
    /// The core2 Error trait — in std environments this is the same as `std::error::Error`.
    pub use std::error::Error;
}

pub mod io {
    pub use std::io::{Cursor, Error, ErrorKind, Read, Result, Write};
}
