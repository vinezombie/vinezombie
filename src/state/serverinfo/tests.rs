use crate::{
    state::{
        serverinfo::{isupport::EXCEPTS, ISupportParser},
        Mode,
    },
    string::{Key, Word},
};

#[test]
fn excepts_default() {
    use super::ServerInfo;
    let mut si = ServerInfo::new();
    // EXCEPTS has a default value. Providing an empty string should use this default value instead.
    ISupportParser::global()
        .parse_and_update(&mut si, &Key::from_str("EXCEPTS"), Word::default())
        .expect("cannot update EXCEPTS with empty value");
    let excepts_value = si.get(&EXCEPTS).copied().expect("EXCEPTS value should exist");
    assert_eq!(excepts_value, Mode::new(b'e').unwrap());
}
