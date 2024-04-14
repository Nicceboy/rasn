// use crate::oer::enc::error::Error;
// use crate::oer::enc::Error;
use crate::prelude::{
    Any, BmpString, Choice, Constructed, Enumerated, GeneralString, GeneralizedTime, Ia5String,
    NumericString, PrintableString, SetOf, TeletexString, UtcTime, VisibleString,
};
use crate::types::{constraints::*, BitString, Constraints, Integer};
use crate::{Encode, Tag};
use bitvec::prelude::*;

/// ITU-T X.696 (02/2021) version of (C)OER encoding
/// On this crate, only canonical version will be used to provide unique and reproducible encodings.
/// Basic-OER is not supported and it might be that never will.
mod config;
mod error;
use error::Error;

pub type Result<T, E = Error> = core::result::Result<T, E>;

pub const ITU_T_X696_OER_EDITION: f32 = 3.0;

// ## HELPER FUNCTIONS start which should be refactored elsewhere

/// A convenience type around results needing to return one or many bytes.
// enum ByteOrBytes {
//     Single(u8),
//     Many(Vec<u8>),
// }
//
// fn append_byte_or_bytes(output_vec: &mut Vec<u8>, bytes: ByteOrBytes) {
//     match bytes {
//         ByteOrBytes::Single(b) => output_vec.push(b),
//         ByteOrBytes::Many(mut bs) => output_vec.append(&mut bs),
//     }
// }
// HELPER FUNCTIONS end

/// COER encoder. A subset of OER to provide canonical and unique encoding.  
pub struct Encoder {
    options: config::EncoderOptions,
    output: BitString,
}
// ITU-T X.696 8.2.1 Only the following constraints are OER-visible:
// a) non-extensible single value constraints and value range constraints on integer types;
// b) non-extensible single value constraints on real types where the single value is either plus zero or minus zero or
// one of the special real values PLUS-INFINITY, MINUS-INFINITY and NOT-A-NUMBER;
// c) non-extensible size constraints on known-multiplier character string types, octetstring types, and bitstring
// types;
// d) non-extensible property settings constraints on the time type or on the useful and defined time types;
// e) inner type constraints applying OER-visible constraints to real types when used to restrict the mantissa, base,
// or exponent;
// f) inner type constraints applied to CHARACTER STRING or EMBEDDED-PDV types when used to restrict
// the value of the syntaxes component to a single value, or when used to restrict identification to the fixed
// alternative;
// g) contained subtype constraints in which the constraining type carries an OER-visible constraint.

// Tags are encoded only as part of the encoding of a choice type, where the tag indicates
// which alternative of the choice type is the chosen alternative (see 20.1).
impl Encoder {
    pub fn new(options: config::EncoderOptions) -> Self {
        Self {
            options,
            output: <BitString>::default(),
        }
    }

    /// ITU-T X.696 9.
    /// False is encoded as a single zero octet. In COER, true is always encoded as 0xFF.
    /// In Basic-OER, any non-zero octet value represents true, but we support only canonical encoding.
    // fn encode_bool(&mut self, value: bool) {
    // self.output.push(if value { 0xffu8 } else { 0x00u8 });
    // append_byte_or_bytes(
    //     &mut self.output,
    //     if value {
    //         ByteOrBytes::Single(0xff)
    //     } else {
    //         ByteOrBytes::Single(0x00)
    //     },
    // );
    // }

    /// Encode the length of the value to output.
    /// Length of the data `length` should be provided in bytes (octets), not as bits.
    /// In COER we try to use the shortest possible encoding, hence convert to the smallest integer type.
    fn encode_length(&mut self, length: usize) -> Result<(), Error> {
        // Bits to byte length
        let length = length / 8;
        let bytes: BitVec<u8, Msb0> = match length {
            v if u8::try_from(v).is_ok() => {
                BitVec::<u8, Msb0>::from_slice(&(length as u8).to_be_bytes())
            }
            v if u16::try_from(v).is_ok() => {
                BitVec::<u8, Msb0>::from_slice(&(length as u16).to_be_bytes())
            }
            v if u32::try_from(v).is_ok() => {
                BitVec::<u8, Msb0>::from_slice(&(length as u32).to_be_bytes())
            }
            v if u64::try_from(v).is_ok() => {
                BitVec::<u8, Msb0>::from_slice(&(length as u64).to_be_bytes())
            }
            _ => BitVec::<u8, Msb0>::from_slice(&(length as u128).to_be_bytes()),
        };
        if length < 128 {
            // First bit should be always zero when below 128: ITU-T X.696 8.6.4
            self.output.extend(&bytes);
            return Ok(());
        }
        let length_of_length = u8::try_from(bytes.len() / 8);
        if length_of_length.is_ok() && length_of_length.unwrap() > 127 {
            return Err(Error::TooLongValue {
                length: length as u128,
            });
        } else if length_of_length.is_ok() {
            self.output.extend(length_of_length.unwrap().to_be_bytes());
            // We must swap the first bit to show long form
            // It is always zero by default with u8 type when value being < 128
            _ = self.output.remove(0);
            self.output.insert(0, true);
            self.output.extend(bytes);
        } else {
            return Err(Error::Propagated {
                msg: length_of_length.err().unwrap().to_string(),
            });
        }
        Ok(())
    }

    /// Constraints define whether the integer is encoded as unsigned or signed
    fn integer_bytes_when_range(
        &self,
        value: &Integer,
        value_range: &Extensible<Value>,
    ) -> Vec<u8> {
        // match value.cmp(&BigInt::from(0)) {
        //     Ordering::Greater | Ordering::Equal => value.to_biguint().unwrap().to_bytes_be(),
        //     Ordering::Less => value.to_signed_bytes_be(),
        // }
        match value_range.constraint.effective_bigint_value(value.clone()) {
            either::Left(offset) => offset.to_biguint().unwrap().to_bytes_be(),
            either::Right(value) => value.to_signed_bytes_be(),
        }
    }

    /// Encode an integer value with constraints.
    ///
    /// Encoding depends on the range constraint, and has two scenarios.
    /// a) The effective value constraint has a lower bound, and that lower bound is zero or positive.
    /// b) The effective value constraint has either a negative lower bound or no lower bound.
    /// Other integer constraints are OER-invisible.
    /// Unlike PER, OER does not add an extension bit at the beginning of the encoding of an integer
    /// type with an extensible OER-visible constraint. Such a type is encoded as an integer type with no bounds.
    ///
    /// If the Integer is not bound or outside of range, we encode with the smallest number of octets possible.
    fn encode_integer_with_constraints(
        &mut self,
        constraints: &Constraints,
        value_to_enc: &Integer,
    ) -> Result<(), Error> {
        // Using signed integers as default when no constraints
        let mut encode_unconstrained = |value_to_enc: Integer, signed: bool| -> Result<(), Error> {
            let bytes = if signed {
                BitVec::<u8, Msb0>::from_slice(&value_to_enc.to_signed_bytes_be())
            } else {
                BitVec::<u8, Msb0>::from_slice(&value_to_enc.to_biguint().unwrap().to_bytes_be())
            };
            let result = self.encode_length(bytes.len());
            result?;
            self.output.extend(bytes);
            Ok(())
        };

        if let Some(value) = constraints.value() {
            // Check if Integer is in constraint range
            // Integer type with extension leads ignoring the whole bound in COER
            if !value.constraint.0.bigint_contains(value_to_enc) && value.extensible.is_none() {
                return Err(Error::IntegerOutOfRange {
                    value: value_to_enc.clone(),
                    expected: value.constraint.0,
                });
            } else if value.extensible.is_some() {
                encode_unconstrained(value_to_enc.clone(), true)?;
            }
            if let Bounded::Range { start, end } = value.constraint.0 {
                match (start, end) {
                    // if let (Some(end), Some(start)) ||  = (end, start) {
                    (Some(start), Some(end)) => {
                        // Case a)
                        if start >= 0.into() {
                            let ranges: [i128; 5] = [
                                // encode as a fixed-size unsigned number in a one, two four or eight-octet word
                                // depending on the value of the upper bound
                                -1i128,
                                u8::MAX.into(),  // should be 1 octets
                                u16::MAX.into(), // should be 2 octets
                                u32::MAX.into(), // should be 4 octets
                                u64::MAX.into(), // should be 8 octets
                            ];
                            for (index, range) in ranges[0..(ranges.len() - 1)].iter().enumerate() {
                                if range < &end && end <= ranges[index + 1] {
                                    let bytes = self.integer_bytes_when_range(value_to_enc, &value);
                                    self.encode_integer_with_padding(ranges[index + 1], &bytes)?;
                                    return Ok(());
                                }
                            }
                            // Upper bound is greater than u64::MAX, encode with length determinant
                            encode_unconstrained(value_to_enc.clone(), false)?;
                        } else {
                            // Negative lower bound
                        }
                    }
                    (Some(start), None) => {
                        // No upper bound
                    }
                    _ => {
                        // Hmm
                    }
                }

                // no lower bound
            }
        } else {
            // No constraints,
            encode_unconstrained(value_to_enc.clone(), true)?;
        }
        Ok(())
    }

    /// When range constraints are present, the integer is encoded as a fixed-size unsigned number.
    /// This means that the zero padding is possible even with COER encoding.
    fn encode_integer_with_padding(&mut self, octets: i128, bytes: &[u8]) -> Result<(), Error> {
        use core::cmp::Ordering;
        let total_bits = crate::per::log2(octets) as usize;
        let bits = BitVec::<u8, Msb0>::from_slice(bytes);
        let bits = match total_bits.cmp(&bits.len()) {
            Ordering::Greater => {
                let mut padding = BitString::repeat(false, total_bits - bits.len());
                padding.extend(bits);
                padding
            }
            Ordering::Less => {
                return Err(Error::MoreBytesThanExpected {
                    value: bits.len(),
                    expected: total_bits,
                })
            }
            Ordering::Equal => bits,
        };
        self.output.extend(bits);
        Ok(())
    }
}

impl crate::Encoder for Encoder {
    type Ok = ();
    type Error = Error;

    fn encode_any(&mut self, tag: Tag, value: &Any) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_bool(&mut self, tag: Tag, value: bool) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_bit_string(
        &mut self,
        tag: Tag,
        constraints: Constraints,
        value: &BitString,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_enumerated<E: Enumerated>(
        &mut self,
        tag: Tag,
        value: &E,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_object_identifier(
        &mut self,
        tag: Tag,
        value: &[u32],
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_integer(
        &mut self,
        _tag: Tag,
        constraints: Constraints,
        value: &Integer,
    ) -> Result<Self::Ok, Self::Error> {
        self.encode_integer_with_constraints(&constraints, value)
    }

    fn encode_null(&mut self, tag: Tag) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_octet_string(
        &mut self,
        tag: Tag,
        constraints: Constraints,
        value: &[u8],
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_general_string(
        &mut self,
        tag: Tag,
        constraints: Constraints,
        value: &GeneralString,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_utf8_string(
        &mut self,
        tag: Tag,
        constraints: Constraints,
        value: &str,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_visible_string(
        &mut self,
        tag: Tag,
        constraints: Constraints,
        value: &VisibleString,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_ia5_string(
        &mut self,
        tag: Tag,
        constraints: Constraints,
        value: &Ia5String,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_printable_string(
        &mut self,
        tag: Tag,
        constraints: Constraints,
        value: &PrintableString,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_numeric_string(
        &mut self,
        tag: Tag,
        constraints: Constraints,
        value: &NumericString,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_teletex_string(
        &mut self,
        tag: Tag,
        constraints: Constraints,
        value: &TeletexString,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_bmp_string(
        &mut self,
        tag: Tag,
        constraints: Constraints,
        value: &BmpString,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_generalized_time(
        &mut self,
        tag: Tag,
        value: &GeneralizedTime,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_utc_time(&mut self, tag: Tag, value: &UtcTime) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_explicit_prefix<V: Encode>(
        &mut self,
        tag: Tag,
        value: &V,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_sequence<C, F>(&mut self, tag: Tag, encoder_scope: F) -> Result<Self::Ok, Self::Error>
    where
        C: Constructed,
        F: FnOnce(&mut Self) -> Result<(), Self::Error>,
    {
        todo!()
    }

    fn encode_sequence_of<E: Encode>(
        &mut self,
        tag: Tag,
        value: &[E],
        constraints: Constraints,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_set<C, F>(&mut self, tag: Tag, value: F) -> Result<Self::Ok, Self::Error>
    where
        C: Constructed,
        F: FnOnce(&mut Self) -> Result<(), Self::Error>,
    {
        todo!()
    }

    fn encode_set_of<E: Encode>(
        &mut self,
        tag: Tag,
        value: &SetOf<E>,
        constraints: Constraints,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_some<E: Encode>(&mut self, value: &E) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_some_with_tag_and_constraints<E: Encode>(
        &mut self,
        tag: Tag,
        constraints: Constraints,
        value: &E,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_none<E: Encode>(&mut self) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_none_with_tag(&mut self, tag: Tag) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_choice<E: Encode + Choice>(
        &mut self,
        constraints: Constraints,
        encode_fn: impl FnOnce(&mut Self) -> Result<Tag, Self::Error>,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_extension_addition<E: Encode>(
        &mut self,
        tag: Tag,
        constraints: Constraints,
        value: E,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_extension_addition_group<E>(
        &mut self,
        value: Option<&E>,
    ) -> Result<Self::Ok, Self::Error>
    where
        E: Encode + Constructed,
    {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use num_bigint::BigInt;
    // const ALPHABETS: &[u32] = &{
    //     let mut array = [0; 26];
    //     let mut i = 0;
    //     let mut start = 'a' as u32;
    //     let end = 'z' as u32;
    //     loop {
    //         array[i] = start;
    //         start += 1;
    //         i += 1;
    //
    //         if start > end {
    //             break;
    //         }
    //     }
    //
    //     array
    // };
    #[test]
    fn test_encode_bool() {
        // let mut encoder = Encoder::new(super::config::EncoderOptions::coer());
        // encoder.encode_bool(true);
        // assert_eq!(encoder.output, vec![0xffu8]);
        // encoder.encode_bool(false);
        // assert_eq!(encoder.output, vec![0xffu8, 0x00u8]);
    }
    #[test]
    fn test_encode_integer_manual_setup() {
        let value_range = &[Constraint::Value(Extensible::new(Value::new(
            Bounded::Range {
                start: 0.into(),
                end: 255.into(),
            },
        )))];
        let consts = Constraints::new(value_range);
        let mut encoder = Encoder::new(config::EncoderOptions::coer());
        let result = encoder.encode_integer_with_constraints(&consts, &BigInt::from(244));
        assert!(result.is_ok());
        let v = vec![244u8];
        let bv = BitVec::<_, Msb0>::from_vec(v);
        assert_eq!(encoder.output, bv);
    }
    #[test]
    fn test_integer_with_length() {
        // Using defaults, no limits
        let constraints = Constraints::default();
        let mut encoder = Encoder::new(config::EncoderOptions::coer());
        let result = encoder.encode_integer_with_constraints(&constraints, &BigInt::from(244));
        assert!(result.is_ok());
        let v = vec![2u8, 0, 244];
        let bv = BitVec::<_, Msb0>::from_vec(v);
        assert_eq!(encoder.output, bv);
        encoder.output.clear();
        let result =
            encoder.encode_integer_with_constraints(&constraints, &BigInt::from(-1_234_567));
        assert!(result.is_ok());
        let v = vec![0x03u8, 0xED, 0x29, 0x79];
        let bv = BitVec::<_, Msb0>::from_vec(v);
        assert_eq!(encoder.output, bv);
    }
}
