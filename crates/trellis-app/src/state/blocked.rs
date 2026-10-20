//! The reason shared by every action that can't start while another one runs.

/// Why an action can't start while another one runs.
pub const BUSY: &str = "Another operation is still running.";
