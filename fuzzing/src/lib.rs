// Attempts to decode random fuzz data and if we're successful, we check
// that the encoder can produce encoding that the is *semantically*
// equal to the original decoded value. So we decode that value back
// into Rust because the encoder is guaranteed to produce the same
// encoding as the accepted input since `data` could contain trailing
// bytes not used by the decoder.
#![allow(clippy::missing_docs_in_private_items)]
pub mod fuzz_types;

// use fuzz_types::*;
use fuzz_types::{Choice1, Sequence1, SequenceOptionals, SingleSizeConstrainedBitString};
use log::{debug, info};
use rasn::prelude::*;
// use rasn_smi::v2::ObjectSyntax;
//
#[cfg(debug_assertions)]
fn debug_bytes(data: &[u8], codec: &str) {
    debug!("{codec} encoded data in decimal array: {:?}", data);
    let in_binary: Vec<String> = data.iter().map(|v| format!("0b{:08b}", v)).collect();
    debug!("{codec} encoded data in binary array: {:?}", in_binary);
}
#[cfg(debug_assertions)]
fn debug_object<T: std::fmt::Debug>(data: T, codec: &str) {
    debug!(data:?; "{codec} decoded data");
}

macro_rules! fuzz_any_type_fn {
    ($fn_name:ident, $codec:ident) => {
        pub fn $fn_name<T: Encode + Decode + std::fmt::Debug + PartialEq>(data: &[u8]) {
            #[cfg(debug_assertions)]
            debug_bytes(data, stringify!($codec));
            match rasn::$codec::decode::<T>(data) {
                Ok(value) => {
                    #[cfg(debug_assertions)]
                    debug_object(&value, stringify!($codec));
                    let encoded = rasn::$codec::encode(&value).unwrap();
                    #[cfg(debug_assertions)]
                    debug_bytes(&encoded, stringify!($codec));
                    let decoded = rasn::$codec::decode::<T>(&encoded).unwrap();
                    #[cfg(debug_assertions)]
                    debug_object(&decoded, stringify!($codec));
                    assert_eq!(value, decoded);
                }
                Err(e) => {
                    #[cfg(debug_assertions)]
                    debug_object(&e, stringify!($codec));
                }
            }
        }
    };
}

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
    // fuzz_coer::<ObjectSyntax>(data);
    // fuzz_coer::<Ia5String>(data);
    fuzz_oer::<SequenceOptionals>(data);
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
