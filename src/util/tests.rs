use super::FlatMap;

#[test]
fn thinarc_basic() {
    use super::ThinArc;
    let sarc1 = ThinArc::new(5i32);
    let sarc2 = sarc1.clone();
    assert!(sarc1.try_unwrap().is_err());
    assert_eq!(sarc2.try_unwrap().ok(), Some(5i32));
}

#[test]
fn flatmap_dedup() {
    use super::do_dedup;
    let testcases = [
        (vec![], [].as_slice()),
        (vec![(1, 0)], &[(1, 0)]),
        (vec![(1, 0), (2, 0)], &[(1, 0), (2, 0)]),
        (vec![(1, 0), (2, 0), (3, 0)], &[(1, 0), (2, 0), (3, 0)]),
        (vec![(1, 0), (2, 0), (2, 0)], &[(1, 0), (2, 0)]),
        (vec![(1, 0), (1, 0), (2, 0)], &[(1, 0), (2, 0)]),
        (vec![(1, 0), (2, 0), (2, 0), (3, 0)], &[(1, 0), (2, 0), (3, 0)]),
        (vec![(1, 0), (2, 0), (2, 1), (2, 2), (3, 0)], &[(1, 0), (2, 2), (3, 0)]),
    ];
    for (init, expected) in testcases {
        let result = do_dedup::<_, ()>(init);
        assert_eq!(&result, expected);
    }
    // Simple test to hopefully catch UAFs.
    let vec1 = do_dedup::<_, ()>(vec![
        (1, String::from("foo")),
        (1, String::from("bar")),
        (2, String::from("baz")),
    ]);
    let vec2 = do_dedup::<_, ()>(vec![
        (1, String::from("bar")),
        (2, String::from("foo")),
        (2, String::from("baz")),
    ]);
    assert_eq!(vec1, vec2);
}

#[test]
fn flatmap_guard_insert() {
    let mut map = FlatMap::<(u8, u8)>::from_vec(vec![(0, b'a'), (1, b'b'), (2, b'c')]);
    let mut guard = map.edit();
    for value in b'd'..=b'z' {
        guard.insert((value - b'a', value));
    }
    std::mem::drop(guard);
    assert_eq!(map.len(), 26);
    for expect in b'a'..=b'z' {
        let pair = map.get(&(expect - b'a')).expect("missing expected value {expect}");
        assert_eq!(pair.1, expect);
    }
}

#[test]
fn flatmap_guard_forget() {
    let mut map = FlatMap::<(u32, char)>::from_vec(vec![(1, 'a'), (2, 'b'), (3, 'c')]);
    let mut guard = map.edit();
    guard.insert((4, 'd'));
    // Remove swap-removes, leaving the the map unsorted after the element that was removed.
    guard.remove(&3).unwrap();
    std::mem::forget(guard);
    assert_eq!(map.len(), 2);
}
