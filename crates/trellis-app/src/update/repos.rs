//! Repositories across folders: the Find repositories dialog and its scan (repo_scan.rs).

use crate::effect::Effect;
use crate::msg::ReposMsg;
use crate::state::AppState;

pub(super) fn handle(st: &mut AppState, msg: ReposMsg, fx: &mut Vec<Effect>) {
    match msg {
        ReposMsg::ShowFinder => super::repo_scan::show(st, fx),
        ReposMsg::AddFolder => super::repo_scan::add_folder(fx),
        ReposMsg::RemoveFolder(p) => super::repo_scan::remove_folder(st, &p, fx),
        ReposMsg::Rescan => super::repo_scan::rescan(st, fx),
        ReposMsg::Scanned { generation, report } => super::repo_scan::scanned(st, generation, *report),
    }
}
