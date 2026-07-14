//! Steady-state allocation contracts under `std` with a counting global allocator.
#![cfg(feature = "std")]

use std::alloc::{GlobalAlloc, Layout, System as StdAlloc};
use std::sync::atomic::{AtomicUsize, Ordering};

use moirai::component::ComponentOptions;
use moirai::diagnostics::{DiagnosticEvent, Observer};
use moirai::event::{EventOptions, EventReaderStart};
use moirai::query::{QueryPolicy, QuerySpec, QueryWindow};
use moirai::schedule::{stage, Condition, ScheduleBuilder, System, SystemSet};
use moirai::state::{apply, State};
use moirai::world::WorldBuilder;
use moirai::{AppBuilder, DenseEntityScratch};

static ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static ALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
static REALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static DEALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
static DEALLOCATED_BYTES: AtomicUsize = AtomicUsize::new(0);
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);
static PEAK_LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

struct TrackingAlloc;

fn record_live_growth(bytes: usize) {
    let live = LIVE_BYTES.fetch_add(bytes, Ordering::SeqCst) + bytes;
    let mut peak = PEAK_LIVE_BYTES.load(Ordering::SeqCst);
    while live > peak {
        match PEAK_LIVE_BYTES.compare_exchange(peak, live, Ordering::SeqCst, Ordering::SeqCst) {
            Ok(_) => break,
            Err(current) => peak = current,
        }
    }
}

unsafe impl GlobalAlloc for TrackingAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let pointer = StdAlloc.alloc(layout);
        if !pointer.is_null() {
            ALLOCATIONS.fetch_add(1, Ordering::SeqCst);
            ALLOCATED_BYTES.fetch_add(layout.size(), Ordering::SeqCst);
            record_live_growth(layout.size());
        }
        pointer
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        DEALLOCATIONS.fetch_add(1, Ordering::SeqCst);
        DEALLOCATED_BYTES.fetch_add(layout.size(), Ordering::SeqCst);
        LIVE_BYTES.fetch_sub(layout.size(), Ordering::SeqCst);
        StdAlloc.dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        let replacement = StdAlloc.realloc(ptr, layout, new_size);
        if !replacement.is_null() {
            REALLOCATIONS.fetch_add(1, Ordering::SeqCst);
            if new_size >= layout.size() {
                let growth = new_size - layout.size();
                ALLOCATED_BYTES.fetch_add(growth, Ordering::SeqCst);
                record_live_growth(growth);
            } else {
                let shrink = layout.size() - new_size;
                DEALLOCATED_BYTES.fetch_add(shrink, Ordering::SeqCst);
                LIVE_BYTES.fetch_sub(shrink, Ordering::SeqCst);
            }
        }
        replacement
    }
}

#[global_allocator]
static GLOBAL: TrackingAlloc = TrackingAlloc;

#[derive(Clone, Copy)]
struct Pos(i32);

#[derive(Clone, Copy)]
#[allow(dead_code)]
struct Vel(i32);

#[derive(Clone, Copy)]
struct TablePos(i32);

#[derive(Clone, Copy)]
struct Player;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Damage(u32);

struct NoopObserver;

impl Observer for NoopObserver {
    fn observe(&mut self, event: DiagnosticEvent<'_>) {
        match event {
            DiagnosticEvent::UpdateStart { .. }
            | DiagnosticEvent::UpdateFinish
            | DiagnosticEvent::StageStart { .. }
            | DiagnosticEvent::StageFinish { .. }
            | DiagnosticEvent::SystemStart { .. }
            | DiagnosticEvent::SystemFinish { .. }
            | DiagnosticEvent::FlushComplete => {}
            _ => {}
        }
    }
}

fn reset_tracking() {
    ALLOCATIONS.store(0, Ordering::SeqCst);
    ALLOCATED_BYTES.store(0, Ordering::SeqCst);
    REALLOCATIONS.store(0, Ordering::SeqCst);
    DEALLOCATIONS.store(0, Ordering::SeqCst);
    DEALLOCATED_BYTES.store(0, Ordering::SeqCst);
    PEAK_LIVE_BYTES.store(LIVE_BYTES.load(Ordering::SeqCst), Ordering::SeqCst);
}

#[test]
fn tracking_allocator_reports_allocation_lifecycle_metrics() {
    reset_tracking();
    let starting_live = LIVE_BYTES.load(Ordering::SeqCst);
    let mut values = Vec::with_capacity(1);
    values.push(1_u64);
    values.reserve_exact(1_024);
    std::hint::black_box(&values);

    assert!(ALLOCATIONS.load(Ordering::SeqCst) >= 1);
    assert!(REALLOCATIONS.load(Ordering::SeqCst) >= 1);
    assert!(ALLOCATED_BYTES.load(Ordering::SeqCst) > 0);
    assert!(PEAK_LIVE_BYTES.load(Ordering::SeqCst) > starting_live);

    drop(values);
    assert!(DEALLOCATIONS.load(Ordering::SeqCst) >= 1);
    assert!(DEALLOCATED_BYTES.load(Ordering::SeqCst) > 0);
    assert!(LIVE_BYTES.load(Ordering::SeqCst) <= PEAK_LIVE_BYTES.load(Ordering::SeqCst));
}

fn assert_no_repeated_steady_state_growth(mut steps: usize, mut step: impl FnMut()) {
    reset_tracking();
    step();
    let baseline = ALLOCATIONS.load(Ordering::SeqCst);
    while steps > 1 {
        step();
        assert_eq!(
            ALLOCATIONS.load(Ordering::SeqCst),
            baseline,
            "steady-state path must not accumulate allocations (bytes={})",
            ALLOCATED_BYTES.load(Ordering::SeqCst)
        );
        steps -= 1;
    }
}

fn assert_steady_state_zero_allocations(mut step: impl FnMut()) {
    reset_tracking();
    step();
    assert_eq!(
        ALLOCATIONS.load(Ordering::SeqCst),
        0,
        "steady-state path allocated {} bytes",
        ALLOCATED_BYTES.load(Ordering::SeqCst)
    );
    assert_eq!(REALLOCATIONS.load(Ordering::SeqCst), 0);
}

fn sparse_world() -> moirai::world::World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Pos>(ComponentOptions::sparse())
        .expect("register");
    builder
        .register_component::<Vel>(ComponentOptions::sparse())
        .expect("register");
    builder
        .register_component::<Player>(ComponentOptions::tag())
        .expect("tag");
    builder.build().expect("build")
}

fn warmed_sparse_world(count: usize) -> moirai::world::World {
    let mut world = sparse_world();
    for i in 0..count {
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(i as i32)).expect("pos");
        if i % 2 == 0 {
            world.insert(entity, Vel(i as i32)).expect("vel");
        }
    }
    world
}

fn table_world(count: usize) -> moirai::world::World {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<TablePos>(ComponentOptions::table())
        .expect("register");
    let mut world = builder.build().expect("build");
    for i in 0..count {
        let entity = world.spawn().expect("spawn");
        world.insert(entity, TablePos(i as i32)).expect("insert");
    }
    world
}

fn warmed_idle_app() -> moirai::App {
    let mut builder = AppBuilder::new();
    builder
        .add_system(System::new("noop", stage::UPDATE, |_world, _dt| {}))
        .expect("system");
    let mut app = builder.build().expect("app");
    for _ in 0..8 {
        app.update(1.0 / 60.0).expect("warmup");
    }
    app
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn app_update_steady_state_is_allocation_free() {
    let mut app = warmed_idle_app();
    assert_no_repeated_steady_state_growth(4, || {
        app.update(1.0 / 60.0).expect("update");
    });
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn diagnostics_observer_steady_state_is_allocation_free() {
    let mut builder = AppBuilder::new();
    builder.observer(NoopObserver);
    builder
        .add_system(System::new("noop", stage::UPDATE, |_world, _dt| {}))
        .expect("system");
    let mut app = builder.build().expect("app");
    for _ in 0..4 {
        app.update(1.0 / 60.0).expect("warmup");
    }
    assert_no_repeated_steady_state_growth(4, || {
        app.update(1.0 / 60.0).expect("update");
    });
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn table_query_steady_state_is_allocation_free() {
    let mut world = table_world(32);
    let spec = QuerySpec::new();
    let mut query = world
        .prepare_query1::<TablePos>(spec, QueryPolicy::Prepared)
        .expect("prepare");
    for _ in 0..4 {
        let _: Vec<_> = query
            .iter(&mut world, QueryWindow::All)
            .expect("warm")
            .map(|(_, value)| value.0)
            .collect();
    }
    assert_no_repeated_steady_state_growth(4, || {
        let count = query
            .iter(&mut world, QueryWindow::All)
            .expect("query")
            .count();
        assert_eq!(count, 32);
    });
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn query_result_cache_hit_is_allocation_free() {
    let mut world = warmed_sparse_world(16);
    let mut query = world
        .prepare_query1::<Pos>(QuerySpec::new(), QueryPolicy::Result)
        .expect("prepare result query");
    for _ in 0..4 {
        let _: Vec<_> = query
            .iter(&mut world, QueryWindow::All)
            .expect("warm")
            .map(|(_, p)| p.0)
            .collect();
    }
    assert_no_repeated_steady_state_growth(4, || {
        let count = query
            .iter(&mut world, QueryWindow::All)
            .expect("query")
            .count();
        assert_eq!(count, 16);
    });
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn command_flush_steady_state_is_allocation_free() {
    let mut world = warmed_sparse_world(4);
    for _ in 0..16 {
        let _ = world.flush().expect("flush");
    }
    assert_no_repeated_steady_state_growth(4, || {
        let report = world.flush().expect("flush");
        assert_eq!(report.commands_applied, 0);
    });
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn command_buffer_reuses_capacity_after_warmup() {
    let mut world = warmed_sparse_world(2);
    for _ in 0..32 {
        let reserved = world
            .commands()
            .expect("commands")
            .spawn()
            .expect("reserve");
        world.discard_commands().expect("discard");
        assert!(!world.is_alive(reserved));
    }
    assert_no_repeated_steady_state_growth(4, || {
        let reserved = world
            .commands()
            .expect("commands")
            .spawn()
            .expect("reserve");
        world.discard_commands().expect("discard");
        assert!(!world.is_alive(reserved));
    });
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn successful_command_flush_reuses_capacity_after_warmup() {
    let mut world = warmed_sparse_world(2);
    for _ in 0..32 {
        let reserved = world
            .commands()
            .expect("commands")
            .spawn()
            .expect("reserve");
        world
            .commands()
            .expect("commands")
            .despawn(reserved)
            .expect("despawn");
        world.flush().expect("flush");
    }
    assert_no_repeated_steady_state_growth(4, || {
        let reserved = world
            .commands()
            .expect("commands")
            .spawn()
            .expect("reserve");
        world
            .commands()
            .expect("commands")
            .despawn(reserved)
            .expect("despawn");
        let report = world.flush().expect("flush");
        assert_eq!(report.commands_applied, 2);
    });
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn event_send_read_steady_state_is_allocation_free() {
    let mut builder = WorldBuilder::new();
    builder
        .add_event::<Damage>(EventOptions::bounded(1).expect("bounded"))
        .expect("register");
    let mut world = builder.build().expect("build");
    let mut reader = world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("reader");
    for i in 0..8u32 {
        world.send(Damage(i)).expect("send");
        let _ = world.read_event(&mut reader).expect("read");
    }
    assert_no_repeated_steady_state_growth(4, || {
        world.send(Damage(99)).expect("send");
        let value = world.read_event(&mut reader).expect("read").map(|d| d.0);
        assert_eq!(value, Some(99));
    });
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn event_compact_steady_state_is_allocation_free() {
    let mut builder = AppBuilder::new();
    builder
        .world_builder()
        .add_event::<Damage>(EventOptions::frame(moirai::StageOperation::Update))
        .expect("register");
    builder
        .add_system(
            System::new("send", stage::UPDATE, |world, _dt| {
                world.send(Damage(1)).expect("send");
            })
            .emits::<Damage>(),
        )
        .expect("system");
    let mut app = builder.build().expect("app");
    for _ in 0..6 {
        app.update(1.0 / 60.0).expect("warmup");
    }
    assert_no_repeated_steady_state_growth(4, || {
        app.update(1.0 / 60.0).expect("update");
    });
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn event_pool_reuses_payload_after_warmup() {
    let mut builder = WorldBuilder::new();
    builder
        .add_event::<Damage>(EventOptions::bounded(1).expect("bounded"))
        .expect("register");
    let mut world = builder.build().expect("build");
    let mut reader = world
        .event_reader::<Damage>(EventReaderStart::OldestRetained)
        .expect("reader");
    for i in 0..32u32 {
        world.send(Damage(i)).expect("send");
        let _ = world.read_event(&mut reader).expect("read");
    }
    assert_no_repeated_steady_state_growth(4, || {
        world.send(Damage(100)).expect("send");
        let value = world.read_event(&mut reader).expect("read").map(|d| d.0);
        assert_eq!(value, Some(100));
    });
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn stage_flush_steady_state_is_allocation_free() {
    let mut app = warmed_idle_app();
    assert_no_repeated_steady_state_growth(4, || {
        app.update(1.0 / 60.0).expect("update");
    });
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn state_transition_steady_state_is_allocation_free() {
    let mut builder = AppBuilder::new();
    builder.insert_state(1u8);
    builder
        .add_system(apply::<u8>("apply", stage::UPDATE))
        .expect("apply");
    let mut app = builder.build().expect("app");
    for next in 2u8..=6 {
        app.world_mut()
            .resource_mut::<State<u8>>()
            .expect("mut")
            .expect("present")
            .request(next)
            .expect("request");
        app.update(1.0 / 60.0).expect("warmup");
    }
    app.world_mut()
        .resource_mut::<State<u8>>()
        .expect("mut")
        .expect("present")
        .request(7)
        .expect("request");
    assert_no_repeated_steady_state_growth(4, || {
        app.update(1.0 / 60.0).expect("update");
    });
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn run_if_and_set_steady_state_is_allocation_free() {
    let set = SystemSet::new("workers");
    let mut builder = AppBuilder::new();
    builder.register_set(set.clone()).expect("set");
    builder
        .add_system(
            System::new("work", stage::UPDATE, |_world, _dt| {})
                .in_set(&set)
                .run_if(Condition::always()),
        )
        .expect("system");
    let mut app = builder.build().expect("app");
    for _ in 0..6 {
        app.update(1.0 / 60.0).expect("warmup");
    }
    assert_no_repeated_steady_state_growth(4, || {
        app.update(1.0 / 60.0).expect("update");
    });
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn composite_conditions_are_allocation_free_after_construction() {
    let mut condition = Condition::always();
    for _ in 0..64 {
        condition = condition.and(Condition::always());
    }

    let mut builder = AppBuilder::new();
    builder
        .add_system(System::new("conditional", stage::UPDATE, |_world, _dt| {}).run_if(condition))
        .expect("system");
    let mut app = builder.build().expect("app");
    for _ in 0..4 {
        app.update(1.0 / 60.0).expect("warmup");
    }

    assert_no_repeated_steady_state_growth(4, || {
        app.update(1.0 / 60.0).expect("update");
    });
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn event_dispatch_steady_state_is_allocation_free() {
    let mut builder = AppBuilder::new();
    builder
        .world_builder()
        .add_event::<Damage>(EventOptions::bounded(1).expect("bounded"))
        .expect("register");
    builder
        .add_system(
            System::new("send", stage::UPDATE, |world, _dt| {
                world.send(Damage(1)).expect("send");
            })
            .emits::<Damage>(),
        )
        .expect("system");
    let mut app = builder.build().expect("app");
    for _ in 0..6 {
        app.update(1.0 / 60.0).expect("warmup");
    }
    assert_no_repeated_steady_state_growth(4, || {
        app.update(1.0 / 60.0).expect("update");
    });
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn component_event_dispatch_is_allocation_free() {
    let mut builder = AppBuilder::new();
    builder
        .world_builder()
        .register_component::<Pos>(ComponentOptions::sparse())
        .expect("register");
    builder
        .add_system(System::new("noop", stage::UPDATE, |_world, _dt| {}))
        .expect("system");
    let mut app = builder.build().expect("app");
    for _ in 0..4 {
        let entity = app.world_mut().spawn().expect("spawn");
        app.world_mut().insert(entity, Pos(1)).expect("insert");
        app.update(1.0 / 60.0).expect("warmup");
    }
    assert_no_repeated_steady_state_growth(4, || {
        app.update(1.0 / 60.0).expect("update");
    });
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn component_event_read_is_allocation_free() {
    let mut builder = WorldBuilder::new();
    builder
        .register_component::<Pos>(ComponentOptions::sparse())
        .expect("register");
    let mut world = builder.build().expect("build");
    let mut reader = world
        .on_add_reader::<Pos>(EventReaderStart::OldestRetained)
        .expect("reader");
    for _ in 0..8 {
        let entity = world.spawn().expect("spawn");
        world.insert(entity, Pos(1)).expect("insert");
        let _ = world.read_event(&mut reader).expect("read");
    }
    let entity = world.spawn().expect("spawn");
    world.insert(entity, Pos(2)).expect("queue event");
    assert!(world.read_event(&mut reader).expect("read").is_some());
    assert_no_repeated_steady_state_growth(4, || {
        assert!(world.read_event(&mut reader).expect("drain").is_none());
    });
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn membership_cache_hit_is_allocation_free() {
    let mut world = warmed_sparse_world(12);
    let mut query = world
        .prepare_query1::<Pos>(QuerySpec::new().without::<Vel>(), QueryPolicy::Membership)
        .expect("prepare membership query");
    for _ in 0..4 {
        let _: Vec<_> = query
            .iter(&mut world, QueryWindow::All)
            .expect("warm")
            .map(|(_, p)| p.0)
            .collect();
    }
    assert_no_repeated_steady_state_growth(4, || {
        let count = query
            .iter(&mut world, QueryWindow::All)
            .expect("query")
            .count();
        assert_eq!(count, 6);
    });
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn schedule_build_reuses_world_without_extra_topology_alloc_in_steady_query() {
    let mut world = warmed_sparse_world(8);
    let schedule = ScheduleBuilder::standard()
        .build(&mut world)
        .expect("schedule");
    drop(schedule);
    let mut query = world
        .prepare_query1::<Pos>(QuerySpec::new(), QueryPolicy::Prepared)
        .expect("prepare");
    assert_no_repeated_steady_state_growth(4, || {
        let count = query
            .iter(&mut world, QueryWindow::All)
            .expect("query")
            .count();
        assert_eq!(count, 8);
    });
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn prepared_query1_mutation_is_allocation_free_after_warmup() {
    for policy in [
        QueryPolicy::Prepared,
        QueryPolicy::Membership,
        QueryPolicy::DeltaMembership,
        QueryPolicy::Result,
    ] {
        let mut world = warmed_sparse_world(32);
        let mut query = world
            .prepare_query1::<Pos>(QuerySpec::new(), policy)
            .expect("prepare");
        for _ in 0..4 {
            query
                .for_each_mut(&mut world, QueryWindow::All, |_, position| {
                    position.0 = position.0.wrapping_add(1);
                    Ok(())
                })
                .expect("warm");
        }
        assert_steady_state_zero_allocations(|| {
            query
                .for_each_mut(&mut world, QueryWindow::All, |_, position| {
                    position.0 = position.0.wrapping_add(1);
                    Ok(())
                })
                .expect("mutate");
        });
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn prepared_query2_mut_mut_is_allocation_free_after_warmup() {
    for policy in [
        QueryPolicy::Prepared,
        QueryPolicy::Membership,
        QueryPolicy::DeltaMembership,
        QueryPolicy::Result,
    ] {
        let mut world = warmed_sparse_world(32);
        let mut query = world
            .prepare_query2::<Pos, Vel>(QuerySpec::new(), policy)
            .expect("prepare");
        for _ in 0..4 {
            query
                .for_each_mut_mut(&mut world, QueryWindow::All, |_, position, velocity| {
                    position.0 = position.0.wrapping_add(velocity.0);
                    velocity.0 = velocity.0.wrapping_neg();
                    Ok(())
                })
                .expect("warm");
        }
        assert_steady_state_zero_allocations(|| {
            query
                .for_each_mut_mut(&mut world, QueryWindow::All, |_, position, velocity| {
                    position.0 = position.0.wrapping_add(velocity.0);
                    velocity.0 = velocity.0.wrapping_neg();
                    Ok(())
                })
                .expect("mutate");
        });
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn prepared_query2_mut_read_is_allocation_free_after_warmup() {
    for policy in [
        QueryPolicy::Prepared,
        QueryPolicy::Membership,
        QueryPolicy::DeltaMembership,
        QueryPolicy::Result,
    ] {
        let mut world = warmed_sparse_world(32);
        let mut query = world
            .prepare_query2::<Pos, Vel>(QuerySpec::new(), policy)
            .expect("prepare");
        for _ in 0..4 {
            query
                .for_each_mut_read(&mut world, QueryWindow::All, |_, position, velocity| {
                    position.0 = position.0.wrapping_add(velocity.0);
                    Ok(())
                })
                .expect("warm");
        }
        assert_steady_state_zero_allocations(|| {
            query
                .for_each_mut_read(&mut world, QueryWindow::All, |_, position, velocity| {
                    position.0 = position.0.wrapping_add(velocity.0);
                    Ok(())
                })
                .expect("mutate");
        });
    }
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn dense_entity_scratch_reuses_storage_without_allocating() {
    let mut world = WorldBuilder::new().build().expect("world");
    let entities: Vec<_> = (0..32).map(|_| world.spawn().expect("spawn")).collect();
    let mut scratch = DenseEntityScratch::with_capacity(&world, entities.len());
    for (index, &entity) in entities.iter().enumerate() {
        scratch.insert(&world, entity, index).expect("warm insert");
    }
    scratch.clear();
    assert_steady_state_zero_allocations(|| {
        for (index, &entity) in entities.iter().enumerate() {
            scratch.insert(&world, entity, index).expect("insert");
        }
        for &entity in &entities {
            assert!(scratch.get(&world, entity).expect("get").is_some());
        }
        scratch.clear();
    });
}

#[derive(Clone, Copy)]
struct ScopedResource(u64);

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn resource_scopes_reuse_erased_box_without_allocation() {
    let mut builder = WorldBuilder::new();
    builder.insert_resource(ScopedResource(1));
    let mut world = builder.build().expect("world");
    world
        .resource_scope_ref::<ScopedResource, _>(|resource, _| {
            std::hint::black_box(resource.expect("present").0)
        })
        .expect("warm ref");
    world
        .resource_scope_mut::<ScopedResource, _>(|resource, _| {
            resource.expect("present").0 += 1;
        })
        .expect("warm mut");
    reset_tracking();
    let live_before = LIVE_BYTES.load(Ordering::SeqCst);
    world
        .resource_scope_ref::<ScopedResource, _>(|resource, _| {
            std::hint::black_box(resource.expect("present").0)
        })
        .expect("ref");
    world
        .resource_scope_mut::<ScopedResource, _>(|resource, _| {
            resource.expect("present").0 += 1;
        })
        .expect("mut");
    assert_eq!(ALLOCATIONS.load(Ordering::SeqCst), 0);
    assert_eq!(DEALLOCATIONS.load(Ordering::SeqCst), 0);
    assert_eq!(ALLOCATED_BYTES.load(Ordering::SeqCst), 0);
    assert_eq!(DEALLOCATED_BYTES.load(Ordering::SeqCst), 0);
    assert_eq!(REALLOCATIONS.load(Ordering::SeqCst), 0);
    assert_eq!(LIVE_BYTES.load(Ordering::SeqCst), live_before);
}

#[test]
#[cfg_attr(debug_assertions, ignore = "allocation contracts require --release")]
fn system_local_runtime_is_allocation_free_after_build() {
    let mut builder = AppBuilder::new();
    builder
        .add_system(System::with_local(
            "local-counter",
            stage::UPDATE,
            |_| Ok(0_u64),
            |_world, _dt, local| {
                *local = local.wrapping_add(1);
                std::hint::black_box(*local);
                Ok(())
            },
        ))
        .expect("system");
    let mut app = builder.build().expect("app");
    for _ in 0..4 {
        app.update(1.0 / 60.0).expect("warm");
    }
    assert_steady_state_zero_allocations(|| {
        app.update(1.0 / 60.0).expect("update");
    });
}
