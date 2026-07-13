use core::fmt;

use moirai::event::{EventOptions, EventReader, EventReaderStart};
use moirai::world::{World, WorldBuilder};
use moirai::{App, AppBuilder, StageOperation};

const BOUNDED_CAPACITIES: [usize; 5] = [1, 4, 16, 256, 4_096];

#[allow(dead_code)]
#[derive(Clone)]
struct Payload<const N: usize>([u8; N]);

#[derive(Clone, Copy)]
struct ReaderEvent;

#[derive(Clone, Copy)]
struct RegistryEvent<const N: usize>;

macro_rules! for_each_registry_event {
    ($callback:ident, $($arg:ident),+ $(,)?) => {
        $callback!(0, $($arg),+); $callback!(1, $($arg),+);
        $callback!(2, $($arg),+); $callback!(3, $($arg),+);
        $callback!(4, $($arg),+); $callback!(5, $($arg),+);
        $callback!(6, $($arg),+); $callback!(7, $($arg),+);
        $callback!(8, $($arg),+); $callback!(9, $($arg),+);
        $callback!(10, $($arg),+); $callback!(11, $($arg),+);
        $callback!(12, $($arg),+); $callback!(13, $($arg),+);
        $callback!(14, $($arg),+); $callback!(15, $($arg),+);
        $callback!(16, $($arg),+); $callback!(17, $($arg),+);
        $callback!(18, $($arg),+); $callback!(19, $($arg),+);
        $callback!(20, $($arg),+); $callback!(21, $($arg),+);
        $callback!(22, $($arg),+); $callback!(23, $($arg),+);
        $callback!(24, $($arg),+); $callback!(25, $($arg),+);
        $callback!(26, $($arg),+); $callback!(27, $($arg),+);
        $callback!(28, $($arg),+); $callback!(29, $($arg),+);
        $callback!(30, $($arg),+); $callback!(31, $($arg),+);
        $callback!(32, $($arg),+); $callback!(33, $($arg),+);
        $callback!(34, $($arg),+); $callback!(35, $($arg),+);
        $callback!(36, $($arg),+); $callback!(37, $($arg),+);
        $callback!(38, $($arg),+); $callback!(39, $($arg),+);
        $callback!(40, $($arg),+); $callback!(41, $($arg),+);
        $callback!(42, $($arg),+); $callback!(43, $($arg),+);
        $callback!(44, $($arg),+); $callback!(45, $($arg),+);
        $callback!(46, $($arg),+); $callback!(47, $($arg),+);
        $callback!(48, $($arg),+); $callback!(49, $($arg),+);
        $callback!(50, $($arg),+); $callback!(51, $($arg),+);
        $callback!(52, $($arg),+); $callback!(53, $($arg),+);
        $callback!(54, $($arg),+); $callback!(55, $($arg),+);
        $callback!(56, $($arg),+); $callback!(57, $($arg),+);
        $callback!(58, $($arg),+); $callback!(59, $($arg),+);
        $callback!(60, $($arg),+); $callback!(61, $($arg),+);
        $callback!(62, $($arg),+); $callback!(63, $($arg),+);
        $callback!(64, $($arg),+); $callback!(65, $($arg),+);
        $callback!(66, $($arg),+); $callback!(67, $($arg),+);
        $callback!(68, $($arg),+); $callback!(69, $($arg),+);
        $callback!(70, $($arg),+); $callback!(71, $($arg),+);
        $callback!(72, $($arg),+); $callback!(73, $($arg),+);
        $callback!(74, $($arg),+); $callback!(75, $($arg),+);
        $callback!(76, $($arg),+); $callback!(77, $($arg),+);
        $callback!(78, $($arg),+); $callback!(79, $($arg),+);
        $callback!(80, $($arg),+); $callback!(81, $($arg),+);
        $callback!(82, $($arg),+); $callback!(83, $($arg),+);
        $callback!(84, $($arg),+); $callback!(85, $($arg),+);
        $callback!(86, $($arg),+); $callback!(87, $($arg),+);
        $callback!(88, $($arg),+); $callback!(89, $($arg),+);
        $callback!(90, $($arg),+); $callback!(91, $($arg),+);
        $callback!(92, $($arg),+); $callback!(93, $($arg),+);
        $callback!(94, $($arg),+); $callback!(95, $($arg),+);
        $callback!(96, $($arg),+); $callback!(97, $($arg),+);
        $callback!(98, $($arg),+); $callback!(99, $($arg),+);
        $callback!(100, $($arg),+); $callback!(101, $($arg),+);
        $callback!(102, $($arg),+); $callback!(103, $($arg),+);
        $callback!(104, $($arg),+); $callback!(105, $($arg),+);
        $callback!(106, $($arg),+); $callback!(107, $($arg),+);
        $callback!(108, $($arg),+); $callback!(109, $($arg),+);
        $callback!(110, $($arg),+); $callback!(111, $($arg),+);
        $callback!(112, $($arg),+); $callback!(113, $($arg),+);
        $callback!(114, $($arg),+); $callback!(115, $($arg),+);
        $callback!(116, $($arg),+); $callback!(117, $($arg),+);
        $callback!(118, $($arg),+); $callback!(119, $($arg),+);
        $callback!(120, $($arg),+); $callback!(121, $($arg),+);
        $callback!(122, $($arg),+); $callback!(123, $($arg),+);
        $callback!(124, $($arg),+); $callback!(125, $($arg),+);
        $callback!(126, $($arg),+); $callback!(127, $($arg),+);
        $callback!(128, $($arg),+); $callback!(129, $($arg),+);
        $callback!(130, $($arg),+); $callback!(131, $($arg),+);
        $callback!(132, $($arg),+); $callback!(133, $($arg),+);
        $callback!(134, $($arg),+); $callback!(135, $($arg),+);
        $callback!(136, $($arg),+); $callback!(137, $($arg),+);
        $callback!(138, $($arg),+); $callback!(139, $($arg),+);
        $callback!(140, $($arg),+); $callback!(141, $($arg),+);
        $callback!(142, $($arg),+); $callback!(143, $($arg),+);
        $callback!(144, $($arg),+); $callback!(145, $($arg),+);
        $callback!(146, $($arg),+); $callback!(147, $($arg),+);
        $callback!(148, $($arg),+); $callback!(149, $($arg),+);
        $callback!(150, $($arg),+); $callback!(151, $($arg),+);
        $callback!(152, $($arg),+); $callback!(153, $($arg),+);
        $callback!(154, $($arg),+); $callback!(155, $($arg),+);
        $callback!(156, $($arg),+); $callback!(157, $($arg),+);
        $callback!(158, $($arg),+); $callback!(159, $($arg),+);
        $callback!(160, $($arg),+); $callback!(161, $($arg),+);
        $callback!(162, $($arg),+); $callback!(163, $($arg),+);
        $callback!(164, $($arg),+); $callback!(165, $($arg),+);
        $callback!(166, $($arg),+); $callback!(167, $($arg),+);
        $callback!(168, $($arg),+); $callback!(169, $($arg),+);
        $callback!(170, $($arg),+); $callback!(171, $($arg),+);
        $callback!(172, $($arg),+); $callback!(173, $($arg),+);
        $callback!(174, $($arg),+); $callback!(175, $($arg),+);
        $callback!(176, $($arg),+); $callback!(177, $($arg),+);
        $callback!(178, $($arg),+); $callback!(179, $($arg),+);
        $callback!(180, $($arg),+); $callback!(181, $($arg),+);
        $callback!(182, $($arg),+); $callback!(183, $($arg),+);
        $callback!(184, $($arg),+); $callback!(185, $($arg),+);
        $callback!(186, $($arg),+); $callback!(187, $($arg),+);
        $callback!(188, $($arg),+); $callback!(189, $($arg),+);
        $callback!(190, $($arg),+); $callback!(191, $($arg),+);
        $callback!(192, $($arg),+); $callback!(193, $($arg),+);
        $callback!(194, $($arg),+); $callback!(195, $($arg),+);
        $callback!(196, $($arg),+); $callback!(197, $($arg),+);
        $callback!(198, $($arg),+); $callback!(199, $($arg),+);
        $callback!(200, $($arg),+); $callback!(201, $($arg),+);
        $callback!(202, $($arg),+); $callback!(203, $($arg),+);
        $callback!(204, $($arg),+); $callback!(205, $($arg),+);
        $callback!(206, $($arg),+); $callback!(207, $($arg),+);
        $callback!(208, $($arg),+); $callback!(209, $($arg),+);
        $callback!(210, $($arg),+); $callback!(211, $($arg),+);
        $callback!(212, $($arg),+); $callback!(213, $($arg),+);
        $callback!(214, $($arg),+); $callback!(215, $($arg),+);
        $callback!(216, $($arg),+); $callback!(217, $($arg),+);
        $callback!(218, $($arg),+); $callback!(219, $($arg),+);
        $callback!(220, $($arg),+); $callback!(221, $($arg),+);
        $callback!(222, $($arg),+); $callback!(223, $($arg),+);
        $callback!(224, $($arg),+); $callback!(225, $($arg),+);
        $callback!(226, $($arg),+); $callback!(227, $($arg),+);
        $callback!(228, $($arg),+); $callback!(229, $($arg),+);
        $callback!(230, $($arg),+); $callback!(231, $($arg),+);
        $callback!(232, $($arg),+); $callback!(233, $($arg),+);
        $callback!(234, $($arg),+); $callback!(235, $($arg),+);
        $callback!(236, $($arg),+); $callback!(237, $($arg),+);
        $callback!(238, $($arg),+); $callback!(239, $($arg),+);
        $callback!(240, $($arg),+); $callback!(241, $($arg),+);
        $callback!(242, $($arg),+); $callback!(243, $($arg),+);
        $callback!(244, $($arg),+); $callback!(245, $($arg),+);
        $callback!(246, $($arg),+); $callback!(247, $($arg),+);
        $callback!(248, $($arg),+); $callback!(249, $($arg),+);
        $callback!(250, $($arg),+); $callback!(251, $($arg),+);
        $callback!(252, $($arg),+); $callback!(253, $($arg),+);
        $callback!(254, $($arg),+); $callback!(255, $($arg),+);
    };
}

fn bounded_world<const N: usize>(capacity: usize) -> World {
    let mut builder = WorldBuilder::new();
    builder
        .add_event::<Payload<N>>(EventOptions::bounded(capacity).expect("bounded retention"))
        .expect("register bounded event");
    let mut world = builder.build().expect("build bounded event world");
    // One extra send primes the recycled-payload pool, so the timed send
    // measures retention movement instead of first-use allocation.
    for _ in 0..=capacity {
        world.send(Payload([1; N])).expect("prefill channel");
    }
    world
}

fn bench_bounded_send<const N: usize>(bencher: divan::Bencher<'_, '_>, capacity: usize) {
    bencher
        .with_inputs(|| bounded_world::<N>(capacity))
        .bench_local_refs(|world| {
            world.send(Payload([2; N])).expect("send bounded event");
            divan::black_box(world);
        });
}

#[divan::bench(args = BOUNDED_CAPACITIES)]
fn bounded_send_8_bytes(bencher: divan::Bencher<'_, '_>, capacity: usize) {
    bench_bounded_send::<8>(bencher, capacity);
}

#[divan::bench(args = BOUNDED_CAPACITIES)]
fn bounded_send_64_bytes(bencher: divan::Bencher<'_, '_>, capacity: usize) {
    bench_bounded_send::<64>(bencher, capacity);
}

#[divan::bench(args = BOUNDED_CAPACITIES)]
fn bounded_send_1024_bytes(bencher: divan::Bencher<'_, '_>, capacity: usize) {
    bench_bounded_send::<1_024>(bencher, capacity);
}

struct SendReadInput<const N: usize> {
    world: World,
    reader: EventReader<Payload<N>>,
}

fn send_read_input<const N: usize>(capacity: usize) -> SendReadInput<N> {
    let mut world = bounded_world::<N>(capacity);
    let reader = world
        .event_reader::<Payload<N>>(EventReaderStart::FromNow)
        .expect("create current reader");
    SendReadInput { world, reader }
}

fn lagging_reader_input<const N: usize>(capacity: usize) -> SendReadInput<N> {
    let mut builder = WorldBuilder::new();
    builder
        .add_event::<Payload<N>>(EventOptions::bounded(capacity).expect("bounded retention"))
        .expect("register bounded event");
    let mut world = builder.build().expect("build bounded event world");
    let reader = world
        .event_reader::<Payload<N>>(EventReaderStart::OldestRetained)
        .expect("create lagging reader");
    for _ in 0..=capacity {
        world.send(Payload([1; N])).expect("prefill channel");
    }
    SendReadInput { world, reader }
}

#[divan::bench(args = BOUNDED_CAPACITIES)]
fn bounded_send_then_current_read(bencher: divan::Bencher<'_, '_>, capacity: usize) {
    bencher
        .with_inputs(|| send_read_input::<64>(capacity))
        .bench_local_refs(|input| {
            input.world.send(Payload([3; 64])).expect("send event");
            let value = input
                .world
                .read_event(&mut input.reader)
                .expect("read current event")
                .expect("event present");
            divan::black_box(value);
        });
}

#[divan::bench(args = BOUNDED_CAPACITIES)]
fn bounded_send_with_lagging_reader(bencher: divan::Bencher<'_, '_>, capacity: usize) {
    bencher
        .with_inputs(|| lagging_reader_input::<64>(capacity))
        .bench_local_refs(|input| {
            input.world.send(Payload([4; 64])).expect("send event");
            divan::black_box(&input.reader);
        });
}

#[derive(Clone, Copy)]
struct ReaderCase {
    readers: usize,
    sends: usize,
}

impl fmt::Display for ReaderCase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}_readers_{}_sends", self.readers, self.sends)
    }
}

const READER_CASES: [ReaderCase; 15] = {
    let mut cases = [ReaderCase {
        readers: 0,
        sends: 1,
    }; 15];
    let readers = [0, 1, 8, 64, 1_024];
    let sends = [1, 64, 4_096];
    let mut i = 0;
    let mut r = 0;
    while r < readers.len() {
        let mut s = 0;
        while s < sends.len() {
            cases[i] = ReaderCase {
                readers: readers[r],
                sends: sends[s],
            };
            i += 1;
            s += 1;
        }
        r += 1;
    }
    cases
};

struct ReaderInput {
    world: World,
    live_readers: Vec<EventReader<ReaderEvent>>,
}

fn reader_input(reader_count: usize, keep_count: usize) -> ReaderInput {
    let mut builder = WorldBuilder::new();
    builder
        .add_event::<ReaderEvent>(EventOptions::bounded(1).expect("bounded retention"))
        .expect("register reader event");
    let mut world = builder.build().expect("build reader world");
    world.send(ReaderEvent).expect("prime event channel");
    world.send(ReaderEvent).expect("prime recycled payload");
    let mut readers = Vec::with_capacity(reader_count);
    for _ in 0..reader_count {
        readers.push(
            world
                .event_reader::<ReaderEvent>(EventReaderStart::FromNow)
                .expect("create reader"),
        );
    }
    readers.truncate(keep_count.min(reader_count));
    ReaderInput {
        world,
        live_readers: readers,
    }
}

fn send_reader_batch(input: &mut ReaderInput, sends: usize) {
    for _ in 0..sends {
        input.world.send(ReaderEvent).expect("send reader event");
    }
    divan::black_box(&input.live_readers);
}

#[divan::bench(args = READER_CASES)]
fn reader_send_batch_all_live(bencher: divan::Bencher<'_, '_>, case: ReaderCase) {
    bencher
        .with_inputs(|| reader_input(case.readers, case.readers))
        .bench_local_refs(|input| send_reader_batch(input, case.sends));
}

#[divan::bench(args = READER_CASES)]
fn reader_send_batch_90_percent_dropped(bencher: divan::Bencher<'_, '_>, case: ReaderCase) {
    bencher
        .with_inputs(|| reader_input(case.readers, case.readers / 10))
        .bench_local_refs(|input| send_reader_batch(input, case.sends));
}

#[divan::bench(args = [1, 64, 4_096])]
fn reader_create_drop_churn(bencher: divan::Bencher<'_, '_>, churn: usize) {
    bencher
        .with_inputs(|| reader_input(0, 0))
        .bench_local_refs(|input| {
            for _ in 0..churn {
                let reader = input
                    .world
                    .event_reader::<ReaderEvent>(EventReaderStart::FromNow)
                    .expect("create churn reader");
                drop(reader);
            }
            input.world.send(ReaderEvent).expect("send after churn");
            divan::black_box(&input.world);
        });
}

#[derive(Clone, Copy)]
struct RegistryCase {
    entries: usize,
    target: usize,
    position: &'static str,
}

impl fmt::Display for RegistryCase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}_entries_{}", self.entries, self.position)
    }
}

const REGISTRY_CASES: [RegistryCase; 13] = [
    RegistryCase {
        entries: 1,
        target: 0,
        position: "only",
    },
    RegistryCase {
        entries: 4,
        target: 0,
        position: "first",
    },
    RegistryCase {
        entries: 4,
        target: 2,
        position: "middle",
    },
    RegistryCase {
        entries: 4,
        target: 3,
        position: "last",
    },
    RegistryCase {
        entries: 16,
        target: 0,
        position: "first",
    },
    RegistryCase {
        entries: 16,
        target: 8,
        position: "middle",
    },
    RegistryCase {
        entries: 16,
        target: 15,
        position: "last",
    },
    RegistryCase {
        entries: 64,
        target: 0,
        position: "first",
    },
    RegistryCase {
        entries: 64,
        target: 32,
        position: "middle",
    },
    RegistryCase {
        entries: 64,
        target: 63,
        position: "last",
    },
    RegistryCase {
        entries: 256,
        target: 0,
        position: "first",
    },
    RegistryCase {
        entries: 256,
        target: 128,
        position: "middle",
    },
    RegistryCase {
        entries: 256,
        target: 255,
        position: "last",
    },
];

macro_rules! register_if_in_range {
    ($index:literal, $builder:ident, $entries:ident) => {
        if $index < $entries {
            $builder
                .add_event::<RegistryEvent<$index>>(
                    EventOptions::bounded(1).expect("bounded registry event"),
                )
                .expect("register indexed event");
        }
    };
}

fn registry_world(entries: usize) -> World {
    let mut builder = WorldBuilder::new();
    for_each_registry_event!(register_if_in_range, builder, entries);
    builder.build().expect("build registry world")
}

fn send_registry_target(world: &mut World, target: usize) {
    macro_rules! send_target {
        ($index:literal) => {
            world
                .send(RegistryEvent::<$index>)
                .expect("send registry target")
        };
    }
    match target {
        0 => send_target!(0),
        2 => send_target!(2),
        3 => send_target!(3),
        8 => send_target!(8),
        15 => send_target!(15),
        32 => send_target!(32),
        63 => send_target!(63),
        128 => send_target!(128),
        255 => send_target!(255),
        _ => unreachable!("unsupported registry target"),
    }
}

#[divan::bench(args = REGISTRY_CASES)]
fn event_registry_send(bencher: divan::Bencher<'_, '_>, case: RegistryCase) {
    bencher
        .with_inputs(|| {
            let mut world = registry_world(case.entries);
            send_registry_target(&mut world, case.target);
            send_registry_target(&mut world, case.target);
            world
        })
        .bench_local_refs(|world| {
            send_registry_target(world, case.target);
            divan::black_box(world);
        });
}

#[divan::bench(args = [1, 4, 16, 64, 256])]
fn event_registry_registration_including_build(bencher: divan::Bencher<'_, '_>, entries: usize) {
    bencher.bench_local(|| divan::black_box(registry_world(entries)));
}

#[derive(Clone, Copy)]
struct FrameCase {
    total: usize,
    frame: usize,
    active: usize,
}

impl fmt::Display for FrameCase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}_channels_{}_frame_{}_active",
            self.total, self.frame, self.active
        )
    }
}

const FRAME_CASES: [FrameCase; 19] = [
    FrameCase {
        total: 0,
        frame: 0,
        active: 0,
    },
    FrameCase {
        total: 8,
        frame: 0,
        active: 0,
    },
    FrameCase {
        total: 8,
        frame: 2,
        active: 0,
    },
    FrameCase {
        total: 8,
        frame: 2,
        active: 1,
    },
    FrameCase {
        total: 8,
        frame: 2,
        active: 2,
    },
    FrameCase {
        total: 8,
        frame: 8,
        active: 0,
    },
    FrameCase {
        total: 8,
        frame: 8,
        active: 8,
    },
    FrameCase {
        total: 64,
        frame: 0,
        active: 0,
    },
    FrameCase {
        total: 64,
        frame: 16,
        active: 0,
    },
    FrameCase {
        total: 64,
        frame: 16,
        active: 1,
    },
    FrameCase {
        total: 64,
        frame: 16,
        active: 16,
    },
    FrameCase {
        total: 64,
        frame: 64,
        active: 0,
    },
    FrameCase {
        total: 64,
        frame: 64,
        active: 64,
    },
    FrameCase {
        total: 256,
        frame: 0,
        active: 0,
    },
    FrameCase {
        total: 256,
        frame: 64,
        active: 0,
    },
    FrameCase {
        total: 256,
        frame: 64,
        active: 1,
    },
    FrameCase {
        total: 256,
        frame: 64,
        active: 64,
    },
    FrameCase {
        total: 256,
        frame: 256,
        active: 0,
    },
    FrameCase {
        total: 256,
        frame: 256,
        active: 256,
    },
];

macro_rules! register_frame_case_event {
    ($index:literal, $builder:ident, $total:ident, $frame:ident, $operation:ident) => {
        if $index < $total {
            let options = if $index < $frame {
                EventOptions::frame($operation)
            } else {
                EventOptions::manual()
            };
            $builder
                .world_builder()
                .add_event::<RegistryEvent<$index>>(options)
                .expect("register frame benchmark event");
        }
    };
}

macro_rules! send_if_active {
    ($index:literal, $world:ident, $active:ident) => {
        if $index < $active {
            $world
                .send(RegistryEvent::<$index>)
                .expect("send active frame event");
        }
    };
}

fn frame_app(case: FrameCase, operation: StageOperation) -> App {
    let total = case.total;
    let frame = case.frame;
    let active = case.active;
    let mut builder = AppBuilder::new();
    for_each_registry_event!(register_frame_case_event, builder, total, frame, operation);
    let mut app = builder.build().expect("build frame benchmark app");
    let world = app.world_mut();
    for_each_registry_event!(send_if_active, world, active);
    app
}

#[divan::bench(args = FRAME_CASES)]
fn frame_clear_update(bencher: divan::Bencher<'_, '_>, case: FrameCase) {
    bencher
        .with_inputs(|| frame_app(case, StageOperation::Update))
        .bench_local_refs(|app| {
            app.update(0.0).expect("update boundary");
            divan::black_box(app);
        });
}

#[divan::bench(args = FRAME_CASES)]
fn frame_clear_render(bencher: divan::Bencher<'_, '_>, case: FrameCase) {
    bencher
        .with_inputs(|| frame_app(case, StageOperation::Render))
        .bench_local_refs(|app| {
            app.render(0.0).expect("render boundary");
            divan::black_box(app);
        });
}

fn main() {
    divan::main();
}
