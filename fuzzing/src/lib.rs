// Attempts to decode random fuzz data and if we're successful, we check
// that the encoder can produce encoding that the is *semantically*
// equal to the original decoded value. So we decode that value back
// into Rust because the encoder is guaranteed to produce the same
// encoding as the accepted input since `data` could contain trailing
// bytes not used by the decoder.
#![allow(clippy::missing_docs_in_private_items)]
pub mod fuzz_types;

// use fuzz_types::*;
use fuzz_types::{Choice1, Sequence1, SingleSizeConstrainedBitString};
use rasn::prelude::*;
use rasn_smi::v2::ObjectSyntax;

macro_rules! fuzz_any_type_fn {
    ($fn_name:ident, $codec:ident) => {
        pub fn $fn_name<T: Encode + Decode + std::fmt::Debug + PartialEq>(data: &[u8]) {
            dbg!(data);
            if let Ok(value) = rasn::$codec::decode::<T>(data) {
                dbg!(&value);
                let encoded = rasn::$codec::encode(&value).unwrap();
                dbg!(&encoded);
                let decoded = rasn::$codec::decode::<T>(&encoded).unwrap();
                dbg!(&decoded);
                assert_eq!(value, decoded);
            }
        }
    };
}

// macro_rules! fuzz_many_types {
//     ($codec:ident, $data:expr, $($typ:ty),+ $(,)?) => {
//         $(
//             if let Ok(value) = rasn::$codec::decode::<$typ>($data) {
//                 assert_eq!(value, rasn::$codec::decode::<$typ>(&rasn::$codec::encode(&value).unwrap()).unwrap());
//             }
//         )+
//     }
// }
// Creates a codec-specific fuzz function which can fuzz any ASN.1 type
// that implements `Encode`, `Decode`, `Debug` and `PartialEq` traits.
// Use e.g. fuzz_oer::<Integer>(data);
fuzz_any_type_fn!(fuzz_oer, oer);
fuzz_any_type_fn!(fuzz_coer, coer);
fuzz_any_type_fn!(fuzz_aper, aper);
fuzz_any_type_fn!(fuzz_uper, uper);
fuzz_any_type_fn!(fuzz_ber, ber);
fuzz_any_type_fn!(fuzz_cer, cer);
fuzz_any_type_fn!(fuzz_der, der);

pub fn fuzz_codec(data: &[u8]) {
    // fuzz_coer::<Integers>(data);
    // fuzz_coer::<Enum1>(data);
    fuzz_coer::<ObjectIdentifier>(data);
    // fuzz_coer::<Choice1>(data);
    // fuzz_coer::<IntegerA>(data);
    // fuzz_coer::<IntegerB>(data);
    // fuzz_coer::<IntegerC>(data);
    // fuzz_oer::<ConstrainedBitString>(data);
    // fuzz_oer::<Sequence1>(data);
    // fuzz_oer::<SingleSizeConstrainedBitString>(data);
}
// pub fn fuzz_pkix(data: &[u8]) {
//     fuzz_many_types!(der, data, rasn_pkix::Certificate);
// }

pub fn fuzz(data: &[u8]) {
    fuzz_codec(data);
}
