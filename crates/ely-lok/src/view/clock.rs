//! Times an edit to the tile showing it (PERF-02).
use std::time::Instant;

/// One edit's clock: sent when, its revision, its generation.
type Edit = (Instant, Option<u64>, Option<u64>);

#[derive(Default)]
pub(super) struct EditClock {
    edit: Option<Edit>,
    /// The last generation bump: its revision and generation.
    bumped: (u64, u64),
}

impl EditClock {
    /// An edit queued; an unseen earlier start stays.
    pub(super) fn sent(&mut self) {
        self.edit.get_or_insert((Instant::now(), None, None));
    }

    /// The edit landed at `revision`; its batch may precede it.
    pub(super) fn landed(&mut self, revision: Option<u64>) {
        let Some((_, rev @ None, generation)) = &mut self.edit else {
            return;
        };
        let Some(revision) = revision else {
            self.edit = None;
            return;
        };
        *rev = Some(revision);
        if self.bumped.0 >= revision {
            *generation = Some(self.bumped.1);
        }
    }

    /// A batch at `revision` dirtied tiles into `generation`.
    pub(super) fn bumped(&mut self, revision: u64, generation: u64) {
        self.bumped = (revision, generation);
        if let Some((_, Some(rev), shown @ None)) = &mut self.edit
            && revision >= *rev
        {
            *shown = Some(generation);
        }
    }

    /// A tile of `generation` came; logs the edit it shows.
    pub(super) fn painted(&mut self, generation: u64) {
        if let Some((at, _, Some(shown))) = self.edit
            && generation >= shown
        {
            log::debug!("lok: edit to tile {} ms", at.elapsed().as_millis());
            self.edit = None;
        }
    }

    #[cfg(test)]
    fn running(&self) -> bool {
        self.edit.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::EditClock;

    #[test]
    fn a_batch_before_the_landing_still_ends_the_edit() {
        let mut clock = EditClock::default();
        clock.sent();
        clock.bumped(7, 3);
        clock.landed(Some(7));
        clock.painted(2);
        assert!(clock.running(), "an older picture leaves it running");
        clock.painted(3);
        assert!(!clock.running(), "the edit's own picture ends it");
    }

    #[test]
    fn only_a_batch_at_the_edit_revision_counts() {
        let mut clock = EditClock::default();
        clock.sent();
        clock.bumped(6, 3);
        clock.landed(Some(7));
        clock.painted(3);
        assert!(clock.running(), "a batch before the edit shows the old text");
        clock.bumped(7, 4);
        clock.painted(4);
        assert!(!clock.running());
    }

    #[test]
    fn an_unread_revision_drops_the_edit() {
        let mut clock = EditClock::default();
        clock.sent();
        clock.landed(None);
        assert!(!clock.running());
    }
}
