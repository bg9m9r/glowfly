//! The end-to-end tests of `update`: they run effects through the runner (glowfly-runner's
//! `testing`), and the tests that share helpers with them. A crate of their own so the model's
//! tests never need the runner, and so they build and run without the app's window, views and
//! wgpu. Test code only: without `cfg(test)` this crate is empty.

#[cfg(test)]
mod flows;
