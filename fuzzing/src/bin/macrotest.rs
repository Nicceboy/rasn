use rasn::prelude::*;

#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq, Eq)]
pub struct SequenceOptionals {
    #[rasn(tag(explicit(0)))]
    pub is: Integer,
    #[rasn(tag(explicit(1)))]
    pub late: Option<OctetString>,
    #[rasn(tag(explicit(2)))]
    pub today: Option<Integer>,
}
fn main() {
    let test_seq = SequenceOptionals {
        is: 42.into(),
        late: None,
        today: None,
    };
    dbg!(test_seq);
}
