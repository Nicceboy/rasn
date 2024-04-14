use alloc::{string::ToString, vec::Vec};

use bitvec::prelude::*;
use num_traits::{Signed, ToPrimitive};

pub use config::EncoderOptions;
pub use error::Error;

use crate::oer::helpers;
use crate::prelude::{
    Any, BmpString, Choice, Constructed, Enumerated, GeneralString, GeneralizedTime, Ia5String,
    NumericString, PrintableString, SetOf, TeletexString, UtcTime, VisibleString,
};

use crate::types::{BitString, Constraints, Integer};
use crate::{Encode, Tag};

/// ITU-T X.696 (02/2021) version of (C)OER encoding
/// On this crate, only canonical version will be used to provide unique and reproducible encodings.
/// Basic-OER is not supported and it might be that never will.
mod config;
mod error;

pub type Result<T, E = Error> = core::result::Result<T, E>;

pub const ITU_T_X696_OER_EDITION: f32 = 3.0;

impl Default for Encoder {
    fn default() -> Self {
        Self::new()
    }
}
/// COER encoder. A subset of OER to provide canonical and unique encoding.  
pub struct Encoder {
    // options: EncoderOptions,
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
    // pub fn new(options: EncoderOptions) -> Self {
    #[must_use]
    pub fn new() -> Self {
        Self {
            output: <BitString>::default(),
        }
    }

    #[must_use]
    pub fn output(&self) -> Vec<u8> {
        // TODO, move from per to utility module?
        crate::per::to_vec(&self.output)
    }
    /// ITU-T X.696 9.
    /// False is encoded as a single zero octet. In COER, true is always encoded as 0xFF.
    /// In Basic-OER, any non-zero octet value represents true, but we support only canonical encoding.
    fn encode_bool(&mut self, value: bool) {
        self.output
            .extend(BitVec::<u8, Msb0>::from_slice(&[if value {
                0xffu8
            } else {
                0x00u8
            }]));
    }

    /// Encode the length of the value to output.
    /// `Length` of the data should be provided as bits.
    ///
    /// COER tries to use the shortest possible encoding and avoids leading zeros.
    /// `forced_long_form` is used for cases when length < 128 but we want to force long form. E.g. when encoding a enumerated.
    fn encode_length(
        &mut self,
        length: usize,
        signed: bool,
        forced_long_form: bool,
    ) -> Result<(), Error> {
        // Bits to byte length
        let length = if length % 8 == 0 {
            length / 8
        } else {
            Err(Error::LengthNotAsBitLength {
                value: length,
                remainder: length % 8,
            })?
        };
        // On some cases we want to present length also as signed integer
        // E.g. length of a enumerated value
        //  ITU-T X.696 (02/2021) 11.4 ???? Seems like it is not needed
        let bytes = helpers::integer_to_bitvec_bytes(&Integer::from(length), signed)?;
        if length < 128 && !forced_long_form {
            // First bit should be always zero when below 128: ITU-T X.696 8.6.4
            self.output.extend(&bytes);
            return Ok(());
        }
        if length < 128 && forced_long_form {
            // We must swap the first bit to show long form
            let mut bytes = bytes;
            _ = bytes.remove(0);
            bytes.insert(0, true);
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
    /// Encode integer `value_to_enc` with length determinant
    /// Either as signed or unsigned, set by `signed`
    fn encode_unconstrained_integer(
        &mut self,
        value_to_enc: &Integer,
        signed: bool,
        long_form_short_length: bool,
    ) -> Result<(), Error> {
        let bytes = helpers::integer_to_bitvec_bytes(value_to_enc, signed)?;
        self.encode_length(bytes.len(), false, long_form_short_length)?;
        self.output.extend(bytes);
        Ok(())
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
        if let Some(value) = constraints.value() {
            if !value.constraint.0.bigint_contains(value_to_enc) && value.extensible.is_none() {
                return Err(Error::IntegerOutOfRange {
                    value: value_to_enc.clone(),
                    expected: value.constraint.0,
                });
            }
            return helpers::determine_integer_size_and_sign(
                &value,
                value_to_enc,
                |value_to_enc, sign, octets| {
                    if let Some(octets) = octets {
                        self.encode_integer_with_padding(i128::from(octets), value_to_enc, sign)
                    } else {
                        self.encode_unconstrained_integer(value_to_enc, sign, false)
                    }
                },
            );
        }
        self.encode_unconstrained_integer(value_to_enc, true, false)
    }

    /// When range constraints are present, the integer is encoded as a fixed-size number.
    /// This means that the zero padding is possible even with COER encoding.
    fn encode_integer_with_padding(
        &mut self,
        octets: i128,
        value: &Integer,
        signed: bool,
    ) -> Result<(), Error> {
        use core::cmp::Ordering;
        if octets > 8 {
            return Err(Error::Custom {
                msg: alloc::format!("Unexpected constrained integer byte size: {octets}"),
            });
        }
        let bytes = if signed {
            value.to_signed_bytes_be()
        } else {
            value.to_biguint().unwrap().to_bytes_be()
        };
        let bits = BitVec::<u8, Msb0>::from_slice(&bytes);
        // octets * 8 never > 64, safe conversion and multiplication
        #[allow(clippy::cast_sign_loss)]
        let octets_as_bits: usize = octets as usize * 8;
        let bits = match octets_as_bits.cmp(&bits.len()) {
            Ordering::Greater => {
                let mut padding: BitVec<u8, Msb0>;
                if signed && value.is_negative() {
                    // 2's complement
                    padding = BitString::repeat(true, octets_as_bits - bits.len());
                } else {
                    padding = BitString::repeat(false, octets_as_bits - bits.len());
                }
                padding.extend(bits);
                padding
            }
            Ordering::Less => {
                return Err(Error::MoreBytesThanExpected {
                    value: bits.len(),
                    expected: octets_as_bits,
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

    fn encode_any(&mut self, _tag: Tag, value: &Any) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_bool(&mut self, _tag: Tag, value: bool) -> Result<Self::Ok, Self::Error> {
        self.encode_bool(value);
        Ok(())
    }

    fn encode_bit_string(
        &mut self,
        _: Tag,
        constraints: Constraints,
        value: &BitString,
    ) -> Result<Self::Ok, Self::Error> {
        // TODO When Rec. ITU-T X.680 | ISO/IEC 8824-1, 22.7 applies (i.e., the bitstring type is defined with a
        // "NamedBitList"), the bitstring value shall be encoded with trailing 0 bits added or removed as necessary to satisfy the
        // effective size constraint.
        // Rasn does not currently support NamedBitList
        if let Some(size) = constraints.size() {
            if !size.constraint.contains(&value.len()) && size.extensible.is_none() {
                return Err(Error::NotInSizeConstraintRange {
                    length: value.len(),
                });
            }
            // Encode without length determinant
            if size.constraint.is_fixed() {
                let missing_bits: usize = 8 - value.len() % 8;
                let trailing = BitVec::<u8, Msb0>::repeat(false, missing_bits);
                if missing_bits > 0 {
                    self.output.extend(value);
                    self.output.extend(trailing);
                } else {
                    self.output.extend(value);
                }
                return Ok(());
            }
        }
        // With length determinant
        let missing_bits: usize = (8 - value.len() % 8) % 8;
        if missing_bits < 8 {}
        let trailing = BitVec::<u8, Msb0>::repeat(false, missing_bits);
        let mut bit_string = BitVec::<u8, Msb0>::new();
        // missing bits never > 8
        bit_string.extend(missing_bits.to_u8().unwrap().to_be_bytes());
        bit_string.extend(value);
        bit_string.extend(trailing);
        self.encode_length(bit_string.len(), false, false)?;
        self.output.extend(bit_string);
        Ok(())
    }

    fn encode_enumerated<E: Enumerated>(
        &mut self,
        tag: Tag,
        value: &E,
    ) -> Result<Self::Ok, Self::Error> {
        // 11.5 The presence of an extension marker in the definition of an enumerated type does not affect the encoding of
        // the values of the enumerated type.
        // TODO max size for enumerated value is currently only isize MIN/MAX
        // Spec allows between –2^1015 and 2^1015 – 1
        // TODO negative discriminant values are not currently possibly
        let number = value.discriminant();
        if 0isize <= number && number <= i8::MAX.into() {
            self.encode_integer_with_padding(1, &number.into(), false)?;
        } else {
            //Value is signed here as defined in section 11.4
            // Long form
            self.encode_unconstrained_integer(&number.into(), true, true)?;
        }
        Ok(())
    }

    fn encode_object_identifier(
        &mut self,
        tag: Tag,
        value: &[u32],
    ) -> Result<Self::Ok, Self::Error> {
        let octets = match (|| {
            let mut enc = crate::ber::enc::Encoder::new(crate::ber::enc::EncoderOptions::ber());
            enc.object_identifier_as_bytes(value)
        })() {
            Ok(oid) => oid,
            Err(err) => {
                return Err(Error::Propagated {
                    msg: err.to_string(),
                })
            }
        };
        let max_permitted_length = usize::MAX / 8; // In compile time, no performance penalty?
        if value.len() > max_permitted_length {
            return Err(Error::TooLongValue {
                length: value.len() as u128,
            });
        }
        self.encode_length(octets.len() * 8, false, false)?;
        self.output.extend(BitVec::<u8, Msb0>::from_slice(&octets));
        Ok(())
    }

    fn encode_integer(
        &mut self,
        _: Tag,
        constraints: Constraints,
        value: &Integer,
    ) -> Result<Self::Ok, Self::Error> {
        self.encode_integer_with_constraints(&constraints, value)
    }

    fn encode_null(&mut self, _: Tag) -> Result<Self::Ok, Self::Error> {
        Ok(())
    }

    fn encode_octet_string(
        &mut self,
        _: Tag,
        constraints: Constraints,
        value: &[u8],
    ) -> Result<Self::Ok, Self::Error> {
        if let Some(size) = constraints.size() {
            if !size.constraint.contains(&value.len()) && size.extensible.is_none() {
                return Err(Error::NotInSizeConstraintRange {
                    length: value.len(),
                });
            }
            // Encode without length determinant
            if size.constraint.is_fixed() && size.extensible.is_none() {
                self.output.extend(value);
                return Ok(());
            }
        }
        let max_permitted_length = usize::MAX / 8; // In compile time, no performance penalty?
        if value.len() > max_permitted_length {
            return Err(Error::TooLongValue {
                length: value.len() as u128,
            });
        }
        self.encode_length(value.len() * 8, false, false)?;
        self.output.extend(value);
        Ok(())
    }

    fn encode_general_string(
        &mut self,
        _: Tag,
        constraints: Constraints,
        value: &GeneralString,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_utf8_string(
        &mut self,
        _: Tag,
        constraints: Constraints,
        value: &str,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_visible_string(
        &mut self,
        _: Tag,
        constraints: Constraints,
        value: &VisibleString,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_ia5_string(
        &mut self,
        _: Tag,
        constraints: Constraints,
        value: &Ia5String,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_printable_string(
        &mut self,
        _: Tag,
        constraints: Constraints,
        value: &PrintableString,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_numeric_string(
        &mut self,
        _: Tag,
        constraints: Constraints,
        value: &NumericString,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_teletex_string(
        &mut self,
        _: Tag,
        constraints: Constraints,
        value: &TeletexString,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_bmp_string(
        &mut self,
        _: Tag,
        constraints: Constraints,
        value: &BmpString,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_generalized_time(
        &mut self,
        _: Tag,
        value: &GeneralizedTime,
    ) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_utc_time(&mut self, _tag: Tag, value: &UtcTime) -> Result<Self::Ok, Self::Error> {
        todo!()
    }

    fn encode_explicit_prefix<V: Encode>(
        &mut self,
        _: Tag,
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

    fn encode_set<C, F>(&mut self, _tag: Tag, value: F) -> Result<Self::Ok, Self::Error>
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
    use num_bigint::BigInt;

    use crate::types::constraints::{Bounded, Constraint, Constraints, Extensible, Value};

    use super::*;

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
        let mut encoder = Encoder::new();
        encoder.encode_bool(true);
        let mut bv = BitVec::<u8, Msb0>::from_slice(&[0xffu8]);
        assert_eq!(encoder.output, bv);
        encoder.encode_bool(false);
        bv.append(&mut BitVec::<u8, Msb0>::from_slice(&[0x00u8]));
        assert_eq!(encoder.output, bv);
        assert_eq!(encoder.output.as_raw_slice(), &[0xffu8, 0]);
        // Use higher abstraction
        let decoded = crate::oer::encode(&true).unwrap();
        assert_eq!(decoded, &[0xffu8]);
        let decoded = crate::oer::encode(&false).unwrap();
        assert_eq!(decoded, &[0x0]);
    }
    #[test]
    fn test_encode_integer_manual_setup() {
        let range_bound = Bounded::<i128>::Range {
            start: 0.into(),
            end: 255.into(),
        };
        let value_range = &[Constraint::Value(Extensible::new(Value::new(range_bound)))];
        let consts = Constraints::new(value_range);
        let mut encoder = Encoder::default();
        let result = encoder.encode_integer_with_constraints(&consts, &BigInt::from(244));
        assert!(result.is_ok());
        let v = vec![244u8];
        let bv = BitVec::<_, Msb0>::from_vec(v);
        assert_eq!(encoder.output, bv);
        encoder.output.clear();
        let value = BigInt::from(256);
        let result = encoder.encode_integer_with_constraints(&consts, &value);
        assert!(matches!(
            result,
            Err(Error::IntegerOutOfRange {
                value,
                expected: bound
            })
        ));
    }
    #[test]
    fn test_integer_with_length_determinant() {
        // Using defaults, no limits
        let constraints = Constraints::default();
        let mut encoder = Encoder::default();
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
    #[test]
    fn test_large_lengths() {
        let constraints = Constraints::default();
        let mut encoder = Encoder::default();

        // Signed integer with byte length of 128
        // Needs long form to represent
        let number = BigInt::from(256).pow(127) - 1;
        let result = encoder.encode_integer_with_constraints(&constraints, &number);
        assert!(result.is_ok());
        let vc = [
            0x81, 0x80, 0x00, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff,
            0xff, 0xff, 0xff, 0xff,
        ];
        assert_eq!(encoder.output(), vc);
    }
}
