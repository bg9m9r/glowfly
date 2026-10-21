//! The job of Effect::OpPreview: what an operation would do, worked out without changing the
//! repository (runner/op_preview.rs, trellis_core::op_preview).

use std::path::PathBuf;

/// One preview of `request` in the repository at `workdir`.
#[derive(Clone, Debug, PartialEq)]
pub struct OpPreviewJob {
    pub workdir: PathBuf,
    pub request: trellis_core::op_preview::Request,
}

impl OpPreviewJob {
    /// The effect's name, for the worker thread and the debug log.
    pub fn name() -> &'static str {
        "op-preview"
    }
}
