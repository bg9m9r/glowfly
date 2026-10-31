//! Glowfly's own updates: asking for the newest release, downloading and verifying its file
//! (streamed to disk, with progress at most every `PROGRESS_EVERY` and a stop checked between
//! chunks), checking a downloaded file again, Install and restart (runner/update_restart.rs), and
//! at the start: removing the staging folders and old programs earlier updates left behind, and
//! reading how the last installer run went.

use super::execute::{Cx, Execute};
use super::update_restart;
use crate::effect::UpdateEffect;
use crate::msg::{CheckedRelease, Msg, UpdateMsg};
use glowfly_core::app_update::{Offer, Progress};
use glowfly_core::{app_update, paths, settings};
use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime};

/// A download's progress is sent at most this often.
const PROGRESS_EVERY: Duration = Duration::from_millis(250);

pub(super) fn run(e: UpdateEffect, cx: &Cx<'_>) -> Option<Msg> {
    match e {
        UpdateEffect::Check { manual } => Some(Msg::Updates(UpdateMsg::Checked { manual, result: check() })),
        UpdateEffect::Download(offer) => Some(Msg::Updates(download(&offer, cx))),
        UpdateEffect::CheckSaved { file, sha256 } => {
            let problem = app_update::check_saved(&file, &sha256);
            Some(Msg::Updates(UpdateMsg::SavedChecked { file, problem }))
        }
        UpdateEffect::Restart(job) => Some(update_restart::run(*job)),
        UpdateEffect::Tidy => tidy(),
    }
}

impl Execute for UpdateEffect {
    fn execute(self, cx: &Cx<'_>) -> Option<Msg> {
        run(self, cx)
    }
}

/// Removes what earlier updates left (staging folders wherever Install and restart may put
/// them, and old programs no process runs), then reads the last installer run's problem.
fn tidy() -> Option<Msg> {
    let own = update_restart::own_files().ok();
    let programs: Vec<PathBuf> = own.iter().flat_map(|(exe, image)| std::iter::once(exe.clone()).chain(image.clone())).collect();
    let mut places = vec![paths::temp_dir()];
    for p in &programs {
        places.extend(update_restart::staging_places(p).into_iter().skip(1));
    }
    places.sort();
    places.dedup();
    let n: usize = places.iter().map(|p| app_update::tidy_staging(p, SystemTime::now())).sum();
    tracing::debug!("removed {n} staging folders earlier updates left");
    let cleared = app_update::remove_unused_backups(&programs, glowfly_core::terminal::program_paths);
    tracing::debug!(
        "old programs: removed {:?}, still running {:?}, not recorded {:?}, failed {:?}",
        cleared.removed,
        cleared.running,
        cleared.unrecorded,
        cleared.failed
    );
    let last = settings::log_dir().and_then(|dir| app_update::take_last_install(&dir))?;
    Some(Msg::Updates(UpdateMsg::LastInstall(last)))
}

/// Downloads `offer`'s file into the Downloads folder, sending its progress through `cx.send`
/// and stopping between chunks once `cx.stop` is asked.
fn download(offer: &Offer, cx: &Cx<'_>) -> UpdateMsg {
    let cancelled = || cx.stop.is_some_and(|s| s.requested());
    let mut throttle = Throttle::default();
    let mut progress = |p: Progress| {
        if throttle.due(Instant::now()) {
            (cx.send)(Msg::Updates(UpdateMsg::DownloadProgress(p)));
        }
    };
    let result = app_update::Feed::from_env().and_then(|feed| match settings::downloads_dir() {
        Some(dir) => app_update::download_streamed(&feed, offer, &dir, &mut progress, &cancelled),
        None => Err("No folder to download into".to_owned()),
    });
    match result {
        Ok(Some(saved)) => UpdateMsg::Downloaded(Ok(saved)),
        Ok(None) => UpdateMsg::DownloadCancelled,
        Err(e) => UpdateMsg::Downloaded(Err(e)),
    }
}

/// Lets a report through at most once per `PROGRESS_EVERY`, the first one at once.
#[derive(Default)]
struct Throttle {
    last: Option<Instant>,
}

impl Throttle {
    fn due(&mut self, now: Instant) -> bool {
        if self.last.is_some_and(|last| now.saturating_duration_since(last) < PROGRESS_EVERY) {
            return false;
        }
        self.last = Some(now);
        true
    }
}

/// The newest release (from the hosting service, or the folder GLOWFLY_UPDATE_FEED names), and
/// how this copy was installed.
fn check() -> Result<Option<CheckedRelease>, String> {
    let feed = app_update::Feed::from_env()?;
    if matches!(feed, app_update::Feed::Folder(_)) {
        tracing::info!("checking for updates in the {}", feed.describe());
    }
    let release = feed.latest()?;
    let exe = std::env::current_exe().map_err(|e| format!("Couldn't find Glowfly's own program: {e}"))?;
    let install = app_update::install_kind(&exe);
    Ok(release.map(|release| CheckedRelease { release, install }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn progress_goes_out_a_few_times_a_second() {
        let mut throttle = Throttle::default();
        let t0 = Instant::now();
        let sent: Vec<u64> = (0..=1000u64).step_by(50).filter(|ms| throttle.due(t0 + Duration::from_millis(*ms))).collect();
        assert_eq!(sent, [0, 250, 500, 750, 1000]);
    }
}
