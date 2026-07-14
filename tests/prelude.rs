use moirai::prelude::*;

#[test]
fn prelude_exports_system_authoring_vocabulary() {
    let _ = core::mem::size_of::<App>();
    let _ = core::mem::size_of::<World>();
    let _ = core::mem::size_of::<System>();
    let _ = core::mem::size_of::<StateError>();
    let _ = core::mem::size_of::<QuerySpec>();
    let _ = core::mem::size_of::<PreparedQuery1<()>>();
    let _ = core::mem::size_of::<PreparedQuery2<(), ()>>();
    let _ = core::mem::size_of::<QueryPolicy>();
    let _ = core::mem::size_of::<DenseEntityScratch<u8>>();
    let _ = core::mem::size_of::<EntityScratchError>();
    fn accepts_bundle(_bundle: impl Bundle) {}
    accepts_bundle((1u8,));
    let _ = WorldTick::ZERO;
}
