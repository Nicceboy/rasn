/// Options for configuring the [`Encoder`][super::Encoder].
#[derive(Clone, Copy, Debug)]
#[allow(dead_code)]
pub struct EncoderOptions {
    pub(crate) encoding_rules: EncodingRules,
}

#[allow(dead_code)]
impl EncoderOptions {
    // Return the default configuration for COER.
    // We reserve the possibility to use OER in the future by using the rules.
    pub const fn coer() -> Self {
        Self {
            encoding_rules: EncodingRules::Coer,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
#[allow(dead_code)]
pub enum EncodingRules {
    Coer,
}

#[allow(dead_code)]
impl EncodingRules {
    pub fn is_coer(self) -> bool {
        matches!(self, Self::Coer)
    }
}
