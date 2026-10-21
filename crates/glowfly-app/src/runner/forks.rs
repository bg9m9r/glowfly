//! The fork dialogs' hosting-service calls (`Effect::Remote`): listing a repository's forks,
//! finding the user's own fork and forking into the user's account. Signs in the way the pull
//! request calls do, in the dialog's repository.

use crate::effect::ForgeError;
use crate::msg::Msg;
use crate::remote::{ForkRequest, RemoteEffect, RemoteMsg};
use glowfly_core::forge::{self, Auth, Wait};

fn auth(req: &mut ForkRequest) -> Result<Auth, ForgeError> {
    match req.auth.take() {
        Some(a) => Ok(a),
        None => forge::resolve_auth(&req.target.account(), &req.workdir, true, req.connected).map_err(ForgeError::from),
    }
}

pub(super) fn run(e: RemoteEffect) -> Msg {
    let msg = match e {
        RemoteEffect::ListForks(mut req) => {
            let result = auth(&mut req).and_then(|a| forge::list_forks(&req.target, &a).map_err(ForgeError::from));
            RemoteMsg::ForksLoaded { workdir: req.workdir, target: req.target, result: Box::new(result) }
        }
        RemoteEffect::FindMyFork(mut req) => {
            let result = auth(&mut req).and_then(|a| forge::find_my_fork(&req.target, &a).map_err(ForgeError::from));
            RemoteMsg::MyForkFound { workdir: req.workdir, target: req.target, result: Box::new(result) }
        }
        RemoteEffect::CreateFork { mut req, name } => {
            let result = auth(&mut req).and_then(|a| forge::create_fork(&req.target, &a, &Wait::default()).map_err(ForgeError::from));
            RemoteMsg::ForkCreated { workdir: req.workdir, target: req.target, name, result: Box::new(result) }
        }
    };
    Msg::Remote(msg)
}
