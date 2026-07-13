use alloc::rc::Rc;
use alloc::vec::Vec;

use crate::query::{QueryError, QuerySpec};
use crate::world::World;

use super::plan::ResolvedPlan;
use super::spec::{resolve_entities, resolve_query1, resolve_query2};

#[derive(Default)]
pub(crate) struct QueryResolveScratch {
    pub required: Vec<usize>,
    pub without: Vec<usize>,
    pub with_tags: Vec<usize>,
    pub without_tags: Vec<usize>,
}

impl World {
    pub(crate) fn resolve_entity_plan(
        &mut self,
        spec: &QuerySpec,
    ) -> Result<Rc<ResolvedPlan>, QueryError> {
        let fingerprint = {
            let mut scratch = self.query_resolve_scratch.borrow_mut();
            super::spec::peek_entities_fingerprint(self, spec, &mut scratch)?
        };
        if let Some(plan) = self.resolved_plan_cache.get(&fingerprint) {
            return Ok(plan.clone());
        }
        let plan = {
            let mut scratch = self.query_resolve_scratch.borrow_mut();
            Rc::new(resolve_entities(self, spec, &mut scratch)?)
        };
        self.resolved_plan_cache.insert(fingerprint, plan.clone());
        Ok(plan)
    }

    pub(crate) fn resolve_query1_plan<T: 'static>(
        &mut self,
        spec: &QuerySpec,
    ) -> Result<Rc<ResolvedPlan>, QueryError> {
        let fingerprint = {
            let mut scratch = self.query_resolve_scratch.borrow_mut();
            super::spec::peek_query1_fingerprint::<T>(self, spec, &mut scratch)?
        };
        if let Some(plan) = self.resolved_plan_cache.get(&fingerprint) {
            return Ok(plan.clone());
        }
        let plan = {
            let mut scratch = self.query_resolve_scratch.borrow_mut();
            Rc::new(resolve_query1::<T>(self, spec, &mut scratch)?)
        };
        self.resolved_plan_cache.insert(fingerprint, plan.clone());
        Ok(plan)
    }

    pub(crate) fn resolve_query2_plan<A: 'static, B: 'static>(
        &mut self,
        spec: &QuerySpec,
    ) -> Result<(Rc<ResolvedPlan>, usize, bool), QueryError> {
        let (plan, second_index, second_is_table) = {
            let mut scratch = self.query_resolve_scratch.borrow_mut();
            resolve_query2::<A, B>(self, spec, &mut scratch)?
        };
        let cached = self
            .resolved_plan_cache
            .entry(plan.fingerprint)
            .or_insert_with(|| Rc::new(plan.clone()))
            .clone();
        Ok((cached, second_index, second_is_table))
    }
}
