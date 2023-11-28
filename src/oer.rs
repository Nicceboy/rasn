//! Codec for Octet Encoding Rules (OER).
//! Encodes in canonical format (COER), and decodes in more versatile format (OER).
pub mod de;
pub mod enc;
mod ranges;

pub use self::{de::Decoder, enc::Encoder};
use crate::error::{DecodeError, EncodeError};
use crate::types::Constraints;
/// Attempts to decode `T` from `input` using OER.
///
/// # Errors
/// Returns `DecodeError` if `input` is not valid OER encoding specific to the expected type.
pub(crate) fn decode<T: crate::Decode>(
    // options: de::DecoderOptions,
    input: &[u8],
) -> Result<T, DecodeError> {
    T::decode(&mut Decoder::new(
        crate::types::BitStr::from_slice(input),
        de::DecoderOptions::oer(),
    ))
}
/// Attempts to encode `value` of type `T` to OER.
///
/// # Errors
/// Returns `EncodeError` if `value` cannot be encoded as COER, usually meaning that constraints
/// are not met.
pub(crate) fn encode<T: crate::Encode>(
    // options: enc::EncoderOptions,
    value: &T,
) -> Result<alloc::vec::Vec<u8>, EncodeError> {
    let mut enc = Encoder::new(enc::EncoderOptions::coer());
    value.encode(&mut enc)?;
    Ok(enc.output())
}
/// Attempts to decode `T` from `input` using OER with constraints.
///
/// # Errors
/// Returns `DecodeError` if `input` is not valid OER encoding, while setting specific constraints.
pub(crate) fn decode_with_constraints<T: crate::Decode>(
    // options: de::DecoderOptions,
    constraints: Constraints,
    input: &[u8],
) -> Result<T, DecodeError> {
    T::decode_with_constraints(
        &mut Decoder::new(
            crate::types::BitStr::from_slice(input),
            de::DecoderOptions::oer(),
        ),
        constraints,
    )
}
/// Attempts to encode `value` to COER with constraints.
///
/// # Errors
/// Returns `EncodeError` if `value` cannot be encoded as COER, while setting specific constraints.
pub(crate) fn encode_with_constraints<T: crate::Encode>(
    // options: enc::EncoderOptions,
    constraints: Constraints,
    value: &T,
) -> Result<alloc::vec::Vec<u8>, EncodeError> {
    let mut enc = Encoder::new(enc::EncoderOptions::coer());
    value.encode_with_constraints(&mut enc, constraints)?;
    Ok(enc.output())
}
