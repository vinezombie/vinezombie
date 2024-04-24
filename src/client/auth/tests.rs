use crate::{
    client::auth::{Clear, Sasl, Secret},
    string::{Bytes, NoNul},
};

#[test]
fn sasl_plain() {
    use super::sasl::Plain;
    let sasl = Plain::<Clear>::new(NoNul::from_str("foobar"), Secret::new(Bytes::from("12345")));
    let mut logic = sasl.logic();
    let reply = logic.reply(b"").expect("SASL auth should not fail");
    assert_eq!(reply, b"\0foobar\012345");
}

#[cfg(feature = "serde")]
mod serde {
    use crate::client::auth::{Clear, Secret};
    use crate::string::Line;

    #[test]
    fn de_clear() {
        let string = serde_json::Value::String("aHVudGVyMg==".to_owned());
        let clear: Secret<Line<'static>, Clear> =
            serde_json::from_value(string).expect("deserialization should not fail");
        assert_eq!(clear.as_bytes(), b"hunter2");
    }
}
