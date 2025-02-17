pub use super::{BuildConfig, Index};

// functions in these modules are not supposed to call `Index::save_to_file`
mod add;
mod archive;
mod build;
mod check;
mod clone;
mod config;
mod gc;
mod ls;
mod merge;
mod meta;
mod migrate;
mod push;
mod recover;
mod remove;
mod reset;

pub use add::{AddMode, AddResult};
pub use merge::{MergeMode, MergeResult};
pub use migrate::{VersionInfo, get_compatibility_warning};
pub use recover::RecoverResult;
