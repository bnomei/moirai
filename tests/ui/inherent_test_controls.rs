fn main() {
    let mut world = moirai::WorldBuilder::new().build().unwrap();
    world.set_world_tick_for_test(7);
}
