use moirai::component::ComponentOptions;
use moirai::world::{WorldBuilder, WorldError};

#[derive(Clone, Copy, Debug, PartialEq)]
struct Health(i32);

#[test]
fn sparse_world_vertical_slice() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Health>(ComponentOptions::sparse())
        .expect("register health");
    let mut world = builder.build().expect("build world");

    let a = world.spawn().expect("spawn a");
    let b = world.spawn().expect("spawn b");
    assert!(world.insert(a, Health(10)).expect("insert a").is_none());
    assert!(world.insert(b, Health(20)).expect("insert b").is_none());
    assert_eq!(
        world.get::<Health>(a).expect("get a").map(|h| h.0),
        Some(10)
    );

    assert_eq!(
        world.insert(a, Health(15)).expect("replace a").map(|h| h.0),
        Some(10)
    );
    assert_eq!(world.len_sparse::<Health>().expect("len"), 2);

    world.despawn(a).expect("despawn a");
    assert!(!world.is_alive(a));
    assert_eq!(
        world.get::<Health>(a),
        Err(WorldError::StaleEntity { entity: a })
    );

    let c = world.spawn().expect("spawn c");
    assert!(world.is_alive(c));
    assert!(world.insert(c, Health(99)).expect("insert c").is_none());
    assert_eq!(
        world.get::<Health>(c).expect("get c").map(|h| h.0),
        Some(99)
    );
    assert_eq!(
        world.get::<Health>(a),
        Err(WorldError::StaleEntity { entity: a })
    );
    assert_eq!(
        world.get::<Health>(b).expect("get b").map(|h| h.0),
        Some(20)
    );
}

#[test]
fn mutation_on_stale_insert_does_not_change_storage() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Health>(ComponentOptions::sparse())
        .expect("register");
    let mut world = builder.build().expect("build");
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Health(1)).expect("seed component");
    world.despawn(entity).expect("despawn");
    let before = world.len_sparse::<Health>().expect("len");
    assert_eq!(
        world.insert(entity, Health(2)),
        Err(WorldError::StaleEntity { entity })
    );
    assert_eq!(world.len_sparse::<Health>().expect("len after"), before);
}
