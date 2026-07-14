//! # D01 — Keep persistent system-local state
//!
//! **Goal:** initialize a local value once and reuse it across updates.
//!
//! ```
//! use moirai::{stage, AppBuilder, System};
//! use std::cell::{Cell, RefCell};
//! use std::rc::Rc;
//!
//! let initializations = Rc::new(Cell::new(0));
//! let values = Rc::new(RefCell::new(Vec::new()));
//! let init_count = Rc::clone(&initializations);
//! let observed = Rc::clone(&values);
//!
//! let mut builder = AppBuilder::new();
//! builder.add_system(System::with_local(
//!     "counter",
//!     stage::UPDATE,
//!     move |_| { init_count.set(init_count.get() + 1); Ok(0_u32) },
//!     move |_, _, local| { *local += 1; observed.borrow_mut().push(*local); Ok(()) },
//! )).unwrap();
//! let mut app = builder.build().unwrap();
//! app.update(0.0).unwrap();
//! app.update(0.0).unwrap();
//!
//! assert_eq!(initializations.get(), 1);
//! assert_eq!(&*values.borrow(), &[1, 2]);
//! ```
//!
//! The initializer runs during schedule construction, when it can also prepare
//! owner-bound readers or queries. The resulting local lives with that system body.
//!
//! **Next:** [`d02_diagnostics`](super::d02_diagnostics).
