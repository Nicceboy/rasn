//! This module contains collections types that can be used for fuzzing.
//! They help on reaching the most branches of the code with feedback-driven fuzzing.
//!
//! Currently they have been created on COER/OER codec on mind,
//! so especialy some extension variants and constraints are missing.
//!
//!
#![allow(clippy::no_effect_underscore_binding)]

use rasn::prelude::*;

// On top of unconstrained `Integer`, we also test some value constrained types.
type ConstrainedNonNegativeInteger = ConstrainedInteger<0, { u64::MAX as i128 }>;
type ConstrainedSigned = ConstrainedInteger<{ i64::MIN as i128 }, { i64::MAX as i128 }>;
type ConstrainedNonPositiveInteger = ConstrainedInteger<{ i64::MIN as i128 }, 0>;

// Variations of BitString

#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq, Eq)]
#[rasn(delegate, size("12"))]
pub struct SingleSizeConstrainedBitString(pub BitString);

// No effect in OER/COER
#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq, Eq)]
#[rasn(delegate, size("0..12"))]
pub struct RangeSizeConstrainedBitString(pub BitString);

// Variations of OctetString
// TODO permitted alphabet constraints

#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq, Eq)]
#[rasn(delegate, size("12"))]
pub struct SingleSizeConstrainedOctectString(pub OctetString);

#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq, Eq)]
#[rasn(delegate, size("0..12"))]
pub struct RangeSizeConstrainedOctectString(pub OctetString);

// Variations of OctetString

#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq, Eq)]
#[rasn(delegate, size("12"))]
pub struct SingleSizeConstrainedUtf8String(pub Utf8String);
#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq, Eq)]
#[rasn(delegate, size("0..12"))]
pub struct RangeSizeConstrainedUtf8String(pub Utf8String);

#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq, Eq)]
#[rasn(automatic_tags)]
pub struct Integers {
    pub int1: Integer,
    pub int2: ConstrainedNonNegativeInteger,
    pub int3: ConstrainedSigned,
    pub int4: ConstrainedNonPositiveInteger,
}
// value Rocket ::= {
//   name "Falcon",
//   -- use default for the message --
//   fuel solid,
//   speed mph : 18000,
//   payload {
//     "Car",
//     "GPS"
//   }
// }
#[derive(AsnType, Decode, Encode, Clone, Copy, Debug, PartialEq, Eq)]
#[rasn(enumerated, automatic_tags)]
pub enum Fuel {
    Solid,
    Liquid,
    Gas,
}

#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq, Eq)]
#[rasn(automatic_tags)]
pub struct Speed {
    pub mph: Integer,
}

#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq, Eq)]
#[rasn(automatic_tags)]
pub struct Rocket {
    pub name: Utf8String,
    pub fuel: Fuel,
    pub speed: Speed,
    pub payload: Vec<Utf8String>,
}

#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq, Eq)]
#[rasn(automatic_tags)]
pub struct Strings {
    pub str1: Utf8String,
    pub str3: BitString,
    pub str5: OctetString,
}

#[derive(AsnType, Decode, Encode, Clone, Copy, Debug, PartialEq, Eq)]
#[rasn(enumerated, automatic_tags)]
pub enum Enum1 {
    Value1 = -1,
    Value2 = 0,
    Value3 = 1,
}
#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq, Eq)]
#[rasn(choice, automatic_tags)]
pub enum Choice1 {
    Value1(Integer),
    Value2(usize),
    Value3(Utf8String),
    Value4(SingleSizeConstrainedBitString),
}

#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq, Eq)]
#[rasn(automatic_tags)]
pub struct Sequence1 {
    pub int1: Integers,
    pub str5: Strings,
    pub enum1: Enum1,
    pub choice1: Choice1,
}

#[allow(unused_macros)]
macro_rules! populate {
    ($codec:ident, $asn_typ:expr, $typ:ty, $value:expr) => {{
        let value: $typ = $value;
        let actual_encoding = rasn::$codec::encode(&value).unwrap();
        let decoded_value: $typ = rasn::$codec::decode(&actual_encoding).unwrap();
        pretty_assertions::assert_eq!(value, decoded_value);
        // Generate seeds for fuzzing, based on type
        let generate_seeds: u8 = std::env::var("RASN_FUZZ_SEEDS")
            .ok()
            .map_or(0, |v| v.parse().unwrap_or(0));
        if generate_seeds > 0 {
            let fp = format!(
                "in/{}/{}/{}.bin",
                stringify!($codec),
                $asn_typ,
                stringify!($typ)
            );
            let fp = std::path::Path::new(&fp);
            if let Some(parent) = fp.parent() {
                std::fs::create_dir_all(parent).unwrap();
                // Write the data to the file
                std::fs::write(fp, &actual_encoding).unwrap();
            }
        }
    }};
}

// Enum about all ASN.1 types
// Avoid conflicts with the `types` module
enum ASN1Types {
    Integer,
    BitString,
    Choice,
    Enum,
    Sequence,
    String,
    OctetString,
}
// impl Display for ASN1Types, lowercase
impl std::fmt::Display for ASN1Types {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            ASN1Types::Integer => "integer",
            ASN1Types::BitString => "bitstring",
            ASN1Types::String => "string",
            ASN1Types::Sequence => "sequence",
            ASN1Types::Enum => "enum",
            ASN1Types::Choice => "choice",
            ASN1Types::OctetString => "octetstring",
        };
        write!(f, "{name}")
    }
}

mod tests {
    use super::{
        ASN1Types, BitString, Choice1, ConstrainedInteger, Enum1, Fuel, Integer, Integers,
        OctetString, Rocket, Sequence1, SingleSizeConstrainedBitString, Speed, Strings, Utf8String,
    };
    use std::iter::FromIterator;
    #[test]
    fn test_coer() {
        let _rocket: Rocket = Rocket {
            name: "Falcon".to_string(),
            fuel: Fuel::Solid,
            speed: Speed {
                mph: Integer::from(18000),
            },
            payload: vec!["Car".to_string(), "GPS".to_string()],
        };
        let data: Sequence1 = Sequence1 {
            int1: Integers {
                int1: 1.into(),
                int2: 2.into(),
                int3: 3.into(),
                int4: ConstrainedInteger::from(-4),
            },
            str5: Strings {
                str1: Utf8String::from("str1"),
                // str2: "str2".to_string(),
                str3: BitString::from_slice(&[0, 1, 1, 1, 1, 0]),
                // str4: BitString::from_bytes(&[0x01, 0x02, 0x03, 0x04]),
                str5: OctetString::from_static(&[0x01, 0x02, 0x03, 0x04]),
                // str6: OctetString::from_bytes(&[0x01, 0x02, 0x03, 0x04]),
            },
            enum1: Enum1::Value1,
            choice1: Choice1::Value1(1.into()),
        };
        populate!(coer, ASN1Types::Sequence, Sequence1, data);
    }
    #[test]
    fn test_bitstring() {
        let data1: BitString = [
            false, false, true, false, true, true, false, true, true, false, true, true,
        ]
        .iter()
        .collect::<BitString>();
        populate!(coer, ASN1Types::BitString, BitString, data1.clone());
        let data2: SingleSizeConstrainedBitString = SingleSizeConstrainedBitString(data1);
        populate!(
            coer,
            ASN1Types::BitString,
            SingleSizeConstrainedBitString,
            data2
        );
    }
}

// temci short shell --sudo
