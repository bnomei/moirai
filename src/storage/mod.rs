//! Component storage engines: sparse sets and archetype tables.
//!
//! **Semantic map**
//! - [`sparse`]: entity-indexed sparse arrays with dense iteration and change ticks.
//! - [`erased`]: type-erased sparse facade for registry-driven lookup.
//! - [`archetype`]: signature-grouped entity rows and column migration.
//! - [`table`]: typed component columns within an archetype.

mod archetype;
mod erased;
mod sparse;
mod table;

pub(crate) use archetype::{table_column_factory, ArchetypeStorage, TableColumnFactory};
pub(crate) use erased::{SparseStore, TagSparseStorage, TypedSparseStorage};
