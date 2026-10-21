//! Repositories across folders (`Effect::Repos`): scanning the project folders, then telling
//! apart the spellings that name one folder (a link, nested roots) by identity; and fetching or
//! pulling one listed repository.

use crate::effect::ReposEffect;
use crate::msg::{Msg, ReposMsg, ScanFound};
use crate::state::repo_scan::FoundRepo;
use glowfly_core::paths::FolderId;

pub(super) fn run(e: ReposEffect) -> Msg {
    match e {
        ReposEffect::Scan { generation, roots } => {
            let report = glowfly_core::repo_scan::scan(&roots);
            let mut seen: Vec<FolderId> = Vec::new();
            let mut found = Vec::new();
            for f in report.found {
                let folder = FolderId::of(&f.path);
                if let Some(id) = folder {
                    if seen.contains(&id) {
                        continue; // the same repository, reached again
                    }
                    seen.push(id);
                }
                let name = f.path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                found.push(FoundRepo { root: f.root, path: f.path, name, folder });
            }
            let report = ScanFound { found, unreadable: report.unreadable, truncated: report.truncated };
            Msg::Repos(ReposMsg::Scanned { generation, report: Box::new(report) })
        }
        ReposEffect::Sync { generation, index, path, op, trust } => {
            let outcome = glowfly_core::repo_sync::sync(&path, op, trust.as_ref());
            Msg::Repos(ReposMsg::Synced { generation, index, outcome })
        }
    }
}
