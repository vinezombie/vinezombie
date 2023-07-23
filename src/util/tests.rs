#[test]
fn thinarc_basic() {
    use super::ThinArc;
    let sarc1 = ThinArc::new(5i32);
    let sarc2 = sarc1.clone();
    assert!(sarc1.try_unwrap().is_err());
    assert_eq!(sarc2.try_unwrap().ok(), Some(5i32));
}
