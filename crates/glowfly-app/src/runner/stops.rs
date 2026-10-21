//! The stops of the git commands that can be cancelled (`Effect::stop_key`): registered when
//! their effect is handed to `Runner::run` (so a stop asked while it still waits in its queue
//! is kept), asked by `Effect::StopGit`, and released when the effect ends.

use crate::effect::partial::StopKey;
use glowfly_core::gitcli::Stop;
use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

#[derive(Default)]
pub(super) struct Stops(Mutex<HashMap<StopKey, Stop>>);

impl Stops {
    fn map(&self) -> MutexGuard<'_, HashMap<StopKey, Stop>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// A new stop for the effect registered under `key` (replacing an older one's).
    pub fn register(&self, key: StopKey) -> Stop {
        let stop = Stop::new();
        self.map().insert(key, stop.clone());
        stop
    }

    /// Asks the command registered under `key` to stop; false when none is.
    pub fn stop(&self, key: StopKey) -> bool {
        self.map().get(&key).inspect(|s| s.stop()).is_some()
    }

    /// The effect that registered `stop` ended (a newer one under the same key stays).
    pub fn release(&self, key: StopKey, stop: &Stop) {
        let mut map = self.map();
        if map.get(&key).is_some_and(|s| Stop::same(s, stop)) {
            map.remove(&key);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_stop_reaches_only_the_latest_registration() {
        let stops = Stops::default();
        assert!(!stops.stop(StopKey::Job), "nothing registered");
        let old = stops.register(StopKey::Job);
        let new = stops.register(StopKey::Job);
        stops.release(StopKey::Job, &old);
        assert!(stops.stop(StopKey::Job), "the newer one stays registered");
        assert!(new.requested() && !old.requested());
        let content = stops.register(StopKey::Content(3));
        assert!(!content.requested(), "keys are apart");
        stops.release(StopKey::Job, &new);
        assert!(!stops.stop(StopKey::Job));
        assert!(stops.stop(StopKey::Content(3)) && content.requested());
    }
}
