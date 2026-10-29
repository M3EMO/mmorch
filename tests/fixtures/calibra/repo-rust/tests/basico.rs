use calibra_fixture::almacen::Almacen;

#[test]
fn almacen_guarda_y_lee() {
    let mut almacen = Almacen::default();
    almacen.set("a", 1);
    assert!(almacen.get("a").is_some());
    assert_eq!(almacen.get("a"), Some(1));
}
