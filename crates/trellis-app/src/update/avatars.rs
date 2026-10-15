//! Author avatars: requests from the views become background fetches, and fetched images wait
//! in `view.avatars_loaded` for the shell to turn them into textures.

use crate::effect::Effect;
use crate::msg::AvatarMsg;
use crate::state::AppState;

/// Emails per fetch. Each batch runs one request at a time, so a screen of new authors uses a
/// few connections rather than one per author, and the first faces still appear quickly.
const BATCH: usize = 8;

pub(super) fn handle(st: &mut AppState, msg: AvatarMsg, fx: &mut Vec<Effect>) {
    match msg {
        AvatarMsg::WantAvatars(emails) => fx.extend(emails.chunks(BATCH).map(|c| Effect::FetchAvatars(c.to_vec()))),
        AvatarMsg::AvatarsLoaded(batch) => st.view.avatars_loaded.extend(batch),
    }
}
