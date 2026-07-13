use moirai::prelude::*;

#[test]
fn prelude_exports_system_authoring_vocabulary() {
    let _ = core::mem::size_of::<App>();
    let _ = core::mem::size_of::<World>();
    let _ = core::mem::size_of::<System>();
    let _ = core::mem::size_of::<StateError>();
    let _ = core::mem::size_of::<QuerySpec>();
    let _ = core::mem::size_of::<QueryIds<'_, '_>>();
    let _ = core::mem::size_of::<QueryEntities<'_, '_>>();
    let _ = core::mem::size_of::<EntityRef<'_>>();
    let _ = WorldTick::ZERO;
}
