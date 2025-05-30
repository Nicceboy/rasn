//! # Decoding BER

mod config;
pub(super) mod parser;

use super::identifier::Identifier;
use crate::{
    types::{
        self,
        oid::{MAX_OID_FIRST_OCTET, MAX_OID_SECOND_OCTET},
        Constraints, Enumerated, Tag,
    },
    Decode,
};
use alloc::{borrow::Cow, borrow::ToOwned, string::ToString, vec::Vec};
use chrono::{DateTime, NaiveDate, NaiveDateTime};
use parser::ParseNumberError;

pub use self::config::DecoderOptions;

pub use crate::error::DecodeError;
pub use crate::error::{BerDecodeErrorKind, CodecDecodeError, DecodeErrorKind, DerDecodeErrorKind};
type Result<T, E = DecodeError> = core::result::Result<T, E>;

const EOC: &[u8] = &[0, 0];

/// A BER and variants decoder. Capable of decoding BER, CER, and DER.
pub struct Decoder<'input> {
    input: &'input [u8],
    config: DecoderOptions,
    initial_len: usize,
}

impl<'input> Decoder<'input> {
    /// Return the current codec `Codec` variant
    #[must_use]
    pub fn codec(&self) -> crate::Codec {
        self.config.current_codec()
    }
    /// Returns reference to the remaining input data that has not been parsed.
    #[must_use]
    pub fn remaining(&self) -> &'input [u8] {
        self.input
    }
    /// Create a new [`Decoder`] from the given `input` and `config`.
    #[must_use]
    pub fn new(input: &'input [u8], config: DecoderOptions) -> Self {
        Self {
            input,
            config,
            initial_len: input.len(),
        }
    }

    /// Return a number of the decoded bytes by this decoder
    #[must_use]
    pub fn decoded_len(&self) -> usize {
        self.initial_len - self.input.len()
    }

    fn parse_eoc(&mut self) -> Result<()> {
        let (i, _) = nom::bytes::streaming::tag(EOC)(self.input)
            .map_err(|e| DecodeError::map_nom_err(e, self.codec()))?;
        self.input = i;
        Ok(())
    }

    pub(crate) fn parse_value(&mut self, tag: Tag) -> Result<(Identifier, Option<&'input [u8]>)> {
        let (input, (identifier, contents)) =
            self::parser::parse_value(&self.config, self.input, Some(tag))?;
        self.input = input;
        Ok((identifier, contents))
    }

    pub(crate) fn parse_primitive_value(&mut self, tag: Tag) -> Result<(Identifier, &'input [u8])> {
        let (input, (identifier, contents)) =
            self::parser::parse_value(&self.config, self.input, Some(tag))?;
        self.input = input;
        match contents {
            Some(contents) => Ok((identifier, contents)),
            None => Err(BerDecodeErrorKind::IndefiniteLengthNotAllowed.into()),
        }
    }

    /// Parses a constructed ASN.1 value, checking the `tag`, and optionally
    /// checking if the identifier is marked as encoded. This should be true
    /// in all cases except explicit prefixes.
    fn parse_constructed_contents<D, F>(
        &mut self,
        tag: Tag,
        check_identifier: bool,
        decode_fn: F,
    ) -> Result<D>
    where
        F: FnOnce(&mut Self) -> Result<D>,
    {
        let (identifier, contents) = self.parse_value(tag)?;

        BerDecodeErrorKind::assert_tag(tag, identifier.tag)?;

        if check_identifier && identifier.is_primitive() {
            return Err(BerDecodeErrorKind::InvalidConstructedIdentifier.into());
        }

        let (streaming, contents) = match contents {
            Some(contents) => (false, contents),
            None => (true, self.input),
        };

        let mut inner = Self::new(contents, self.config);

        let result = (decode_fn)(&mut inner)?;

        if streaming {
            self.input = inner.input;
            self.parse_eoc()?;
        } else if !inner.input.is_empty() {
            return Err(DecodeError::unexpected_extra_data(
                inner.input.len(),
                self.codec(),
            ));
        }

        Ok(result)
    }
    /// Decode an object identifier from a byte slice in BER format.
    /// Function is public to be used by other codecs.
    pub fn decode_object_identifier_from_bytes(
        &self,
        data: &[u8],
    ) -> Result<crate::types::ObjectIdentifier, DecodeError> {
        let (mut contents, root_octets) =
            parser::parse_base128_number(data).map_err(|e| match e {
                ParseNumberError::Nom(e) => DecodeError::map_nom_err(e, self.codec()),
                ParseNumberError::Overflow => DecodeError::integer_overflow(32u32, self.codec()),
            })?;
        let first: u32;
        let second: u32;
        const MAX_OID_THRESHOLD: u32 = MAX_OID_SECOND_OCTET + 1;
        if root_octets > MAX_OID_FIRST_OCTET * MAX_OID_THRESHOLD + MAX_OID_SECOND_OCTET {
            first = MAX_OID_FIRST_OCTET;
            second = root_octets - MAX_OID_FIRST_OCTET * MAX_OID_THRESHOLD;
        } else {
            second = root_octets % MAX_OID_THRESHOLD;
            first = (root_octets - second) / MAX_OID_THRESHOLD;
        }

        // preallocate some capacity for the OID arcs, maxing out at 16 elements
        // to prevent excessive preallocation from malformed or malicious
        // packets
        let mut buffer = alloc::vec::Vec::with_capacity(core::cmp::min(contents.len() + 2, 16));
        buffer.push(first);
        buffer.push(second);

        while !contents.is_empty() {
            let (c, number) = parser::parse_base128_number(contents).map_err(|e| match e {
                ParseNumberError::Nom(e) => DecodeError::map_nom_err(e, self.codec()),
                ParseNumberError::Overflow => DecodeError::integer_overflow(32u32, self.codec()),
            })?;
            contents = c;
            buffer.push(number);
        }
        crate::types::ObjectIdentifier::new(buffer)
            .ok_or_else(|| BerDecodeErrorKind::InvalidObjectIdentifier.into())
    }
    /// Parse any GeneralizedTime string, allowing for any from ASN.1 definition
    /// TODO, move to type itself?
    pub fn parse_any_generalized_time_string(
        string: alloc::string::String,
    ) -> Result<types::GeneralizedTime, DecodeError> {
        // Reference https://obj-sys.com/asn1tutorial/node14.html
        // If data contains ., 3 decimal places of seconds are expected
        // If data contains explict Z, result is UTC
        // If data contains + or -, explicit timezone is given
        // If neither Z nor + nor -, purely local time is implied
        // Replace comma with dot for fractional seconds.
        let mut s = if string.contains(',') {
            string.replace(',', ".")
        } else {
            string
        };
        if s.ends_with("Z") {
            s.pop(); // We default to UTC
        }
        // Timezone offset markers are in static location if present
        let has_offset = s.len() >= 5 && {
            let bytes = s.as_bytes();
            bytes[s.len() - 5] == b'+' || bytes[s.len() - 5] == b'-'
        };
        let format_candidates: &[&str] = if s.contains('.') {
            if has_offset {
                &["%Y%m%d%H%M%S%.f%z", "%Y%m%d%H%M%.f%z", "%Y%m%d%H%.f%z"] // We don't know the count of fractions
            } else {
                &["%Y%m%d%H%M%S%.f", "%Y%m%d%H%M%.f", "%Y%m%d%H%.f"] // We don't know the count of fractions
            }
        } else if has_offset {
            match s.len() {
                // Length including timezone offset (YYYYMMDDHHMMSS+HHMM)
                19 => &["%Y%m%d%H%M%S%z"],
                17 => &["%Y%m%d%H%M%z"],
                15 => &["%Y%m%d%H%z"],
                _ => &["%Y%m%d%H%M%S%z", "%Y%m%d%H%M%z", "%Y%m%d%H%z"],
            }
        } else {
            // For local times without timezone, default to UTC later
            match s.len() {
                8 => &["%Y%m%d"],
                10 => &["%Y%m%d%H"],
                12 => &["%Y%m%d%H%M"],
                14 => &["%Y%m%d%H%M%S"],
                _ => &[],
            }
        };
        for fmt in format_candidates {
            if has_offset {
                if let Ok(dt) = DateTime::parse_from_str(&s, fmt) {
                    return Ok(dt);
                }
            } else if let Ok(dt) = NaiveDateTime::parse_from_str(&s, fmt) {
                return Ok(dt.and_utc().into());
            }
        }
        Err(BerDecodeErrorKind::invalid_date(s).into())
    }
    /// Enforce CER/DER restrictions defined in Section 11.7, strictly raise error on non-compliant
    pub fn parse_canonical_generalized_time_string(
        string: alloc::string::String,
    ) -> Result<types::GeneralizedTime, DecodeError> {
        let len = string.len();
        // Helper function to deal with fractions of seconds and without timezone
        let parse_without_timezone =
            |string: &str| -> core::result::Result<NaiveDateTime, DecodeError> {
                let len = string.len();
                if string.contains('.') {
                    // https://github.com/chronotope/chrono/issues/238#issuecomment-378737786
                    NaiveDateTime::parse_from_str(string, "%Y%m%d%H%M%S%.f")
                        .map_err(|_| BerDecodeErrorKind::invalid_date(string.to_string()).into())
                } else if len == 14 {
                    NaiveDateTime::parse_from_str(string, "%Y%m%d%H%M%S")
                        .map_err(|_| BerDecodeErrorKind::invalid_date(string.to_string()).into())
                } else {
                    // CER/DER encoding rules don't allow for timezone offset +/
                    // Or missing seconds/minutes/hours
                    // Or comma , instead of dot .
                    // Or local time without timezone
                    Err(BerDecodeErrorKind::invalid_date(string.to_string()).into())
                }
            };
        if string.ends_with('Z') {
            let naive = parse_without_timezone(&string[..len - 1])?;
            Ok(naive.and_utc().into())
        } else {
            Err(BerDecodeErrorKind::invalid_date(string.to_string()).into())
        }
    }
    /// Parse any UTCTime string, can be any from ASN.1 definition
    /// TODO, move to type itself?
    pub fn parse_any_utc_time_string(
        string: alloc::string::String,
    ) -> Result<types::UtcTime, DecodeError> {
        // When compared to GeneralizedTime, UTC time has no fractions.
        let len = string.len();
        // Largest string, e.g. "820102070000-0500".len() == 17
        if len > 17 {
            return Err(BerDecodeErrorKind::invalid_date(string.to_string()).into());
        }
        let format = if string.contains('Z') {
            if len == 11 {
                "%y%m%d%H%MZ"
            } else {
                "%y%m%d%H%M%SZ"
            }
        } else if len == 15 {
            "%y%m%d%H%M%z"
        } else {
            "%y%m%d%H%M%S%z"
        };
        match len {
            11 | 13 => {
                let naive = NaiveDateTime::parse_from_str(&string, format)
                    .map_err(|_| BerDecodeErrorKind::invalid_date(string.to_string()))?;
                Ok(naive.and_utc())
            }
            15 | 17 => Ok(DateTime::parse_from_str(&string, format)
                .map_err(|_| BerDecodeErrorKind::invalid_date(string.to_string()))?
                .into()),
            _ => Err(BerDecodeErrorKind::invalid_date(string.to_string()).into()),
        }
    }

    /// Enforce CER/DER restrictions defined in Section 11.8, strictly raise error on non-compliant
    pub fn parse_canonical_utc_time_string(string: &str) -> Result<types::UtcTime, DecodeError> {
        let len = string.len();
        if string.ends_with('Z') {
            let naive = match len {
                13 => NaiveDateTime::parse_from_str(string, "%y%m%d%H%M%SZ")
                    .map_err(|_| BerDecodeErrorKind::invalid_date(string.to_string()))?,
                _ => Err(BerDecodeErrorKind::invalid_date(string.to_string()))?,
            };
            Ok(naive.and_utc())
        } else {
            Err(BerDecodeErrorKind::invalid_date(string.to_string()).into())
        }
    }

    /// X.690 8.26.2 and 11.9 -> YYYYMMDD
    pub fn parse_date_string(string: &str) -> Result<types::Date, DecodeError> {
        let date = NaiveDate::parse_from_str(string, "%Y%m%d")
            .map_err(|_| BerDecodeErrorKind::invalid_date(string.to_string()))?;

        Ok(date)
    }
}

impl<'input> crate::Decoder for Decoder<'input> {
    type Ok = ();
    type Error = DecodeError;
    type AnyDecoder<const R: usize, const E: usize> = Decoder<'input>;

    fn codec(&self) -> crate::Codec {
        Self::codec(self)
    }
    fn decode_any(&mut self) -> Result<types::Any> {
        let (mut input, (identifier, contents)) =
            self::parser::parse_value(&self.config, self.input, None)?;

        if contents.is_none() {
            let (i, _) = self::parser::parse_encoded_value(
                &self.config,
                self.input,
                identifier.tag,
                |input, _| Ok(alloc::vec::Vec::from(input)),
            )?;
            input = i;
        }
        let diff = self.input.len() - input.len();
        let contents = &self.input[..diff];
        self.input = input;

        Ok(types::Any {
            contents: contents.to_vec(),
        })
    }

    fn decode_bool(&mut self, tag: Tag) -> Result<bool> {
        let (_, contents) = self.parse_primitive_value(tag)?;
        DecodeError::assert_length(1, contents.len(), self.codec())?;
        Ok(match contents[0] {
            0 => false,
            0xFF => true,
            _ if self.config.encoding_rules.is_ber() => true,
            _ => {
                return Err(DecodeError::from_kind(
                    DecodeErrorKind::InvalidBool { value: contents[0] },
                    self.codec(),
                ))
            }
        })
    }

    fn decode_enumerated<E: Enumerated>(&mut self, tag: Tag) -> Result<E> {
        let discriminant = self.decode_integer::<isize>(tag, Constraints::default())?;

        E::from_discriminant(discriminant)
            .ok_or_else(|| DecodeError::discriminant_value_not_found(discriminant, self.codec()))
    }

    fn decode_integer<I: types::IntegerType>(&mut self, tag: Tag, _: Constraints) -> Result<I> {
        let primitive_bytes = self.parse_primitive_value(tag)?.1;
        let integer_width = I::WIDTH as usize / 8;
        if primitive_bytes.len() > integer_width {
            // in the case of superfluous leading bytes (especially zeroes),
            // we may still want to try to decode the integer even though
            // the length is > integer width ...
            let leading_byte = if primitive_bytes[0] & 0x80 == 0x80 {
                0xFF
            } else {
                0x00
            };
            let input_iter = primitive_bytes
                .iter()
                .copied()
                .skip_while(|n| *n == leading_byte);
            let data_length = input_iter.clone().count();
            I::try_from_bytes(
                &primitive_bytes[primitive_bytes.len() - data_length..primitive_bytes.len()],
                self.codec(),
            )
        } else {
            I::try_from_bytes(primitive_bytes, self.codec())
        }
    }

    fn decode_real<R: types::RealType>(
        &mut self,
        _: Tag,
        _: Constraints,
    ) -> Result<R, Self::Error> {
        Err(DecodeError::real_not_supported(self.codec()))
    }

    fn decode_octet_string<'b, T: From<&'b [u8]> + From<Vec<u8>>>(
        &'b mut self,
        tag: Tag,
        _: Constraints,
    ) -> Result<T> {
        let (identifier, contents) = self.parse_value(tag)?;

        if identifier.is_primitive() {
            match contents {
                Some(c) => Ok(T::from(c)),
                None => Err(BerDecodeErrorKind::IndefiniteLengthNotAllowed.into()),
            }
        } else if identifier.is_constructed() && self.config.encoding_rules.is_der() {
            Err(DerDecodeErrorKind::ConstructedEncodingNotAllowed.into())
        } else {
            let mut buffer = Vec::new();

            if let Some(mut contents) = contents {
                while !contents.is_empty() {
                    let (c, mut vec) = self::parser::parse_encoded_value(
                        &self.config,
                        contents,
                        Tag::OCTET_STRING,
                        |input, _| Ok(alloc::vec::Vec::from(input)),
                    )?;
                    contents = c;

                    buffer.append(&mut vec);
                }
            } else {
                while !self.input.starts_with(EOC) {
                    let (c, mut vec) = self::parser::parse_encoded_value(
                        &self.config,
                        self.input,
                        Tag::OCTET_STRING,
                        |input, _| Ok(alloc::vec::Vec::from(input)),
                    )?;
                    self.input = c;

                    buffer.append(&mut vec);
                }

                self.parse_eoc()?;
            }
            Ok(T::from(buffer))
        }
    }

    fn decode_null(&mut self, tag: Tag) -> Result<()> {
        let (_, contents) = self.parse_primitive_value(tag)?;
        DecodeError::assert_length(0, contents.len(), self.codec())?;
        Ok(())
    }

    fn decode_object_identifier(&mut self, tag: Tag) -> Result<crate::types::ObjectIdentifier> {
        let contents = self.parse_primitive_value(tag)?.1;
        self.decode_object_identifier_from_bytes(contents)
    }

    fn decode_bit_string(&mut self, tag: Tag, _: Constraints) -> Result<types::BitString> {
        let (input, bs) =
            self::parser::parse_encoded_value(&self.config, self.input, tag, |input, codec| {
                let unused_bits = input
                    .first()
                    .copied()
                    .ok_or(DecodeError::unexpected_empty_input(codec))?;

                match unused_bits {
                    // TODO: https://github.com/myrrlyn/bitvec/issues/72
                    bits @ 0..=7 => {
                        let mut buffer = input[1..].to_owned();
                        if let Some(last) = buffer.last_mut() {
                            *last &= !((1 << bits) - 1);
                        }

                        let mut string = types::BitString::from_vec(buffer);
                        let bit_length = string
                            .len()
                            .checked_sub(bits as usize)
                            .ok_or_else(|| DecodeError::invalid_bit_string(unused_bits, codec))?;
                        string.truncate(bit_length);

                        Ok(string)
                    }
                    _ => Err(DecodeError::invalid_bit_string(unused_bits, codec)),
                }
            })?;

        self.input = input;
        Ok(bs)
    }

    fn decode_visible_string(
        &mut self,
        tag: Tag,
        constraints: Constraints,
    ) -> Result<types::VisibleString, Self::Error> {
        types::VisibleString::try_from(
            self.decode_octet_string::<Cow<[u8]>>(tag, constraints)?
                .as_ref(),
        )
        .map_err(|e| DecodeError::permitted_alphabet_error(e, self.codec()))
    }

    fn decode_ia5_string(
        &mut self,
        tag: Tag,
        constraints: Constraints,
    ) -> Result<types::Ia5String> {
        types::Ia5String::try_from(
            self.decode_octet_string::<Cow<[u8]>>(tag, constraints)?
                .as_ref(),
        )
        .map_err(|e| DecodeError::permitted_alphabet_error(e, self.codec()))
    }

    fn decode_printable_string(
        &mut self,
        tag: Tag,
        constraints: Constraints,
    ) -> Result<types::PrintableString> {
        types::PrintableString::try_from(
            self.decode_octet_string::<Cow<[u8]>>(tag, constraints)?
                .as_ref(),
        )
        .map_err(|e| DecodeError::permitted_alphabet_error(e, self.codec()))
    }

    fn decode_numeric_string(
        &mut self,
        tag: Tag,
        constraints: Constraints,
    ) -> Result<types::NumericString> {
        types::NumericString::try_from(
            self.decode_octet_string::<Cow<[u8]>>(tag, constraints)?
                .as_ref(),
        )
        .map_err(|e| DecodeError::permitted_alphabet_error(e, self.codec()))
    }

    fn decode_teletex_string(
        &mut self,
        tag: Tag,
        constraints: Constraints,
    ) -> Result<types::TeletexString> {
        types::TeletexString::try_from(
            self.decode_octet_string::<Cow<[u8]>>(tag, constraints)?
                .as_ref(),
        )
        .map_err(|e| DecodeError::permitted_alphabet_error(e, self.codec()))
    }

    fn decode_bmp_string(
        &mut self,
        tag: Tag,
        constraints: Constraints,
    ) -> Result<types::BmpString> {
        types::BmpString::try_from(
            self.decode_octet_string::<Cow<[u8]>>(tag, constraints)?
                .as_ref(),
        )
        .map_err(|e| DecodeError::permitted_alphabet_error(e, self.codec()))
    }

    fn decode_utf8_string(
        &mut self,
        tag: Tag,
        constraints: Constraints,
    ) -> Result<types::Utf8String> {
        let vec = self.decode_octet_string(tag, constraints)?;
        types::Utf8String::from_utf8(vec).map_err(|e| {
            DecodeError::string_conversion_failed(
                types::Tag::UTF8_STRING,
                e.to_string(),
                self.codec(),
            )
        })
    }

    fn decode_general_string(
        &mut self,
        tag: Tag,
        constraints: Constraints,
    ) -> Result<types::GeneralString> {
        <types::GeneralString>::try_from(
            self.decode_octet_string::<Cow<[u8]>>(tag, constraints)?
                .as_ref(),
        )
        .map_err(|e| DecodeError::permitted_alphabet_error(e, self.codec()))
    }

    fn decode_graphic_string(
        &mut self,
        tag: Tag,
        constraints: Constraints,
    ) -> Result<types::GraphicString> {
        <types::GraphicString>::try_from(
            self.decode_octet_string::<Cow<[u8]>>(tag, constraints)?
                .as_ref(),
        )
        .map_err(|e| DecodeError::permitted_alphabet_error(e, self.codec()))
    }

    fn decode_generalized_time(&mut self, tag: Tag) -> Result<types::GeneralizedTime> {
        let string = self.decode_utf8_string(tag, Constraints::default())?;
        if self.config.encoding_rules.is_ber() {
            Self::parse_any_generalized_time_string(string)
        } else {
            Self::parse_canonical_generalized_time_string(string)
        }
    }

    fn decode_utc_time(&mut self, tag: Tag) -> Result<types::UtcTime> {
        // Reference https://obj-sys.com/asn1tutorial/node15.html
        let string = self.decode_utf8_string(tag, Constraints::default())?;
        if self.config.encoding_rules.is_ber() {
            Self::parse_any_utc_time_string(string)
        } else {
            Self::parse_canonical_utc_time_string(&string)
        }
    }

    fn decode_date(&mut self, tag: Tag) -> core::result::Result<types::Date, Self::Error> {
        let string = self.decode_utf8_string(tag, Constraints::default())?;
        Self::parse_date_string(&string)
    }

    fn decode_sequence_of<D: Decode>(
        &mut self,
        tag: Tag,
        _: Constraints,
    ) -> Result<Vec<D>, Self::Error> {
        self.parse_constructed_contents(tag, true, |decoder| {
            let mut items = Vec::new();

            if decoder.input.is_empty() {
                return Ok(items);
            }

            while let Ok(item) = D::decode(decoder) {
                items.push(item);

                if decoder.input.is_empty() {
                    return Ok(items);
                }
            }

            Ok(items)
        })
    }

    fn decode_set_of<D: Decode + Eq + core::hash::Hash>(
        &mut self,
        tag: Tag,
        _: Constraints,
    ) -> Result<types::SetOf<D>, Self::Error> {
        self.parse_constructed_contents(tag, true, |decoder| {
            let mut items = types::SetOf::new();

            while let Ok(item) = D::decode(decoder) {
                items.insert(item);
            }

            Ok(items)
        })
    }

    fn decode_sequence<
        const RC: usize,
        const EC: usize,
        D: crate::types::Constructed<RC, EC>,
        DF: FnOnce() -> D,
        F: FnOnce(&mut Self) -> Result<D>,
    >(
        &mut self,
        tag: Tag,
        default_initializer_fn: Option<DF>,
        decode_fn: F,
    ) -> Result<D> {
        self.parse_constructed_contents(tag, true, |decoder| {
            // If there are no fields, or the input is empty and we know that
            // all fields are optional or default fields, we call the default
            // initializer and skip calling the decode function at all.
            if D::FIELDS.is_empty() && D::EXTENDED_FIELDS.is_none()
                || (D::FIELDS.len() == D::FIELDS.number_of_optional_and_default_fields()
                    && decoder.input.is_empty())
            {
                if let Some(default_initializer_fn) = default_initializer_fn {
                    return Ok((default_initializer_fn)());
                }
                return Err(DecodeError::from_kind(
                    DecodeErrorKind::UnexpectedEmptyInput,
                    decoder.codec(),
                ));
            }
            (decode_fn)(decoder)
        })
    }

    fn decode_explicit_prefix<D: Decode>(&mut self, tag: Tag) -> Result<D> {
        self.parse_constructed_contents(tag, false, D::decode)
    }
    fn decode_optional_with_explicit_prefix<D: Decode>(
        &mut self,
        tag: Tag,
    ) -> Result<Option<D>, Self::Error> {
        self.decode_explicit_prefix(tag)
            .map(Some)
            .or_else(|_| Ok(None))
    }

    fn decode_set<const RL: usize, const EL: usize, FIELDS, SET, D, F>(
        &mut self,
        tag: Tag,
        _decode_fn: D,
        field_fn: F,
    ) -> Result<SET, Self::Error>
    where
        SET: Decode + crate::types::Constructed<RL, EL>,
        FIELDS: Decode,
        D: Fn(&mut Self, usize, Tag) -> Result<FIELDS, Self::Error>,
        F: FnOnce(Vec<FIELDS>) -> Result<SET, Self::Error>,
    {
        self.parse_constructed_contents(tag, true, |decoder| {
            let mut fields = Vec::new();

            while let Ok(value) = FIELDS::decode(decoder) {
                fields.push(value);
            }

            (field_fn)(fields)
        })
    }

    fn decode_optional<D: Decode>(&mut self) -> Result<Option<D>, Self::Error> {
        if D::TAG == Tag::EOC {
            Ok(D::decode(self).ok())
        } else {
            self.decode_optional_with_tag(D::TAG)
        }
    }

    /// Decode an the optional value in a `SEQUENCE` or `SET` with `tag`.
    /// Passing the correct tag is required even when used with codecs where
    /// the tag is not present.
    fn decode_optional_with_tag<D: Decode>(&mut self, tag: Tag) -> Result<Option<D>, Self::Error> {
        Ok(D::decode_with_tag(self, tag).ok())
    }

    fn decode_optional_with_constraints<D: Decode>(
        &mut self,
        constraints: Constraints,
    ) -> Result<Option<D>, Self::Error> {
        Ok(D::decode_with_constraints(self, constraints).ok())
    }

    fn decode_optional_with_tag_and_constraints<D: Decode>(
        &mut self,
        tag: Tag,
        constraints: Constraints,
    ) -> Result<Option<D>, Self::Error> {
        Ok(D::decode_with_tag_and_constraints(self, tag, constraints).ok())
    }

    fn decode_choice<D>(&mut self, _: Constraints) -> Result<D, Self::Error>
    where
        D: crate::types::DecodeChoice,
    {
        let (_, identifier) = parser::parse_identifier_octet(self.input).map_err(|e| match e {
            ParseNumberError::Nom(e) => DecodeError::map_nom_err(e, self.codec()),
            ParseNumberError::Overflow => DecodeError::integer_overflow(32u32, self.codec()),
        })?;
        D::from_tag(self, identifier.tag)
    }

    fn decode_extension_addition_with_explicit_tag_and_constraints<D>(
        &mut self,
        tag: Tag,
        _constraints: Constraints,
    ) -> core::result::Result<Option<D>, Self::Error>
    where
        D: Decode,
    {
        self.decode_explicit_prefix(tag).map(Some)
    }

    fn decode_extension_addition_with_tag_and_constraints<D>(
        &mut self,
        tag: Tag,
        // Constraints are irrelevant using BER
        _: Constraints,
    ) -> core::result::Result<Option<D>, Self::Error>
    where
        D: Decode,
    {
        <Option<D>>::decode_with_tag(self, tag)
    }

    fn decode_extension_addition_group<
        const RL: usize,
        const EL: usize,
        D: Decode + crate::types::Constructed<RL, EL>,
    >(
        &mut self,
    ) -> Result<Option<D>, Self::Error> {
        <Option<D>>::decode(self)
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    #[derive(Clone, Copy, Hash, Debug, PartialEq)]
    struct C2;
    impl AsnType for C2 {
        const TAG: Tag = Tag::new(Class::Context, 2);
    }

    #[derive(Clone, Copy, Hash, Debug, PartialEq)]
    struct A3;
    impl AsnType for A3 {
        const TAG: Tag = Tag::new(Class::Application, 3);
    }

    #[derive(Clone, Copy, Hash, Debug, PartialEq)]
    struct A7;
    impl AsnType for A7 {
        const TAG: Tag = Tag::new(Class::Application, 7);
    }

    use super::*;
    use crate::types::*;

    fn decode<T: crate::Decode>(input: &[u8]) -> Result<T, DecodeError> {
        let mut decoder = self::Decoder::new(input, self::DecoderOptions::ber());
        match T::decode(&mut decoder) {
            Ok(result) => {
                assert_eq!(decoder.decoded_len(), input.len());
                Ok(result)
            }
            Err(e) => Err(e),
        }
    }

    #[test]
    fn boolean() {
        assert!(decode::<bool>(&[0x01, 0x01, 0xff]).unwrap());
        assert!(!decode::<bool>(&[0x01, 0x01, 0x00]).unwrap());
    }

    #[test]
    fn tagged_boolean() {
        assert_eq!(
            Explicit::<C2, _>::new(true),
            decode(&[0xa2, 0x03, 0x01, 0x01, 0xff]).unwrap()
        );
    }

    #[test]
    fn integer() {
        assert_eq!(
            32768,
            decode::<i32>(&[0x02, 0x03, 0x00, 0x80, 0x00,]).unwrap()
        );
        assert_eq!(32767, decode::<i32>(&[0x02, 0x02, 0x7f, 0xff]).unwrap());
        assert_eq!(256, decode::<i16>(&[0x02, 0x02, 0x01, 0x00]).unwrap());
        assert_eq!(255, decode::<i16>(&[0x02, 0x02, 0x00, 0xff]).unwrap());
        assert_eq!(128, decode::<i16>(&[0x02, 0x02, 0x00, 0x80]).unwrap());
        assert_eq!(127, decode::<i8>(&[0x02, 0x01, 0x7f]).unwrap());
        assert_eq!(1, decode::<i8>(&[0x02, 0x01, 0x01]).unwrap());
        assert_eq!(0, decode::<i8>(&[0x02, 0x01, 0x00]).unwrap());
        assert_eq!(-1, decode::<i8>(&[0x02, 0x01, 0xff]).unwrap());
        assert_eq!(-128, decode::<i16>(&[0x02, 0x01, 0x80]).unwrap());
        assert_eq!(-129i16, decode::<i16>(&[0x02, 0x02, 0xff, 0x7f]).unwrap());
        assert_eq!(-256i16, decode::<i16>(&[0x02, 0x02, 0xff, 0x00]).unwrap());
        assert_eq!(-32768i32, decode::<i32>(&[0x02, 0x02, 0x80, 0x00]).unwrap());
        assert_eq!(
            -32769i32,
            decode::<i32>(&[0x02, 0x03, 0xff, 0x7f, 0xff]).unwrap()
        );

        let mut data = [0u8; 261];
        data[0] = 0x02;
        data[1] = 0x82;
        data[2] = 0x01;
        data[3] = 0x01;
        data[4] = 0x01;
        let mut bigint = num_bigint::BigInt::from(1);
        bigint <<= 2048;
        assert_eq!(bigint, decode::<num_bigint::BigInt>(&data).unwrap());
    }

    #[test]
    fn octet_string() {
        let octet_string = types::OctetString::from(alloc::vec![1, 2, 3, 4, 5, 6]);
        let primitive_encoded = &[0x4, 0x6, 1, 2, 3, 4, 5, 6];
        let constructed_encoded = &[0x24, 0x80, 0x4, 0x4, 1, 2, 3, 4, 0x4, 0x2, 5, 6, 0x0, 0x0];

        assert_eq!(
            octet_string,
            decode::<types::OctetString>(primitive_encoded).unwrap()
        );
        assert_eq!(
            octet_string,
            decode::<types::OctetString>(constructed_encoded).unwrap()
        );
    }

    #[test]
    fn bit_string() {
        let mut bitstring =
            types::BitString::from_vec([0x0A, 0x3B, 0x5F, 0x29, 0x1C, 0xD0][..].to_owned());
        bitstring.truncate(bitstring.len() - 4);

        let primitive_encoded: types::BitString =
            decode(&[0x03, 0x07, 0x04, 0x0A, 0x3B, 0x5F, 0x29, 0x1C, 0xD0][..]).unwrap();

        let constructed_encoded: types::BitString = decode(
            &[
                0x23, 0x80, // TAG + LENGTH
                0x03, 0x03, 0x00, 0x0A, 0x3B, // Part 1
                0x03, 0x05, 0x04, 0x5F, 0x29, 0x1C, 0xD0, // Part 2
                0x00, 0x00, // EOC
            ][..],
        )
        .unwrap();

        assert_eq!(bitstring, primitive_encoded);
        assert_eq!(bitstring, constructed_encoded);

        let empty_bitstring_primitive_encoded: types::BitString =
            decode(&[0x03, 0x01, 0x00][..]).unwrap();
        assert_eq!(
            types::BitString::from_vec(vec![]),
            empty_bitstring_primitive_encoded
        );

        assert!(decode::<types::BitString>(&[0x03, 0x00][..]).is_err());
    }

    #[test]
    fn utf8_string() {
        let name = String::from("Jones");
        let primitive = &[0x0C, 0x05, 0x4A, 0x6F, 0x6E, 0x65, 0x73];
        let definite_constructed = &[
            0x2C, 0x09, // TAG + LENGTH
            0x04, 0x03, // PART 1 TLV
            0x4A, 0x6F, 0x6E, 0x04, 0x02, // PART 2 TLV
            0x65, 0x73,
        ];
        let indefinite_constructed = &[
            0x2C, 0x80, // TAG + LENGTH
            0x04, 0x03, // PART 1 TLV
            0x4A, 0x6F, 0x6E, 0x04, 0x02, // PART 2 TLV
            0x65, 0x73, 0x00, 0x00,
        ];

        assert_eq!(name, decode::<String>(primitive).unwrap());
        assert_eq!(name, decode::<String>(definite_constructed).unwrap());
        assert_eq!(name, decode::<String>(indefinite_constructed).unwrap());
    }

    #[test]
    fn utc_time() {
        let time =
            crate::types::GeneralizedTime::parse_from_str("991231235959+0000", "%y%m%d%H%M%S%z")
                .unwrap();
        // 991231235959Z
        let has_z = &[
            0x17, 0x0D, 0x39, 0x39, 0x31, 0x32, 0x33, 0x31, 0x32, 0x33, 0x35, 0x39, 0x35, 0x39,
            0x5A,
        ];
        // 991231235959+0000
        let has_noz = &[
            0x17, 0x11, 0x39, 0x39, 0x31, 0x32, 0x33, 0x31, 0x32, 0x33, 0x35, 0x39, 0x35, 0x39,
            0x2B, 0x30, 0x30, 0x30, 0x30,
        ];
        assert_eq!(
            time,
            decode::<chrono::DateTime::<chrono::Utc>>(has_z).unwrap()
        );

        assert_eq!(
            time,
            crate::der::decode::<crate::types::UtcTime>(has_z).unwrap()
        );

        assert_eq!(
            time,
            decode::<chrono::DateTime::<chrono::Utc>>(has_noz).unwrap()
        );
        assert!(crate::der::decode::<crate::types::UtcTime>(has_noz).is_err());
    }

    #[test]
    fn generalized_time() {
        let time = crate::types::GeneralizedTime::parse_from_str(
            "20001231205959.999+0000",
            "%Y%m%d%H%M%S%.3f%z",
        )
        .unwrap();
        let has_z = &[
            0x18, 0x13, 0x32, 0x30, 0x30, 0x30, 0x31, 0x32, 0x33, 0x31, 0x32, 0x30, 0x35, 0x39,
            0x35, 0x39, 0x2E, 0x39, 0x39, 0x39, 0x5A,
        ];
        assert_eq!(
            time,
            decode::<chrono::DateTime::<chrono::FixedOffset>>(has_z).unwrap()
        );
    }

    #[test]
    fn sequence_of() {
        let vec = alloc::vec!["Jon", "es"];
        let from_raw: Vec<String> = decode(
            &[
                0x30, 0x9, 0x0C, 0x03, 0x4A, 0x6F, 0x6E, 0x0C, 0x02, 0x65, 0x73,
            ][..],
        )
        .unwrap();

        assert_eq!(vec, from_raw);
    }

    #[test]
    fn sequence() {
        use types::Ia5String;
        // Taken from examples in 8.9 of X.690.
        #[derive(Debug, PartialEq)]
        struct Foo {
            name: Ia5String,
            ok: bool,
        }

        impl types::Constructed<2, 0> for Foo {
            const FIELDS: types::fields::Fields<2> = types::fields::Fields::from_static([
                types::fields::Field::new_required(0, Ia5String::TAG, Ia5String::TAG_TREE, "name"),
                types::fields::Field::new_required(1, bool::TAG, bool::TAG_TREE, "ok"),
            ]);
        }

        impl types::AsnType for Foo {
            const TAG: Tag = Tag::SEQUENCE;
        }

        impl Decode for Foo {
            fn decode_with_tag_and_constraints<D: crate::Decoder>(
                decoder: &mut D,
                tag: Tag,
                _: Constraints,
            ) -> Result<Self, D::Error> {
                decoder.decode_sequence(tag, None::<fn() -> Self>, |sequence| {
                    let name: Ia5String = Ia5String::decode(sequence)?;
                    let ok: bool = bool::decode(sequence)?;
                    Ok(Self { name, ok })
                })
            }
        }

        let foo = Foo {
            name: String::from("Smith").try_into().unwrap(),
            ok: true,
        };
        let bytes = &[
            0x30, 0x0A, // TAG + LENGTH
            0x16, 0x05, 0x53, 0x6d, 0x69, 0x74, 0x68, // Ia5String "Smith"
            0x01, 0x01, 0xff, // BOOL True
        ];

        assert_eq!(foo, decode(bytes).unwrap());
    }

    #[test]
    fn tagging() {
        type Type1 = VisibleString;
        type Type2 = Implicit<A3, Type1>;
        type Type3 = Explicit<C2, Type2>;
        type Type4 = Implicit<A7, Type3>;
        type Type5 = Implicit<C2, Type2>;

        let jones = String::from("Jones");
        let jones1 = Type1::try_from(jones).unwrap();
        let jones2 = Type2::from(jones1.clone());
        let jones3 = Type3::from(jones2.clone());
        let jones4 = Type4::from(jones3.clone());
        let jones5 = Type5::from(jones2.clone());

        assert_eq!(
            jones1,
            decode(&[0x1A, 0x05, 0x4A, 0x6F, 0x6E, 0x65, 0x73]).unwrap()
        );
        assert_eq!(
            jones2,
            decode(&[0x43, 0x05, 0x4A, 0x6F, 0x6E, 0x65, 0x73]).unwrap()
        );
        assert_eq!(
            jones3,
            decode(&[0xa2, 0x07, 0x43, 0x5, 0x4A, 0x6F, 0x6E, 0x65, 0x73]).unwrap()
        );
        assert_eq!(
            jones4,
            decode(&[0x67, 0x07, 0x43, 0x5, 0x4A, 0x6F, 0x6E, 0x65, 0x73]).unwrap()
        );
        assert_eq!(
            jones5,
            decode(&[0x82, 0x05, 0x4A, 0x6F, 0x6E, 0x65, 0x73]).unwrap()
        );
    }

    #[test]
    fn flip1() {
        let _ = decode::<Open>(&[
            0x10, 0x10, 0x23, 0x00, 0xfe, 0x7f, 0x10, 0x03, 0x00, 0xff, 0xe4, 0x04, 0x50, 0x10,
            0x50, 0x10, 0x10, 0x10,
        ]);
    }

    #[test]
    fn any() {
        let expected = &[0x1A, 0x05, 0x4A, 0x6F, 0x6E, 0x65, 0x73];
        assert_eq!(
            Any {
                contents: expected.to_vec()
            },
            decode(expected).unwrap()
        );
    }

    #[test]
    fn any_indefinite() {
        let any = &[
            0x30, 0x80, 0x2C, 0x80, 0x04, 0x03, 0x4A, 0x6F, 0x6E, 0x04, 0x02, 0x65, 0x73, 0x00,
            0x00, 0x00, 0x00,
        ];
        assert_eq!(
            Any {
                contents: any.to_vec()
            },
            decode(any).unwrap(),
        );
    }

    #[test]
    fn any_indefinite_fail_no_eoc() {
        let any = &[
            0x30, 0x80, 0x2C, 0x80, 0x04, 0x03, 0x4A, 0x6F, 0x6E, 0x04, 0x02, 0x65, 0x73, 0x00,
            0x00,
        ];
        assert!(decode::<Any>(any).is_err());
    }

    #[test]
    fn decoding_oid() {
        use crate::Decoder;

        let mut decoder =
            super::Decoder::new(&[0x06, 0x03, 0x88, 0x37, 0x01], DecoderOptions::der());
        let oid = decoder.decode_object_identifier(Tag::OBJECT_IDENTIFIER);
        assert!(oid.is_ok());
        let oid = oid.unwrap();
        assert_eq!(ObjectIdentifier::new([2, 999, 1].to_vec()).unwrap(), oid);
    }
}
