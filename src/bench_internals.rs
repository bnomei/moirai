//! Unstable measurement seams for repository benchmarks.
//!
//! This module exists only with the non-default `bench-internals` feature. It is
//! not a compatibility API and may change or disappear with benchmark needs.

use crate::query::{QueryError, QueryParams, QuerySpec};
use crate::world::World;

/// Resolves and executes the retained ad-hoc one-component path once.
pub fn adhoc_query1_count<T: 'static>(
    world: &mut World,
    spec: &QuerySpec,
) -> Result<usize, QueryError> {
    Ok(world.query::<T>(spec, QueryParams::new())?.count())
}

/// Resolves and executes the retained ad-hoc two-component path once.
pub fn adhoc_query2_count<A: 'static, B: 'static>(
    world: &mut World,
    spec: &QuerySpec,
) -> Result<usize, QueryError> {
    Ok(world.query2::<A, B>(spec, QueryParams::new())?.count())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::component::ComponentOptions;
    use crate::world::WorldBuilder;

    struct Position;
    struct Velocity;

    #[test]
    fn adhoc_count_seams_execute_both_query_shapes() {
        let mut builder = WorldBuilder::new();
        builder
            .register_component::<Position>(ComponentOptions::sparse())
            .expect("position");
        builder
            .register_component::<Velocity>(ComponentOptions::sparse())
            .expect("velocity");
        let mut world = builder.build().expect("world");
        let entity = world.spawn().expect("entity");
        world.insert(entity, Position).expect("position insert");
        world.insert(entity, Velocity).expect("velocity insert");

        let spec = QuerySpec::new();
        assert_eq!(adhoc_query1_count::<Position>(&mut world, &spec), Ok(1));
        assert_eq!(
            adhoc_query2_count::<Position, Velocity>(&mut world, &spec),
            Ok(1)
        );
    }
}
