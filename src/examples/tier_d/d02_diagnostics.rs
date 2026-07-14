//! # D02 — Observe execution without platform coupling
//!
//! **Goal:** count completed systems through the diagnostic observer contract.
//!
//! ```
//! use moirai::diagnostics::{DiagnosticEvent, Observer};
//! use moirai::{stage, AppBuilder, System};
//! use std::cell::Cell;
//! use std::rc::Rc;
//!
//! struct CounterObserver(Rc<Cell<u32>>);
//! impl Observer for CounterObserver {
//!     fn observe(&mut self, event: DiagnosticEvent<'_>) {
//!         if matches!(event, DiagnosticEvent::SystemFinish { .. }) {
//!             self.0.set(self.0.get() + 1);
//!         }
//!     }
//! }
//!
//! let completed = Rc::new(Cell::new(0));
//! let mut builder = AppBuilder::new();
//! builder.observer(CounterObserver(Rc::clone(&completed)));
//! builder.add_system(System::new("work", stage::UPDATE, |_, _| {})).unwrap();
//! let mut app = builder.build().unwrap();
//! app.update(0.0).unwrap();
//!
//! assert_eq!(completed.get(), 1);
//! ```
//!
//! `Observer` is synchronous and receives a non-exhaustive, platform-neutral event
//! vocabulary. Hosts decide whether those events become logs, counters, or traces.
//!
//! **Next:** [`d03_dense_entity_scratch`](super::d03_dense_entity_scratch).
