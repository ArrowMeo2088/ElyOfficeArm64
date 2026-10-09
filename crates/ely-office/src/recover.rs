//! Recovery points of open documents, back after crashes (§6.3).

use std::{
    collections::HashMap,
    io,
    path::{Path, PathBuf},
    time::{Duration, SystemTime},
};

use ely_files::{Crash, Marker, Recovery, RecoveryKey};
use ely_gpui_component::primitives::Severity;
use gpui::{AsyncApp, Context, WeakEntity};
use serde::{Deserialize, Serialize};

use crate::{Engine, Host, Opening as _, Source, Tabs as _, text};

/// A tab's points, and the edit the last holds.
struct Slot {
    kept: Kept,
    written: Option<u64>,
    writing: bool,
}

/// What reopening a slot needs, kept beside its points.
#[derive(Clone, Debug, Serialize, Deserialize)]
struct Kept {
    slot: String,
    name: String,
    format: String,
    sealed: bool,
    /// Not reopened yet; a clean end keeps it.
    #[serde(default)]
    held: bool,
}

/// The app's recovery: marker, slots, what a crash left.
pub struct Keeping {
    dir: PathBuf,
    crash: Option<Crash>,
    slots: HashMap<u64, Slot>,
    crashed: Vec<Kept>,
    /// Copies opening from points: path, point time, old slot.
    reopening: Vec<(PathBuf, SystemTime, Kept)>,
    /// Slots that could not reopen; kept for a later start.
    held: Vec<Kept>,
    /// When the last point was written.
    pub(crate) last: Option<SystemTime>,
    /// The index on disk is behind; the failure shown.
    behind: Option<String>,
}

impl Keeping {
    /// Takes the marker; a crash leaves its slots to reopen.
    pub fn start(app: ely_platform::AppId) -> io::Result<Self> {
        Self::start_in(ely_platform::data_dir(app).map_err(io::Error::other)?)
    }

    fn start_in(dir: PathBuf) -> io::Result<Self> {
        let marker = Crash::check(&dir, ely_files::alive)?;
        log::info!("recovery: the last run ended {marker:?}");
        let listed = read_index(&dir)?;
        let crashed = match marker {
            Marker::Crashed => listed,
            // A clean end keeps only what never reopened.
            Marker::Clean | Marker::Discarded => {
                let held: Vec<Kept> = listed.into_iter().filter(|kept| kept.held).collect();
                prune(&dir.join("recovery"), &held)?;
                held
            }
        };
        let crash = Crash::open(&dir, std::process::id())?;
        let (reopening, held) = (Vec::new(), Vec::new());
        Ok(Self { dir, crash: Some(crash), slots: HashMap::new(), crashed, reopening, held, last: None, behind: None })
    }

    /// A clean quit: the next start finds nothing to recover.
    pub fn end(&mut self) {
        if let Some(crash) = self.crash.take()
            && let Err(error) = crash.close()
        {
            log::error!("recovery: the clean-quit mark failed: {error}");
        }
    }

    fn folder(&self, slot: &str) -> PathBuf {
        self.dir.join("recovery").join(slot)
    }

    fn save_index(&self) -> io::Result<()> {
        let waiting = self.reopening.iter().map(|(_, _, kept)| kept).chain(&self.held);
        let waiting = waiting.map(|kept| Kept { held: true, ..kept.clone() });
        let slots = self.slots.values().map(|slot| slot.kept.clone());
        let kept: Vec<Kept> = slots.chain(self.crashed.iter().cloned()).chain(waiting).collect();
        let path = self.dir.join("recovery").join("index.json");
        std::fs::create_dir_all(self.dir.join("recovery"))?;
        // Staged then renamed: a failed write keeps the last index.
        ely_files::save_atomic(&path, &serde_json::to_vec(&kept).map_err(io::Error::other)?, 0).map(|_| ())
    }
}

fn read_index(dir: &Path) -> io::Result<Vec<Kept>> {
    match std::fs::read(dir.join("recovery").join("index.json")) {
        Ok(bytes) => serde_json::from_slice(&bytes).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error)),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(Vec::new()),
        Err(error) => Err(error),
    }
}

/// Removes every slot folder but those `held`.
fn prune(recovery: &Path, held: &[Kept]) -> io::Result<()> {
    if held.is_empty() {
        return remove(recovery);
    }
    for entry in std::fs::read_dir(recovery)? {
        let entry = entry?;
        let name = entry.file_name();
        if entry.file_type()?.is_dir() && !held.iter().any(|kept| *kept.slot == *name) {
            remove(&entry.path())?;
        }
    }
    Ok(())
}

fn remove(path: &Path) -> io::Result<()> {
    match std::fs::remove_dir_all(path) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

/// When recovery points are written and crashes come back.
pub trait Recovering: Host {
    /// From the app's window, never its tests.
    fn start_recovery(&mut self, cx: &mut Context<Self>) {
        match Keeping::start(Self::KIND.app) {
            Ok(keeping) => self.core_mut().keeping = Some(keeping),
            Err(error) => {
                log::error!("recovery: off for this run: {error}");
                self.core_mut().recovery_off = Some(error.to_string());
            }
        }
        Self::keep_points_every(cx);
        cx.on_app_quit(|host, _| {
            if let Some(keeping) = host.core_mut().keeping.as_mut() {
                keeping.end();
            }
            async {}
        })
        .detach();
    }

    /// Ticks every ten seconds for the window's life.
    fn keep_points_every(cx: &mut Context<Self>) {
        cx.spawn(async move |this: WeakEntity<Self>, cx: &mut AsyncApp| {
            loop {
                cx.background_executor().timer(Duration::from_secs(2)).await;
                if this.update(cx, |host, cx| host.keep_points(cx)).is_err() {
                    break;
                }
                cx.background_executor().timer(Recovery::INTERVAL - Duration::from_secs(2)).await;
            }
        })
        .detach();
    }

    /// Reopens what a crash left, then writes due points.
    fn keep_points(&mut self, cx: &mut Context<Self>) {
        // An idle engine reports no error when it ends.
        crate::Stopping::check_engine(self, cx);
        if let Some(error) = self.core_mut().recovery_off.take() {
            self.toast_unlogged(text(cx, "office.recovery.off", &[("error", &error)]), Severity::Danger, cx);
        }
        if self.core().stopped.is_some() {
            return;
        }
        if self.core().keeping.as_ref().is_some_and(|keeping| keeping.behind.is_some()) {
            self.flush_index(cx);
        }
        if self.core().window.is_some() {
            self.reopen_crashed(cx);
        }
        let app = Self::KIND.app;
        let due: Vec<(u64, u64)> = self
            .core()
            .tabs
            .iter()
            .filter(|tab| tab.dirty(cx) && !tab.exporting && Self::Engine::settled(tab.view.read(cx)))
            .map(|tab| (tab.id, Self::Engine::edited(tab.view.read(cx))))
            .filter(|(id, edited)| {
                let slot = self.core().keeping.as_ref().and_then(|keeping| keeping.slots.get(id));
                slot.is_some_and(|slot| !slot.writing && slot.written != Some(*edited))
            })
            .collect();
        for (id, edited) in due {
            let Some(keeping) = self.core_mut().keeping.as_mut() else { return };
            let slot = keeping.slots.get_mut(&id).expect("a due tab has a slot");
            slot.writing = true;
            let kept = slot.kept.clone();
            let folder = keeping.folder(&kept.slot);
            let work = self.tab(id).expect("a due tab").work.join(format!("recovery.{}", kept.format));
            self.read_in(
                id,
                move |worker, doc| {
                    Self::Engine::snapshot(worker, doc, &work, &kept.format)?;
                    let bytes = std::fs::read(&work)?;
                    std::fs::remove_file(&work)?;
                    let limits = ely_platform::storage::limits(app)?;
                    let recovery = match kept.sealed {
                        true => Recovery::sensitive(folder, RecoveryKey::load().key),
                        false => Recovery::new(folder),
                    };
                    let mut recovery = recovery.with_limits(limits.keep, limits.megabytes * 1024 * 1024);
                    recovery.write_once(&bytes, None, SystemTime::now()).map(|_| ())
                },
                move |host, written, _, cx| {
                    let Some(keeping) = host.core_mut().keeping.as_mut() else { return };
                    let Some(slot) = keeping.slots.get_mut(&id) else { return };
                    slot.writing = false;
                    match written {
                        Ok(()) => {
                            slot.written = Some(edited);
                            keeping.last = Some(SystemTime::now());
                            if keeping.behind.is_some() {
                                host.flush_index(cx);
                            }
                        }
                        Err(error) => {
                            log::error!("recovery: tab {id} point failed ({:?})", error.kind());
                            let shown = text(cx, "office.recovery.failed", &[("error", &error.to_string())]);
                            host.toast_unlogged(shown, Severity::Warning, cx);
                        }
                    }
                },
                cx,
            );
        }
    }

    /// Writes the index; a failure shows once and retries.
    fn flush_index(&mut self, cx: &mut Context<Self>) {
        let Some(keeping) = self.core_mut().keeping.as_mut() else { return };
        let error = match keeping.save_index() {
            Ok(()) => return keeping.behind = None,
            Err(error) => error.to_string(),
        };
        if keeping.behind.as_ref() == Some(&error) {
            return;
        }
        log::error!("recovery: the index is not saved: {error}");
        keeping.behind = Some(error.clone());
        self.toast_unlogged(text(cx, "office.recovery.index", &[("error", &error)]), Severity::Danger, cx);
    }

    /// Each tab gets its own folder of points once open.
    fn keep_tab(&mut self, id: u64, name: &str, sealed: bool, cx: &mut Context<Self>) {
        let format = name
            .rsplit_once('.')
            .map(|(_, ext)| ext.to_ascii_lowercase())
            .filter(|ext| Self::KIND.save_as.contains(&ext.as_str()) || Self::KIND.kept.contains(&ext.as_str()))
            .unwrap_or_else(|| Self::KIND.save_as[0].to_string());
        let Some(keeping) = self.core_mut().keeping.as_mut() else { return };
        let slot = format!("{}-{id}", std::process::id());
        let kept = Kept { slot, name: name.to_string(), format, sealed, held: false };
        keeping.slots.insert(id, Slot { kept, written: None, writing: false });
        self.flush_index(cx);
    }

    /// A closed tab's points go: closing decided its fate.
    fn forget_tab(&mut self, id: u64, cx: &mut Context<Self>) {
        let Some(keeping) = self.core_mut().keeping.as_mut() else { return };
        let Some(slot) = keeping.slots.remove(&id) else { return };
        if let Err(error) = remove(&keeping.folder(&slot.kept.slot)) {
            log::error!("recovery: tab {id} points stay: {error}");
        }
        self.flush_index(cx);
    }

    /// Opens each crashed slot's newest point as unsaved copy.
    fn reopen_crashed(&mut self, cx: &mut Context<Self>) {
        let Some(keeping) = self.core_mut().keeping.as_mut() else { return };
        let crashed = std::mem::take(&mut keeping.crashed);
        if crashed.is_empty() {
            return;
        }
        for kept in crashed {
            let folder = self.core().keeping.as_ref().expect("keeping").folder(&kept.slot);
            match restore(&kept, &folder, &self.core().work, self.core().next, false) {
                Ok(Some((copy, at))) => {
                    log::info!("recovery: reopening a crashed {}", kept.format);
                    self.core_mut().keeping.as_mut().expect("keeping").reopening.push((copy.clone(), at, kept));
                    self.load(Source::Copy(copy), cx);
                }
                Ok(None) => {
                    if let Err(error) = remove(&folder) {
                        log::error!("recovery: an empty slot stays: {error}");
                    }
                }
                Err(error) => {
                    log::error!("recovery: a crashed copy cannot open ({:?})", error.kind());
                    let shown = text(cx, "office.recovery.lost", &[("name", &kept.name), ("error", &error.to_string())]);
                    self.toast_unlogged(shown, Severity::Danger, cx);
                    // Its points stay for a later start.
                    self.core_mut().keeping.as_mut().expect("keeping").held.push(kept);
                }
            }
        }
        self.flush_index(cx);
    }

    /// Opens `id`'s newest point as a copy; false if none.
    fn reopen_point(&mut self, id: u64, cx: &mut Context<Self>) -> io::Result<bool> {
        let (work, next) = (self.core().work.clone(), self.core().next);
        let Some(keeping) = self.core_mut().keeping.as_mut() else { return Ok(false) };
        // Out of the tab's hands: releasing it keeps these points.
        let Some(slot) = keeping.slots.remove(&id) else { return Ok(false) };
        let restored = restore(&slot.kept, &keeping.folder(&slot.kept.slot), &work, next, true);
        match restored {
            Ok(Some((copy, at))) => {
                keeping.reopening.push((copy.clone(), at, slot.kept));
                self.load(Source::Copy(copy), cx);
                Ok(true)
            }
            Ok(None) => {
                keeping.slots.insert(id, slot);
                Ok(false)
            }
            Err(error) => {
                keeping.held.push(slot.kept);
                Err(error)
            }
        }
    }

    /// A reopened copy takes over its old points, unsaved.
    fn reopened(&mut self, id: u64, path: &Path, cx: &mut Context<Self>) {
        let Some(keeping) = self.core_mut().keeping.as_mut() else { return };
        let Some(n) = keeping.reopening.iter().position(|(copy, ..)| copy == path) else {
            return;
        };
        let (_, at, kept) = keeping.reopening.remove(n);
        let name = kept.name.clone();
        if let Some(slot) = keeping.slots.get_mut(&id) {
            let sealed = slot.kept.sealed || kept.sealed;
            slot.kept = Kept { sealed, held: false, ..kept };
        }
        // The point it took over is the latest kept.
        keeping.last = Some(keeping.last.map_or(at, |last| last.max(at)));
        self.flush_index(cx);
        let Some(tab) = self.tab_mut(id) else { return };
        tab.session.edit();
        let shown = text(cx, "office.recovery.back", &[("name", &name), ("at", &clock(at))]);
        self.toast_unlogged(shown, Severity::Warning, cx);
    }
}

impl<H: Host> Recovering for H {}

/// The newest point copied to the work folder, and when.
fn restore(kept: &Kept, folder: &Path, work: &Path, next: u64, this_run: bool) -> io::Result<Option<(PathBuf, SystemTime)>> {
    let recovery = match kept.sealed {
        true => {
            let key = RecoveryKey::load();
            // This run's points open with this run's key.
            if !key.lasting && !this_run {
                return Err(io::Error::new(io::ErrorKind::PermissionDenied, "the recovery key is not in the key store"));
            }
            Recovery::sensitive(folder.to_path_buf(), key.key)
        }
        false => Recovery::new(folder.to_path_buf()),
    };
    let Some(point) = recovery.points()?.into_iter().next() else {
        return Ok(None);
    };
    let dir = crate::files::work_dir(work, next)?;
    let copy = dir.join(&kept.name).with_extension(&kept.format);
    recovery.restore(&point, &copy)?;
    Ok(Some((copy, point.at)))
}

/// A recovery time as this machine's clock reads it.
pub(crate) fn clock(at: SystemTime) -> String {
    match jiff::Timestamp::try_from(at) {
        Ok(at) => at.to_zoned(jiff::tz::TimeZone::system()).strftime("%Y-%m-%d %H:%M").to_string(),
        Err(error) => error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{Keeping, Kept};

    fn kept(slot: &str, held: bool) -> Kept {
        Kept { slot: slot.into(), name: "a.docx".into(), format: "docx".into(), sealed: false, held }
    }

    #[cfg(unix)]
    #[test]
    fn a_failed_index_write_keeps_the_last_one() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("a temp dir");
        let mut keeping = Keeping::start_in(dir.path().to_path_buf()).expect("starts");
        keeping.crashed.push(kept("1-0", false));
        keeping.save_index().expect("the first index");
        let recovery = dir.path().join("recovery");
        std::fs::set_permissions(&recovery, std::fs::Permissions::from_mode(0o555)).expect("locks");
        keeping.crashed.push(kept("1-1", false));
        let failed = keeping.save_index();
        std::fs::set_permissions(&recovery, std::fs::Permissions::from_mode(0o755)).expect("unlocks");
        assert!(failed.is_err(), "the folder refused the write");
        let read = super::read_index(dir.path()).expect("the old index still reads");
        assert_eq!(read.len(), 1);
    }

    #[test]
    fn a_clean_end_keeps_only_what_never_reopened() {
        let dir = tempfile::tempdir().expect("a temp dir");
        let mut keeping = Keeping::start_in(dir.path().to_path_buf()).expect("starts");
        for slot in ["1-0", "1-1"] {
            std::fs::create_dir_all(keeping.folder(slot).join("points")).expect("a slot");
        }
        keeping.crashed.push(kept("1-0", false));
        keeping.held.push(kept("1-1", false));
        keeping.save_index().expect("the index");
        keeping.end();
        let again = Keeping::start_in(dir.path().to_path_buf()).expect("starts again");
        let slots: Vec<&str> = again.crashed.iter().map(|kept| kept.slot.as_str()).collect();
        assert_eq!(slots, ["1-1"], "only the held slot comes back");
        assert!(again.folder("1-1").exists() && !again.folder("1-0").exists());
    }
}
