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
    pub fn new(key: impl Into<String>, value: f64) -> Self {
        Self {
            key: key.into(),
            value,
        }
    }

    pub fn key(&self) -> &str {
        &self.key
    }

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

    pub fn step(&self) -> StepIndex {
        self.step
    }

    pub fn world_tick(&self) -> Option<WorldTick> {
        self.world_tick
    }

    pub fn snapshot(&self) -> &S {
        &self.snapshot
    }

    pub fn metrics(&self) -> &[MetricSample] {
        &self.metrics
    }

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
    pub fn step(&self) -> StepIndex {
        self.step
    }

    pub fn world_tick(&self) -> Option<WorldTick> {
        self.world_tick
    }

    pub fn snapshot(&self) -> &S {
        &self.snapshot
    }

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
