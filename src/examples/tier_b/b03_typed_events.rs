//! # B03 — Broadcast typed events
//!
//! **Goal:** send events to an independent reader in insertion order.
//!
//! ```
//! use moirai::event::{EventOptions, EventReaderStart};
//! use moirai::world::WorldBuilder;
//!
//! #[derive(Clone, Debug, PartialEq)]
//! struct Damage(u16);
//!
//! let mut builder = WorldBuilder::new();
//! builder.add_event::<Damage>(EventOptions::manual()).unwrap();
//! let mut world = builder.build().unwrap();
//! let mut reader = world.event_reader::<Damage>(EventReaderStart::OldestRetained).unwrap();
//!
//! world.send(Damage(2)).unwrap();
//! world.send(Damage(5)).unwrap();
//!
//! assert_eq!(world.read_event(&mut reader).unwrap(), Some(&Damage(2)));
//! assert_eq!(world.read_event(&mut reader).unwrap(), Some(&Damage(5)));
//! assert_eq!(world.read_event(&mut reader).unwrap(), None);
//! ```
//!
//! Event registration fixes retention and payload type. Each reader owns its cursor,
//! while cloned payloads let multiple readers observe the same broadcast safely.
//!
//! **Next:** [`b04_fixed_timestep`](super::b04_fixed_timestep).
