fn main() {
    let app = moirai::AppBuilder::new().build().unwrap();
    let id = app.schedule().stage_id(moirai::stage::UPDATE).unwrap();
    let _raw = id.index();
}
