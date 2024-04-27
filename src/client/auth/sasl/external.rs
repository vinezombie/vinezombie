use crate::{
    client::auth::{Sasl, SaslLogic},
    string::{Arg, NoNul, SecretBuf},
};

/// The [`EXTERNAL` SASL mechanism](https://datatracker.ietf.org/doc/html/rfc4422#appendix-A).
///
/// This is what is used when authentication occurs out-of-band,
/// such as when using TLS client certificate authentication.
///
/// The provided string, if non-empty, is an authzid.
#[derive(Clone, Debug, Default)]
#[cfg_attr(feature = "serde", derive(serde_derive::Serialize, serde_derive::Deserialize))]
pub struct External(#[cfg_attr(feature = "serde", serde(default))] pub NoNul<'static>);

struct ExternalLogic(NoNul<'static>);

static NAME: Arg = Arg::from_str("EXTERNAL");

impl SaslLogic for ExternalLogic {
    fn name(&self) -> Arg<'static> {
        NAME.clone()
    }
    fn reply<'a>(
        &'a mut self,
        input: &[u8],
        output: &mut SecretBuf,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync + 'static>> {
        if !input.is_empty() {
            return Err("non-empty server message".into());
        }
        output.push_slice(self.0.as_bytes());
        Ok(())
    }

    fn size_hint(&self) -> usize {
        self.0.len()
    }
}

impl Sasl for External {
    fn logic(&self) -> Vec<Box<dyn SaslLogic>> {
        vec![Box::new(ExternalLogic(self.0.clone()))]
    }
}
