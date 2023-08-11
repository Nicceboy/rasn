// ITU-T X.696 (02/2021) version of OER decoding
// In OER, without knowledge of the type of the value encoded, it is not possible to determine
// the structure of the encoding. In particular, the end of the encoding cannot be determined from
// the encoding itself without knowledge of the type being encoded ITU-T X.696 (6.2).
use crate::prelude::{
    Any, BitString, BmpString, Constraints, Constructed, DecodeChoice, Enumerated, GeneralString,
    GeneralizedTime, Ia5String, Integer, NumericString, ObjectIdentifier, PrintableString, SetOf,
    TeletexString, UtcTime, VisibleString,
};
use crate::{de::Error as _, Decode, Tag};
use alloc::{string::String, vec::Vec};
use bitvec::macros::internal::funty::Fundamental;
use bitvec::order::Msb0;
use bitvec::vec::BitVec;
use nom::AsBytes;
use num_bigint::BigUint;
use num_traits::ToPrimitive;

// Max length for data type can be 2^1016, below presented as byte array of unsigned int
const MAX_LENGTH: [u8; 127] = [0xff; 127];
const MAX_LENGTH_LENGTH: usize = MAX_LENGTH.len();
pub use crate::per::de::Error;

pub type Result<T, E = Error> = core::result::Result<T, E>;

type InputSlice<'input> = nom_bitvec::BSlice<'input, u8, bitvec::order::Msb0>;

#[derive(Clone, Copy, Debug)]
pub struct DecoderOptions {
    // pub(crate) encoding_rules: EncodingRules,
}

pub struct Decoder<'input> {
    input: InputSlice<'input>,
    // options: DecoderOptions,
}

impl<'input> Decoder<'input> {
    #[must_use]
    // pub fn new(input: &'input crate::types::BitStr, options: DecoderOptions) -> Self {
    pub fn new(input: &'input crate::types::BitStr) -> Self {
        Self {
            input: input.into(),
            // options,
        }
    }
    fn parse_one_bit(&mut self) -> Result<bool> {
        let (input, boolean) = nom::bytes::streaming::take(1u8)(self.input)?;
        self.input = input;
        Ok(boolean[0])
    }
    fn parse_one_byte(&mut self) -> Result<u8> {
        let (input, byte) = nom::bytes::streaming::take(8u8)(self.input)?;
        self.input = input;
        Ok(byte.as_bytes()[0])
    }
    /// There is a short form and long form for length determinant in OER encoding.
    /// In short form one octet is used and the leftmost bit is always zero; length is less than 128
    /// Max length for data type can be 2^1016 - 1 octets
    fn decode_length(&mut self) -> Result<BigUint> {
        // In OER decoding, there might be cases when there are multiple zero octets as padding
        // or the length is encoded in more than one octet.
        let mut possible_length: u8;
        loop {
            // Remove leading zero octets
            possible_length = self.parse_one_byte()?;
            if possible_length != 0 {
                break;
            }
        }
        if possible_length < 128 {
            Ok(BigUint::from(possible_length))
        } else {
            // We have the length of the length, mask and extract only 7 bis
            let length = possible_length & 0x7fu8;
            // Should not overflow, max size 8 x 127 = 1016 < u16::MAX
            let result: Result<(InputSlice, InputSlice), Error> =
                nom::bytes::streaming::take(length.as_u16() * 8)(self.input).map_err(Error::from);
            match result {
                Ok((input, data)) => {
                    self.input = input;
                    Ok(BigUint::from_bytes_be(data.as_bytes()))
                }
                Err(e) => Err(e),
            }
        }
    }
    fn extract_data_by_length(&mut self, length: BigUint) -> Result<BitVec<u8, Msb0>> {
        // Might be more efficient than full integer conversion
        if length.to_bytes_be().len() > MAX_LENGTH_LENGTH {
            return Err(Error::exceeds_max_length(length));
        }
        // Seems like we can take only usize::MAX bytes at once
        let mut data: BitVec<u8, Msb0> = BitVec::new();
        let mut remainder: BigUint = length.clone();
        loop {
            if &remainder * 8u8 > usize::MAX.into() {
                let (input, partial_data) = nom::bytes::streaming::take(usize::MAX)(self.input)?;
                self.input = input;
                data.extend(partial_data.as_bytes());
                remainder -= usize::MAX;
            } else {
                match remainder.to_usize() {
                    Some(0) => break,
                    None => return Err(Error::exceeds_max_length(remainder)),
                    Some(to_usize) => {
                        let (input, partial_data) =
                            nom::bytes::streaming::take(to_usize)(self.input)?;
                        self.input = input;
                        data.extend(partial_data.as_bytes());
                        break;
                    }
                }
            }
        }
        Ok(data)
    }
}

impl<'input> crate::Decoder for Decoder<'input> {
    type Error = Error;

    fn decode_any(&mut self) -> Result<Any, Self::Error> {
        todo!()
    }

    fn decode_bit_string(
        &mut self,
        _: Tag,
        constraints: Constraints,
    ) -> Result<BitString, Self::Error> {
        todo!()
    }
    /// One octet is used to present bool, false is 0x0 and true is value up to 0xff
    fn decode_bool(&mut self, _: Tag) -> Result<bool, Self::Error> {
        Ok(self.parse_one_byte()? > 0)
    }

    fn decode_enumerated<E: Enumerated>(&mut self, tag: Tag) -> Result<E, Self::Error> {
        todo!()
    }

    fn decode_integer(&mut self, _: Tag, constraints: Constraints) -> Result<Integer, Self::Error> {
        todo!()
    }

    fn decode_null(&mut self, _: Tag) -> Result<(), Self::Error> {
        todo!()
    }

    fn decode_object_identifier(&mut self, _: Tag) -> Result<ObjectIdentifier, Self::Error> {
        todo!()
    }

    fn decode_sequence<D, F>(&mut self, _: Tag, decode_fn: F) -> Result<D, Self::Error>
    where
        D: Constructed,
        F: FnOnce(&mut Self) -> Result<D, Self::Error>,
    {
        todo!()
    }

    fn decode_sequence_of<D: Decode>(
        &mut self,
        tag: Tag,
        constraints: Constraints,
    ) -> Result<Vec<D>, Self::Error> {
        todo!()
    }

    fn decode_set_of<D: Decode + Ord>(
        &mut self,
        tag: Tag,
        constraints: Constraints,
    ) -> Result<SetOf<D>, Self::Error> {
        todo!()
    }

    fn decode_octet_string(
        &mut self,
        _: Tag,
        constraints: Constraints,
    ) -> Result<Vec<u8>, Self::Error> {
        todo!()
    }

    fn decode_utf8_string(
        &mut self,
        _: Tag,
        constraints: Constraints,
    ) -> Result<String, Self::Error> {
        todo!()
    }

    fn decode_visible_string(
        &mut self,
        _: Tag,
        constraints: Constraints,
    ) -> Result<VisibleString, Self::Error> {
        todo!()
    }

    fn decode_general_string(
        &mut self,
        _: Tag,
        constraints: Constraints,
    ) -> Result<GeneralString, Self::Error> {
        todo!()
    }

    fn decode_ia5_string(
        &mut self,
        _: Tag,
        constraints: Constraints,
    ) -> Result<Ia5String, Self::Error> {
        todo!()
    }

    fn decode_printable_string(
        &mut self,
        _: Tag,
        constraints: Constraints,
    ) -> Result<PrintableString, Self::Error> {
        todo!()
    }

    fn decode_numeric_string(
        &mut self,
        _: Tag,
        constraints: Constraints,
    ) -> Result<NumericString, Self::Error> {
        todo!()
    }

    fn decode_teletex_string(
        &mut self,
        _: Tag,
        constraints: Constraints,
    ) -> Result<TeletexString, Self::Error> {
        todo!()
    }

    fn decode_bmp_string(
        &mut self,
        _: Tag,
        constraints: Constraints,
    ) -> Result<BmpString, Self::Error> {
        todo!()
    }

    fn decode_explicit_prefix<D: Decode>(&mut self, tag: Tag) -> Result<D, Self::Error> {
        todo!()
    }

    fn decode_utc_time(&mut self, tag: Tag) -> Result<UtcTime, Self::Error> {
        todo!()
    }

    fn decode_generalized_time(&mut self, tag: Tag) -> Result<GeneralizedTime, Self::Error> {
        todo!()
    }

    fn decode_set<FIELDS, SET, D, F>(
        &mut self,
        tag: Tag,
        decode_fn: D,
        field_fn: F,
    ) -> Result<SET, Self::Error>
    where
        SET: Decode + Constructed,
        FIELDS: Decode,
        D: Fn(&mut Self, usize, Tag) -> Result<FIELDS, Self::Error>,
        F: FnOnce(Vec<FIELDS>) -> Result<SET, Self::Error>,
    {
        todo!()
    }

    fn decode_choice<D>(&mut self, constraints: Constraints) -> Result<D, Self::Error>
    where
        D: DecodeChoice,
    {
        todo!()
    }

    fn decode_optional<D: Decode>(&mut self) -> Result<Option<D>, Self::Error> {
        todo!()
    }

    fn decode_optional_with_tag<D: Decode>(&mut self, tag: Tag) -> Result<Option<D>, Self::Error> {
        todo!()
    }

    fn decode_optional_with_constraints<D: Decode>(
        &mut self,
        constraints: Constraints,
    ) -> Result<Option<D>, Self::Error> {
        todo!()
    }

    fn decode_optional_with_tag_and_constraints<D: Decode>(
        &mut self,
        tag: Tag,
        constraints: Constraints,
    ) -> Result<Option<D>, Self::Error> {
        todo!()
    }

    fn decode_extension_addition<D>(&mut self) -> Result<Option<D>, Self::Error>
    where
        D: Decode,
    {
        todo!()
    }

    fn decode_extension_addition_group<D: Decode + Constructed>(
        &mut self,
    ) -> Result<Option<D>, Self::Error> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_decode_bool() {
        let decoded: bool = crate::oer::decode(&[0xffu8]).unwrap();
        assert!(decoded);
        let decoded: bool = crate::oer::decode(&[0u8]).unwrap();
        assert!(!decoded);
        let decoded: bool = crate::oer::decode(&[0xffu8, 0xff]).unwrap();
        assert!(decoded);
        let decoded: bool = crate::oer::decode(&[0x33u8, 0x0]).unwrap();
        assert!(decoded);
    }

    #[test]
    fn test_decode_length_invalid() {
        let data: BitString = BitString::from_slice(&[0xffu8]);
        let mut decoder = crate::oer::Decoder::new(&data);
        // Length determinant is > 127 without subsequent bytes
        assert!(decoder.decode_length().is_err());
        // Still missing some data
        let data: BitString = BitString::from_slice(&[0xffu8, 0xff]);
        let mut decoder = crate::oer::Decoder::new(&data);
        // Length determinant is > 127 without subsequent bytes
        assert!(decoder.decode_length().is_err());
    }

    #[test]
    fn test_decode_length_valid() {
        // Max length
        let max_length: BigUint = BigUint::from(2u8).pow(1016u32) - BigUint::from(1u8);
        assert_eq!(max_length.to_bytes_be(), MAX_LENGTH);
        assert_eq!(max_length.to_bytes_be().len(), MAX_LENGTH_LENGTH);
        // # SHORT FORM
        let data: BitString = BitString::from_slice(&[0x01u8, 0xff]);
        let mut decoder = crate::oer::Decoder::new(&data);
        assert_eq!(decoder.decode_length().unwrap(), BigUint::from(1u8));
        let data: BitString = BitString::from_slice(&[0x03u8, 0xff, 0xff, 0xfe]);
        let mut decoder = crate::oer::Decoder::new(&data);
        assert_eq!(decoder.decode_length().unwrap(), BigUint::from(3u8));
        // Max for short form
        let mut data: [u8; 0x80] = [0xffu8; 0x80];
        data[0] = 0x7f; // length determinant
        let data: BitString = BitString::from_slice(&data);
        let mut decoder = crate::oer::Decoder::new(&data);
        assert_eq!(decoder.decode_length().unwrap(), BigUint::from(127u8));

        // # LONG FORM
        // Length of the length should be 2 octets, 0x7f - 0x82 = 2, length is 258 octets
        let length: [u8; 1] = [0x82u8]; // first bit is 1, remaining tells length of length
        let length_determinant: [u8; 0x02] = [0x01u8, 0x02];
        let data: [u8; 258] = [0xffu8; 258];
        let mut combined: [u8; 261] = [0x0; 261];
        combined[..1].copy_from_slice(&length);
        combined[1..=2].copy_from_slice(&length_determinant);
        combined[3..].copy_from_slice(&data);

        let data: BitString = BitString::from_slice(&combined);
        let mut decoder = crate::oer::Decoder::new(&data);
        assert_eq!(decoder.decode_length().unwrap(), BigUint::from(258u16));
    }
}
