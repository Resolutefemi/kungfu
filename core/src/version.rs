//! Framework version constant.

pub const VERSION: &str = "0.1.0";
pub const NAME: &str = "kungfu";

/// Returns `kungfu/0.1.0` for use in `Server` and `X-Powered-By` headers.
pub fn banner() -> String {
    format!("{NAME}/{VERSION}")
}
