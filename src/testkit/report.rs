//! Completed replay evidence and partial failure reports.

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

    /// Seed from the replay config used for this run.
    pub fn seed(&self) -> u64 {
        self.seed
    }

    /// Replay policy that governed capture and step count.
    pub fn config(&self) -> &ReplayConfig {
        &self.config
    }

    /// Captured step records after [`CapturePolicy`](super::config::CapturePolicy) filtering.
    pub fn step_snapshots(&self) -> &[StepSnapshot<S>] {
        &self.steps
    }

    /// Host fixture snapshots across captured replay steps.
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

    /// Checked replay step where the run failed.
    pub fn step(&self) -> StepIndex {
        self.step
    }

    /// Step records captured before the failure, if any.
    pub fn partial_report(&self) -> &ReplayReport<S> {
        &self.partial_report
    }

    /// Replay failure source at the failing step.
    pub fn source(&self) -> &E {
        &self.source
    }
}
