//! The palette's SUBMODULES entries: Add submodule… (even without submodules: the section and
//! its menu only show with some), Initialize all and Update all while they have something to
//! do, and per submodule Open, Initialize / Update / Restore, Edit… and Remove…, each with the
//! row's status word.

use super::{Entry, PaletteCommand as C, entry, with_detail};
use crate::state::AppState;
use crate::state::submodules::batch::{init_all_blocked, update_all_blocked};
use crate::state::submodules::remove::remove_blocked;
use crate::state::submodules::{edit_blocked, open_blocked, row_word, update_blocked};
use crate::submodule::{Batch, SubmoduleMsg};
use glowfly_core::git_version::Feature;
use glowfly_core::repo::{Snapshot, SubmoduleState};

pub(super) fn submodule_entries(st: &AppState, snap: &Snapshot, out: &mut Vec<Entry>) {
    let sub = |m: SubmoduleMsg| C::Submodule(m);
    out.push(entry(sub(SubmoduleMsg::AddDialog), "Add submodule…", "submodule add nested repository dependency vendor library"));
    let list = &snap.submodules;
    if init_all_blocked(list).is_none() {
        let m = sub(SubmoduleMsg::Batch { batch: Batch::InitAll, confirmed: false });
        out.push(entry(m, "Initialize all submodules", "submodule init clone checkout every nested repository"));
    }
    if update_all_blocked(list).is_none() {
        let m = sub(SubmoduleMsg::Batch { batch: Batch::UpdateAll, confirmed: false });
        out.push(entry(m, "Update all submodules", "submodule sync recorded commit checkout every nested repository"));
    }
    let edit_git = st.git.blocked(Feature::SubmoduleEdit);
    for sm in list {
        let detail = row_word(sm);
        let path = &sm.path;
        if open_blocked(sm).is_none() {
            let e = entry(sub(SubmoduleMsg::Open(path.clone())), format!("Open submodule {path}"), "submodule nested repository open tab");
            out.push(with_detail(e, detail.clone()));
        }
        if update_blocked(sm).is_none() {
            let (verb, keywords) = match sm.state {
                SubmoduleState::Missing => ("Restore", "submodule restore folder checkout recorded commit"),
                SubmoduleState::Uninitialized => ("Initialize", "submodule init clone checkout"),
                _ => ("Update", "submodule update sync recorded commit checkout"),
            };
            let m = sub(SubmoduleMsg::Update { path: path.clone(), confirmed: false });
            out.push(with_detail(entry(m, format!("{verb} submodule {path}"), keywords), detail.clone()));
        }
        if edit_blocked(sm, edit_git.as_deref()).is_none() {
            let m = sub(SubmoduleMsg::EditDialog(path.clone()));
            out.push(with_detail(entry(m, format!("Edit submodule {path}…"), "submodule edit url branch remote change"), detail.clone()));
        }
        if remove_blocked(sm, snap).is_none() {
            let m = sub(SubmoduleMsg::RemoveDialog(path.clone()));
            out.push(with_detail(entry(m, format!("Remove submodule {path}…"), "submodule remove delete deinit"), detail));
        }
    }
}
