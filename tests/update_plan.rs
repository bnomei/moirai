use std::cell::RefCell;
use std::rc::Rc;

use moirai::{stage, AppBuilder, StageOperation, System};

#[test]
fn update_plan_runs_selected_stages_in_compiled_order() {
    let order = Rc::new(RefCell::new(Vec::new()));
    let input_order = Rc::clone(&order);
    let sim_order = Rc::clone(&order);
    let mut builder = AppBuilder::new();
    builder
        .schedule_builder()
        .add_stage("Input", StageOperation::Update)
        .expect("input stage");
    builder
        .schedule_builder()
        .add_stage("Sim", StageOperation::Update)
        .expect("sim stage");
    builder
        .add_system(System::new("input", "Input", move |_, _| {
            input_order.borrow_mut().push("input");
        }))
        .expect("input system");
    builder
        .add_system(System::new("sim", "Sim", move |_, _| {
            sim_order.borrow_mut().push("sim");
        }))
        .expect("sim system");
    let mut app = builder.build().expect("app");
    let input = app.schedule().stage_id("Input").expect("input id");
    let sim = app.schedule().stage_id("Sim").expect("sim id");
    let plan = app.schedule().update_plan([sim, input]).expect("plan");

    app.update_plan(0.0, &plan).expect("planned update");
    assert_eq!(&*order.borrow(), &["input", "sim"]);
}

#[test]
fn planned_update_skips_fixed_accumulation_when_fixed_stage_is_not_selected() {
    use core::time::Duration;
    use moirai::FixedConfig;

    let fixed_runs = Rc::new(RefCell::new(0_u32));
    let fixed_seen = Rc::clone(&fixed_runs);
    let mut builder = AppBuilder::new();
    builder.fixed(FixedConfig::new(Duration::from_millis(10)).expect("fixed"));
    builder
        .add_system(System::new("fixed", stage::FIXED_UPDATE, move |_, _| {
            *fixed_seen.borrow_mut() += 1;
        }))
        .expect("fixed system");
    let mut app = builder.build().expect("app");
    let update = app.schedule().stage_id(stage::UPDATE).expect("update id");
    let update_only = app.schedule().update_plan([update]).expect("plan");
    let fixed = app
        .schedule()
        .stage_id(stage::FIXED_UPDATE)
        .expect("fixed id");
    let fixed_only = app.schedule().update_plan([fixed]).expect("plan");

    app.update_plan(0.050, &update_only).expect("update only");
    app.update_plan(0.0, &fixed_only).expect("fixed only");
    assert_eq!(*fixed_runs.borrow(), 0);
}
