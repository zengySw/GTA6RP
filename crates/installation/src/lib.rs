//! Safe application of files into a managed installation root.
//!
//! All writes are confined to a canonicalised root directory. Paths are
//! validated by [`gta6rp_domain::path::RelativePath`] before they reach the
//! filesystem, symlinked ancestors are refused, and completed files are placed
//! with a temporary file plus rename so a partial write is never left in place
//! under the final name.

mod safe_root;

pub use safe_root::{FreeSpaceInfo, InstallError, SafeRoot, ensure_free_space, free_space_info};
