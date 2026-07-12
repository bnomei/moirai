//! Platform-neutral diagnostic observer contracts.

/// Synchronous diagnostic observer invoked by `App` at execution boundaries.
pub trait Observer {
    fn observe(&mut self, event: DiagnosticEvent<'_>);
}

/// Non-exhaustive diagnostic event vocabulary.
#[non_exhaustive]
pub enum DiagnosticEvent<'a> {
    UpdateStart {
        delta_seconds: f32,
    },
    UpdateFinish,
    RenderStart {
        delta_seconds: f32,
    },
    RenderFinish,
    StageStart {
        name: &'a str,
    },
    StageFinish {
        name: &'a str,
    },
    SystemStart {
        name: &'a str,
    },
    SystemFinish {
        name: &'a str,
    },
    FlushComplete,
    FixedDebtDropped {
        steps: u128,
    },
    Fault {
        stage: Option<&'a str>,
        system: Option<&'a str>,
    },
}
