//! # C04 — Emit side effects from query mutation
//!
//! **Goal:** mutate matching components and send a declared event safely.
//!
//! ```
//! use moirai::component::ComponentOptions;
//! use moirai::event::{EventOptions, EventReaderStart};
//! use moirai::query::{QueryPolicy, QuerySpec, QueryWindow};
//! use moirai::{stage, AppBuilder, System};
//!
//! struct Position(i32);
//! #[derive(Clone, Debug, PartialEq)]
//! struct Moved(i32);
//!
//! let mut builder = AppBuilder::new();
//! builder.world_builder().register_component::<Position>(ComponentOptions::sparse()).unwrap();
//! builder.world_builder().add_event::<Moved>(EventOptions::manual()).unwrap();
//! builder.add_system(System::new("seed", stage::STARTUP, |world, _| {
//!     let entity = world.commands().unwrap().spawn().unwrap();
//!     world.commands().unwrap().insert(entity, Position(1)).unwrap();
//! })).unwrap();
//! builder.add_system(System::new("move", stage::UPDATE, |world, _| {
//!     let mut query = world.prepare_query1::<Position>(QuerySpec::new(), QueryPolicy::Prepared).unwrap();
//!     query.for_each_mut_with_effects(world, QueryWindow::All, |_, position, effects| {
//!         position.0 += 1;
//!         effects.send(Moved(position.0))?;
//!         Ok(())
//!     }).unwrap();
//! }).emits::<Moved>()).unwrap();
//!
//! let mut app = builder.build().unwrap();
//! app.update(0.0).unwrap();
//! let mut reader = app.world_mut().event_reader::<Moved>(EventReaderStart::OldestRetained).unwrap();
//! assert_eq!(app.world_mut().read_event(&mut reader).unwrap(), Some(&Moved(2)));
//! ```
//!
//! `QueryEffects` keeps component borrows separate from the permitted event and command
//! surfaces. Schedule event-role declarations are checked before execution.
//!
//! **Next:** [`crate::examples::tier_d::d01_system_locals`].
