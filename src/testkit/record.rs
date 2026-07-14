//! Per-step replay records and captured report snapshots.

use alloc::string::String;
use alloc::vec::Vec;

use crate::time::WorldTick;

use super::step::StepIndex;

/// Scalar metric sample for report interoperability. Not the exact state proof.
#[derive(Clone, Debug, PartialEq)]
pub struct MetricSample {
    key: String,
    value: f64,
}

impl MetricSample {
    /// Record one scalar metric alongside a replay step snapshot.
    pub fn new(key: impl Into<String>, value: f64) -> Self {
        Self {
            key: key.into(),
            value,
        }
    }

    /// Metric identifier for report comparison.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Metric value stored with bit-exact equality in [`reports_match`](super::replay::reports_match).
    pub fn value(&self) -> f64 {
        self.value
    }
}

/// One replay step before capture policy is applied.
#[derive(Clone, Debug, PartialEq)]
pub struct StepRecord<S> {
    step: StepIndex,
    world_tick: Option<WorldTick>,
    snapshot: S,
    metrics: Vec<MetricSample>,
}

/// Captured host snapshot for one replay step.
#[derive(Clone, Debug, PartialEq)]
pub struct StepSnapshot<S> {
    step: StepIndex,
    world_tick: Option<WorldTick>,
    snapshot: S,
    metrics: Vec<MetricSample>,
}

impl<S> StepRecord<S> {
    /// Build one replay step record before [`CapturePolicy`](super::config::CapturePolicy) filtering.
    pub fn new(
        step: StepIndex,
        world_tick: Option<WorldTick>,
        snapshot: S,
        metrics: Vec<MetricSample>,
    ) -> Self {
        Self {
            step,
            world_tick,
            snapshot,
            metrics,
        }
    }

    /// Checked replay step this record claims to represent.
    pub fn step(&self) -> StepIndex {
        self.step
    }

    /// [`WorldTick`](crate::time::WorldTick) observed after the step flush, when available.
    pub fn world_tick(&self) -> Option<WorldTick> {
        self.world_tick
    }

    /// Host fixture snapshot proving world state for this step.
    pub fn snapshot(&self) -> &S {
        &self.snapshot
    }

    /// Optional scalar metrics recorded with this step.
    pub fn metrics(&self) -> &[MetricSample] {
        &self.metrics
    }

    /// Convert a captured record into report storage.
    pub fn into_snapshot(self) -> StepSnapshot<S> {
        StepSnapshot {
            step: self.step,
            world_tick: self.world_tick,
            snapshot: self.snapshot,
            metrics: self.metrics,
        }
    }
}

impl<S> StepSnapshot<S> {
    /// Replay step index for this captured evidence.
    pub fn step(&self) -> StepIndex {
        self.step
    }

    /// World tick stored with the captured snapshot, when recorded.
    pub fn world_tick(&self) -> Option<WorldTick> {
        self.world_tick
    }

    /// Host snapshot retained in the replay report.
    pub fn snapshot(&self) -> &S {
        &self.snapshot
    }

    /// Metrics retained alongside the captured snapshot.
    pub fn metrics(&self) -> &[MetricSample] {
        &self.metrics
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::vec;

    #[test]
    fn record_accessors_preserve_snapshot_and_metrics() {
        let step = StepIndex::from_raw(2);
        let tick = WorldTick::ZERO;
        let record = StepRecord::new(
            step,
            Some(tick),
            11_u32,
            vec![MetricSample::new("work", 1.5)],
        );
        assert_eq!(record.step(), step);
        assert_eq!(record.world_tick(), Some(tick));
        assert_eq!(record.snapshot(), &11);
        assert_eq!(record.metrics()[0].key(), "work");
        assert_eq!(record.metrics()[0].value(), 1.5);

        let snapshot = record.into_snapshot();
        assert_eq!(snapshot.step(), step);
        assert_eq!(snapshot.world_tick(), Some(tick));
        assert_eq!(snapshot.snapshot(), &11);
        assert_eq!(snapshot.metrics().len(), 1);
    }
}
