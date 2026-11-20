//! The fork dialogs' hosting-service calls (`Effect::Remote`): listing a repository's forks,
//! finding the user's own fork and forking into the user's account. Signs in the way the pull
//! request calls do, in the request's `cwd` (the dialog's repository only while it is trusted).

use super::execute::{Cx, Execute};
use super::sign_ins::{Ask, sign_in};
use glowfly_core::forge::{self, Auth, Wait};
use glowfly_model::effect::ForgeError;
use glowfly_model::msg::Msg;
use glowfly_model::remote::{ForkRequest, RemoteEffect, RemoteMsg};

fn auth(req: &mut ForkRequest, cx: &Cx<'_>) -> Result<Auth, ForgeError> {
    match req.auth.take() {
        Some(a) => Ok(a),
        None => {
            let ask = Ask {
                account: req.target.account(),
                cwd: &req.cwd,
                interactive: true,
                connected: req.connected,
                env_only: false,
                asked: true,
            };
            sign_in(cx, ask).map_err(ForgeError::from)
        }
    }
}

pub(super) fn run(e: RemoteEffect, cx: &Cx<'_>) -> Msg {
    let msg = match e {
        RemoteEffect::ListForks(mut req) => {
            let result = auth(&mut req, cx).and_then(|a| forge::list_forks(&req.target, &a).map_err(ForgeError::from));
            RemoteMsg::ForksLoaded { workdir: req.workdir, target: req.target, result: Box::new(result) }
        }
        RemoteEffect::FindMyFork(mut req) => {
            let result = auth(&mut req, cx).and_then(|a| forge::find_my_fork(&req.target, &a).map_err(ForgeError::from));
            RemoteMsg::MyForkFound { workdir: req.workdir, target: req.target, result: Box::new(result) }
        }
        RemoteEffect::CreateFork { mut req, name } => {
            let result = auth(&mut req, cx).and_then(|a| forge::create_fork(&req.target, &a, &Wait::default()).map_err(ForgeError::from));
            RemoteMsg::ForkCreated { workdir: req.workdir, target: req.target, name, result: Box::new(result) }
        }
    };
    Msg::Remote(msg)
}

impl Execute for RemoteEffect {
    fn execute(self, cx: &Cx<'_>) -> Option<Msg> {
        Some(run(self, cx))
    }
}
