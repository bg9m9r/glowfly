//! The checks on the built executable, one test binary: git runs Glowfly as its sequence editor
//! (`sequence_editor`), askpass program (`askpass`) and gpg.program (`gpg_prompt`), each with
//! the real `glowfly` from `CARGO_BIN_EXE_glowfly`.

mod askpass;
mod gpg_prompt;
mod sequence_editor;
