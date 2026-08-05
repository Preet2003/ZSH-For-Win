//! PowerShell 7 profile markers, module path registration, and host integration.

#![forbid(unsafe_code)]

/// Marker comments owned exclusively by the installer / powershell crate.
pub const PROFILE_BEGIN: &str = "# >>> winzsh >>>";
/// End marker for the managed profile block.
pub const PROFILE_END: &str = "# <<< winzsh <<<";
