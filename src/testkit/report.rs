use alloc::vec::Vec;

use super::config::ReplayConfig;
use super::record::{StepRecord, StepSnapshot};
use super::step::StepIndex;

/// Completed replay evidence for a finite run.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplayReport<S> {
    seed: u64,
    config: ReplayConfig,
    steps: Vec<StepSnapshot<S>>,
}

/// Failed replay step with partial evidence retained.
#[derive(Clone, Debug, PartialEq)]
pub struct ReplayFailure<E, S> {
    step: StepIndex,
    partial_report: ReplayReport<S>,
    source: E,
}

impl<S> ReplayReport<S> {
    pub(crate) fn new(config: ReplayConfig) -> Self {
        Self {
            seed: config.seed(),
            config,
            steps: Vec::new(),
        }
    }

    pub(crate) fn push_record(&mut self, record: StepRecord<S>) {
        self.steps.push(record.into_snapshot());
    }

    pub fn seed(&self) -> u64 {
        self.seed
    }

    pub fn config(&self) -> &ReplayConfig {
        &self.config
    }

    pub fn step_snapshots(&self) -> &[StepSnapshot<S>] {
        &self.steps
    }

    pub fn snapshots(&self) -> impl Iterator<Item = &S> {
        self.steps.iter().map(|step| step.snapshot())
    }
}

impl<E, S> ReplayFailure<E, S> {
    pub(crate) fn new(step: StepIndex, partial_report: ReplayReport<S>, source: E) -> Self {
        Self {
            step,
            partial_report,
            source,
        }
    }

    pub fn step(&self) -> StepIndex {
        self.step
    }

    pub fn partial_report(&self) -> &ReplayReport<S> {
        &self.partial_report
    }

    pub fn source(&self) -> &E {
        &self.source
    }
}
