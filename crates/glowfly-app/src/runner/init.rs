//! The Init dialog's effects (`Effect::Init`): checking its folder, reading git's user.name to
//! suggest as a license's copyright holder, and the init job.

use crate::effect::InitEffect;
use crate::msg::{InitMsg, Msg};
use glowfly_core::gitcli::{self, args};
use glowfly_core::init_ops;

pub(super) fn run(e: InitEffect) -> Msg {
    Msg::Init(match e {
        InitEffect::CheckFolder { folder, delay } => {
            if !delay.is_zero() {
                std::thread::sleep(delay);
            }
            let found = init_ops::inspect_folder(&folder, glowfly_core::settings::home_dir().as_deref());
            InitMsg::FolderChecked { folder, found }
        }
        InitEffect::ReadAuthor { cwd } => {
            let out = gitcli::read(&cwd, &args(&["config", "--get", "user.name"]));
            InitMsg::AuthorRead(Some(out.stdout.trim().to_owned()).filter(|n| out.ok && !n.is_empty()))
        }
        InitEffect::Run { folder, plan } => InitMsg::Finished(init_ops::run(&folder, &plan)),
    })
}
