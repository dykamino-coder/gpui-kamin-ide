//! Immutable snapshots keep profile reads independent of the detection worker.
use super::ShellProfile;
use std::sync::{
    OnceLock,
    atomic::{AtomicBool, Ordering},
};

pub(super) struct ProfileCache {
    started: AtomicBool,
    local: OnceLock<Vec<ShellProfile>>,
    complete: OnceLock<Vec<ShellProfile>>,
}

impl ProfileCache {
    pub(super) const fn new() -> Self {
        Self {
            started: AtomicBool::new(false),
            local: OnceLock::new(),
            complete: OnceLock::new(),
        }
    }

    pub(super) fn profiles(&self) -> &[ShellProfile] {
        self.complete
            .get()
            .or_else(|| self.local.get())
            .map(Vec::as_slice)
            .unwrap_or(&[])
    }

    pub(super) fn profile_by_id(&self, id: &str) -> Option<&ShellProfile> {
        let all = self.profiles();
        all.iter().find(|profile| profile.id == id).or_else(|| {
            if id.is_empty() || self.complete.get().is_some() {
                all.first()
            } else {
                None
            }
        })
    }

    pub(super) fn start(
        &'static self,
        local: impl FnOnce() -> Vec<ShellProfile> + Send + 'static,
        wsl: impl FnOnce(&mut Vec<ShellProfile>) + Send + 'static,
        notify: impl Fn() + Send + 'static,
    ) {
        if self.started.swap(true, Ordering::AcqRel) {
            return;
        }
        if let Err(error) = std::thread::Builder::new()
            .name("kamin-shell-profiles".into())
            .spawn(move || {
                let mut profiles = local();
                let _ = self.local.set(profiles.clone());
                notify();
                wsl(&mut profiles);
                let _ = self.complete.set(profiles);
                notify();
            })
        {
            self.started.store(false, Ordering::Release);
            eprintln!("[terminal] cannot start profile detection: {error}");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, atomic::AtomicUsize, mpsc};
    use std::time::Duration;

    fn profile(id: &str) -> ShellProfile {
        ShellProfile {
            id: id.into(),
            label: id.into(),
            program: id.into(),
            args: vec![],
            icon: "terminal-cmd",
        }
    }

    #[test]
    fn render_reads_do_not_start_or_wait_for_detection() {
        let cache: &'static ProfileCache = Box::leak(Box::new(ProfileCache::new()));
        // The same accessors used by render are inert before startup detection.
        assert!(cache.profiles().is_empty());
        assert!(cache.profile_by_id("").is_none());
        let (release, wait) = mpsc::channel();
        let (wake, woke) = mpsc::channel();
        let calls = Arc::new(AtomicUsize::new(0));
        let detector_calls = calls.clone();
        let caller = std::thread::current().id();
        cache.start(
            move || {
                assert_ne!(std::thread::current().id(), caller);
                detector_calls.fetch_add(1, Ordering::SeqCst);
                vec![profile("cmd")]
            },
            move |profiles| {
                wait.recv_timeout(Duration::from_secs(5)).unwrap();
                profiles.push(profile("wsl-test"));
            },
            move || {
                wake.send(()).unwrap();
            },
        );
        woke.recv_timeout(Duration::from_secs(5)).unwrap();
        // WSL is still blocked: native profiles and render reads remain available.
        for _ in 0..1000 {
            assert_eq!(cache.profiles()[0].id, "cmd");
            assert!(cache.profile_by_id("wsl-test").is_none());
            assert_eq!(cache.profile_by_id("").unwrap().id, "cmd");
        }
        cache.start(
            || panic!("duplicate discovery"),
            |_| panic!("duplicate WSL"),
            || {},
        );
        release.send(()).unwrap();
        woke.recv_timeout(Duration::from_secs(5)).unwrap();
        assert_eq!(cache.profile_by_id("wsl-test").unwrap().id, "wsl-test");
        assert_eq!(cache.profile_by_id("missing").unwrap().id, "cmd");
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn empty_detection_is_safe() {
        let cache: &'static ProfileCache = Box::leak(Box::new(ProfileCache::new()));
        let (wake, woke) = mpsc::channel();
        cache.start(
            Vec::new,
            |_| {},
            move || {
                wake.send(()).unwrap();
            },
        );
        for _ in 0..2 {
            woke.recv_timeout(Duration::from_secs(5)).unwrap();
        }
        assert!(cache.profile_by_id("missing").is_none());
    }
}
