//! # D03 — Attach transient host data to entities
//!
//! **Goal:** store scratch values by checked entity handle and remove stale entries.
//!
//! ```
//! use moirai::{DenseEntityScratch, WorldBuilder};
//!
//! let mut world = WorldBuilder::new().build().unwrap();
//! let entity = world.spawn().unwrap();
//! let mut scratch = DenseEntityScratch::new(&world);
//!
//! assert_eq!(scratch.insert(&world, entity, "visible").unwrap(), None);
//! assert_eq!(scratch.get(&world, entity).unwrap(), Some(&"visible"));
//! world.despawn(entity).unwrap();
//! assert_eq!(scratch.retain_live(&world).unwrap(), 1);
//! assert!(scratch.is_empty());
//! ```
//!
//! Scratch storage is host-owned and does not become an ECS component. It still binds
//! to one world and validates full generations so recycled slots cannot reuse stale data.
//!
//! **Next:** [`d04_fixed_point_values`](super::d04_fixed_point_values).
