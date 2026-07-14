//! # B01 — Declare system ordering
//!
//! **Goal:** make execution order explicit instead of depending on insertion order.
//!
//! ```
//! use moirai::{stage, AppBuilder, System};
//! use std::cell::RefCell;
//! use std::rc::Rc;
//!
//! let order = Rc::new(RefCell::new(Vec::new()));
//! let input_order = Rc::clone(&order);
//! let movement_order = Rc::clone(&order);
//!
//! let mut builder = AppBuilder::new();
//! builder.add_system(System::new("movement", stage::UPDATE, move |_, _| {
//!     movement_order.borrow_mut().push("movement");
//! })).unwrap();
//! builder.add_system(System::new("input", stage::UPDATE, move |_, _| {
//!     input_order.borrow_mut().push("input");
//! }).before("movement")).unwrap();
//!
//! let mut app = builder.build().expect("acyclic schedule");
//! app.update(0.0).unwrap();
//! assert_eq!(&*order.borrow(), &["input", "movement"]);
//! ```
//!
//! The builder resolves named edges with a stable topological sort and rejects
//! missing labels, cross-stage edges, and cycles before the app can run.
//!
//! **Next:** [`b02_state_transitions`](super::b02_state_transitions).
