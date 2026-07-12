use crate::query::{QueryCache, QueryCursor, QueryResultCache};
use crate::time::ChangeTick;

/// Execution options for a resolved query.
pub struct QueryParams<'a> {
    pub(crate) since: Option<ChangeTick>,
    pub(crate) cursor: Option<&'a mut QueryCursor>,
    pub(crate) membership_cache: Option<&'a QueryCache>,
    pub(crate) result_cache: Option<&'a QueryResultCache>,
}

impl<'a> QueryParams<'a> {
    pub fn new() -> Self {
        Self {
            since: None,
            cursor: None,
            membership_cache: None,
            result_cache: None,
        }
    }

    pub fn since(mut self, tick: ChangeTick) -> Self {
        self.since = Some(tick);
        self.cursor = None;
        self
    }

    pub fn cursor(mut self, cursor: &'a mut QueryCursor) -> Self {
        self.cursor = Some(cursor);
        self.since = None;
        self
    }

    pub fn membership_cache(mut self, cache: &'a QueryCache) -> Self {
        self.membership_cache = Some(cache);
        self.result_cache = None;
        self
    }

    pub fn result_cache(mut self, cache: &'a QueryResultCache) -> Self {
        self.result_cache = Some(cache);
        self.membership_cache = None;
        self
    }

    pub(crate) fn since_tick(
        &self,
        fingerprint: u64,
        world: &crate::world::World,
    ) -> Result<ChangeTick, crate::query::QueryError> {
        if let Some(since) = self.since {
            return Ok(since);
        }
        if let Some(cursor) = self.cursor.as_ref() {
            cursor.validate(world, fingerprint)?;
            return Ok(cursor.since());
        }
        Ok(ChangeTick::ZERO)
    }

    pub(crate) fn commit_cursor(
        &mut self,
        fingerprint: u64,
        world: &crate::world::World,
        captured_now: ChangeTick,
    ) -> Result<(), crate::query::QueryError> {
        let Some(cursor) = self.cursor.as_mut() else {
            return Ok(());
        };
        cursor.validate(world, fingerprint)?;
        cursor.commit(captured_now);
        Ok(())
    }
}

impl Default for QueryParams<'_> {
    fn default() -> Self {
        Self::new()
    }
}
