//! The minisign signature over a release's `SHA256SUMS` (`SHA256SUMS.minisig`), made by the
//! Release workflow with the project's key. No checksum in `SHA256SUMS` is trusted until the
//! signature verifies against one of the public keys pinned here, so a download matches what
//! the project signed, not only what the release page lists (docs/RELEASING.md, Signing).

use super::{Asset, Release, UpdateError};
use minisign_verify::{PublicKey, Signature};

/// The signature's file name among the release's files.
pub(super) const SIGNATURE_FILE: &str = "SHA256SUMS.minisig";

/// The project's minisign public keys (base64, the second line of a `.pub` file). A signature
/// by any of them is accepted, so a key change ships the next key here one release before the
/// release secret switches to it, and drops the old one a release later.
pub(super) const KEYS: &[&str] = &[
    // 43C59A9629C2A686, since 2026-11-05.
    "RWSGpsIplprFQ/toKF1uFX1qZOySLsmrcaXgRecNj/C5j+CYKPZYMB1K",
];

/// The test key (C61175B0AC05E8D3). Its secret half is committed on purpose
/// (`minisign_fixtures/test-only.key`), so the tests and the installer checks can sign local
/// feeds; it is trusted only for a feed folder (GLOWFLY_UPDATE_FEED), never for the hosting
/// service's releases.
pub(super) const TEST_KEY: &str = "RWTT6AWssHURxjR/C+5C0lcSHiG4EvwQ/guMhtYrkrb3zLIErX70zLDk";

/// The largest signature file read (minisign's are about 300 bytes).
pub(super) const MAX_SIGNATURE: u64 = 4 * 1024;

/// The release's signature file, when it publishes one.
pub(super) fn signature_asset(release: &Release) -> Option<Asset> {
    release.assets.iter().find(|a| a.name == SIGNATURE_FILE).cloned()
}

/// Whether `signature` (a `.minisig` file's text) is a valid signature of `sums` by one of
/// `keys`. Only pre-hashed signatures count (what minisign makes by default); the legacy kind is
/// refused. A key that doesn't parse is skipped.
pub(super) fn verify(sums: &[u8], signature: &[u8], keys: &[&str]) -> Result<(), UpdateError> {
    let text = std::str::from_utf8(signature).map_err(|_| UpdateError::BadSignature)?;
    let signature = Signature::decode(text).map_err(|_| UpdateError::BadSignature)?;
    let signed = keys.iter().filter_map(|k| PublicKey::from_base64(k).ok()).any(|key| key.verify(sums, &signature, false).is_ok());
    if signed { Ok(()) } else { Err(UpdateError::BadSignature) }
}
