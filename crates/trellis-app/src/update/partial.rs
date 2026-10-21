//! Fetch full history's size check (update/partial_size.rs) and a partial clone's missing file
//! contents (update/partial_content.rs).

use crate::effect::Effect;
use crate::msg::PartialMsg;
use crate::state::AppState;

pub(super) fn handle(st: &mut AppState, msg: PartialMsg, fx: &mut Vec<Effect>) {
    match msg {
        PartialMsg::HistorySized { workdir, id, size } => super::partial_size::sized(st, &workdir, id, size, fx),
        PartialMsg::ContentMissing { missing, retry } => super::partial_content::missing(st, missing, retry, fx),
        PartialMsg::DownloadContent => super::partial_content::download(st, fx),
        PartialMsg::StopContent => super::partial_content::stop(st, fx),
        PartialMsg::ContentFetched { id, result } => super::partial_content::fetched(st, id, result, fx),
    }
}
