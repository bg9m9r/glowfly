//! Fetch full history's size check, and downloading a partial clone's missing contents in the
//! background (`Effect::Partial`).

use super::git_ops;
use crate::effect::partial::{ContentRequest, HistorySizeRequest, PartialEffect};
use crate::msg::{ContentFailure, Msg, PartialMsg};
use trellis_core::forge;
use trellis_core::gitcli::Stop;
use trellis_core::history_size::{HistorySize, SizeSource, local_store_bytes};

pub(super) fn run(e: PartialEffect, stop: Option<&Stop>) -> Msg {
    match e {
        PartialEffect::SizeHistory(r) => size(r),
        PartialEffect::FetchContent(r) => fetch(r, stop),
    }
}

/// The hosting service's reported size, else the size on disk of a source on this computer.
fn size(r: HistorySizeRequest) -> Msg {
    let HistorySizeRequest { workdir, id, url, service } = r;
    let reported = service.and_then(|s| {
        let account = s.target.account();
        let auth = match s.auth {
            Some(a) => Ok(a),
            None => forge::resolve_auth(&account, &workdir, false, s.connected),
        };
        match auth.and_then(|a| forge::repo_size(&s.target, &a)) {
            Ok(bytes) => bytes.map(|bytes| HistorySize::Known { bytes, source: SizeSource::Service(s.target.provider.name().to_owned()) }),
            Err(e) => {
                tracing::debug!("{} didn't report the repository's size: {e}", s.target.provider.name());
                None
            }
        }
    });
    let size = reported
        .or_else(|| {
            let url = url?;
            local_store_bytes(&workdir, &url).map(|bytes| HistorySize::Known { bytes, source: SizeSource::Folder(url) })
        })
        .unwrap_or(HistorySize::Unknown);
    Msg::Partial(PartialMsg::HistorySized { workdir, id, size })
}

/// Runs the download without prompts, stopped when `stop` is asked.
fn fetch(r: ContentRequest, stop: Option<&Stop>) -> Msg {
    let stop = stop.cloned().unwrap_or_default();
    let (ok, output, stopped) = git_ops::run_git_until(&r.workdir, &r.cmds, false, &stop);
    let result = match (ok, stopped) {
        (_, true) => Err(ContentFailure::Stopped),
        (true, false) => Ok(()),
        (false, false) => Err(ContentFailure::Failed(trellis_core::logs::last_message(&output))),
    };
    Msg::Partial(PartialMsg::ContentFetched { id: r.id, result })
}
