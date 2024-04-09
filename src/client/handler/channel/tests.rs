/// Park, expecting the thread to be blocked for 100 to 2000 ms.
/// Panic if the time falls outside of this range.
fn timed_park() {
    let then = std::time::Instant::now();
    std::thread::park_timeout(std::time::Duration::from_secs(2));
    let now = std::time::Instant::now();
    let diff = now - then;
    if diff < std::time::Duration::from_millis(100) {
        panic!("probable non-block; parked for {}ms", diff.as_millis());
    }
    if diff >= std::time::Duration::from_secs(2) {
        panic!("probable deadlock; parked for {}ms", diff.as_millis());
    }
}
#[test]
fn parker_slow_unpark() {
    let (unparker, parker) = super::parker::new(());
    let current = std::thread::current();
    std::thread::spawn(move || {
        parker.park();
        current.unpark();
    });
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        unparker.unpark();
    });
    timed_park();
}
#[test]
fn parker_slow_park() {
    let (unparker, parker) = super::parker::new(());
    let current = std::thread::current();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        parker.park();
        current.unpark();
    });
    std::thread::spawn(move || {
        unparker.unpark();
    });
    timed_park();
}
#[test]
fn unparker_drop_slow_unpark() {
    let (unparker1, parker) = super::parker::new(());
    let unparker2 = unparker1.clone();
    let current = std::thread::current();
    std::thread::spawn(move || {
        parker.park();
        current.unpark();
    });
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        std::mem::drop(unparker1);
        std::mem::drop(unparker2);
    });
    timed_park();
}
#[test]
fn unparker_drop_slow_park() {
    let (unparker1, parker) = super::parker::new(());
    let unparker2 = unparker1.clone();
    let current = std::thread::current();
    std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(200));
        parker.park();
        current.unpark();
    });
    std::thread::spawn(move || {
        std::mem::drop(unparker1);
        std::mem::drop(unparker2);
    });
    timed_park();
}
#[test]
fn oneshot_slow_send() {
    let string = "foobar".to_owned();
    let (send, recv) = super::oneshot::channel();
    let (mut send, parker) = super::parker::new(Some(send));
    std::thread::spawn(move || {
        use super::Sender;
        std::thread::sleep(std::time::Duration::from_millis(200));
        send.send(string);
    });
    let string = recv.recv(&parker).expect("spurious failure in blocking recv");
    assert_eq!(string, "foobar");
}
#[test]
fn oneshot_slow_recv() {
    let string = "foobar".to_owned();
    let (send, recv) = super::oneshot::channel();
    let (mut send, parker) = super::parker::new(Some(send));
    std::thread::spawn(move || {
        use super::Sender;
        send.send(string);
    });
    std::thread::sleep(std::time::Duration::from_millis(200));
    let string = recv.recv(&parker).expect("spurious failure in blocking recv");
    assert_eq!(string, "foobar");
}
