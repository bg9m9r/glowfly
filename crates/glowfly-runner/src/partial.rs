//! Fetch full history's size check, and downloading a partial clone's missing contents in the
//! background (`Effect::Partial`).

use super::execute::{Cx, Execute};
use super::sign_ins::{Ask, sign_in};
use glowfly_core::forge;
use glowfly_core::gitcli::Stop;
use glowfly_core::history_size::{HistorySize, SizeSource, local_store_bytes};
use glowfly_model::effect::partial::{ContentRequest, HistorySizeRequest, PartialEffect};
use glowfly_model::msg::{ContentFailure, Msg, PartialMsg};

pub(super) fn run(e: PartialEffect, cx: &Cx<'_>) -> Msg {
    match e {
        PartialEffect::SizeHistory(r) => size(r, cx),
        PartialEffect::FetchContent(r) => fetch(r, cx.stop),
    }
}

impl Execute for PartialEffect {
    fn execute(self, cx: &Cx<'_>) -> Option<Msg> {
        Some(run(self, cx))
    }
}

/// The hosting service's reported size, else the size on disk of a source on this computer.
fn size(r: HistorySizeRequest, cx: &Cx<'_>) -> Msg {
    let HistorySizeRequest { workdir, id, url, service } = r;
    let reported = service.and_then(|s| {
        let auth = match s.auth {
            Some(a) => Ok(a),
            None => {
                let ask = Ask {
                    account: s.target.account(),
                    cwd: &workdir,
                    interactive: false,
                    connected: s.connected,
                    env_only: false,
                    asked: false,
                };
                sign_in(cx, ask)
            }
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
    let ran = super::job_steps::run(&r.workdir, &r.cmds, false, &stop, &|_| {}, false, &mut |_| {});
    let (ok, output, stopped) = (ran.ok, ran.output, ran.stopped);
    let result = match (ok, stopped) {
        (_, true) => Err(ContentFailure::Stopped),
        (true, false) => Ok(()),
        (false, false) => Err(ContentFailure::Failed(glowfly_core::logs::last_message(&output))),
    };
    Msg::Partial(PartialMsg::ContentFetched { id: r.id, result })
}
