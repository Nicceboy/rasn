//! This module contains collections types that can be used for fuzzing.
//! They help on reaching the most branches of the code with feedback-driven fuzzing.
//!
//! Currently they have been created on COER/OER codec on mind,
//! so especialy some extension variants and constraints are missing.
//!
//!
#![allow(clippy::no_effect_underscore_binding)]

use crate::{debug_bytes, debug_object, LOGGER};
use log::{error, info, warn, Level, LevelFilter, Metadata, Record};
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
#[rasn(choice, automatic_tags)]
pub enum ChoiceInChoice {
    Value1(Choice1),
    Value2(Choice1),
}

#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq, Eq)]
#[rasn(automatic_tags)]
pub struct Sequence1 {
    pub int1: Integers,
    pub str5: Strings,
    pub enum1: Enum1,
    pub choice1: Choice1,
}
#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq, Eq)]
pub struct SequenceOptionals {
    #[rasn(tag(explicit(0)))]
    pub is: Integer,
    #[rasn(tag(explicit(1)))]
    pub late: Option<OctetString>,
    #[rasn(tag(explicit(2)))]
    pub today: Option<Integer>,
}

#[derive(AsnType, Debug, Clone, Decode, Encode, PartialEq, PartialOrd, Eq, Ord, Hash)]
#[rasn(delegate, size("3"))]
pub struct HashedId3(pub OctetString);

#[derive(AsnType, Debug, Decode, Encode, Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
#[rasn(delegate, automatic_tags)]
pub struct Uint16(u16);

#[derive(AsnType, Debug, Decode, Encode, Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
#[rasn(automatic_tags)]
#[non_exhaustive]
pub struct MissingCrlIdentifier {
    pub craca_id: HashedId3,
    pub crl_series: Uint16,
}
#[derive(AsnType, Debug, Decode, Encode, Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
#[rasn(automatic_tags)]
// #[non_exhaustive]
pub struct ExtendedOptions {
    // pub a: HashedId3,
    pub b: Option<HashedId3>,
    // pub c: Option<MissingCrlIdentifier>,
    // #[rasn(extension_addition)]
    // pub d: Option<SequenceOf<HashedId3>>,
}

#[allow(unused_macros)]
macro_rules! populate {
    ($codec:ident, $asn_typ:expr, $typ:ty, $value:expr, $case:expr) => {{
        let value: $typ = $value;
        let actual_encoding = rasn::$codec::encode(&value).unwrap();
        debug_bytes(&actual_encoding, stringify!($codec));
        let decoded_value: $typ = rasn::$codec::decode(&actual_encoding).unwrap();
        debug_object(&decoded_value, stringify!($codec));
        pretty_assertions::assert_eq!(value, decoded_value);
        // Generate seeds for fuzzing, based on type
        let generate_seeds: u8 = std::env::var("RASN_FUZZ_SEEDS")
            .ok()
            .map_or(0, |v| v.parse().unwrap_or(0));
        if generate_seeds > 0 {
            // Format is in/{codec}/{asn_typ}/{subtype}/{case}.bin
            let fp = format!(
                "in/{}/{}/{}/data{}.bin",
                stringify!($codec),
                $asn_typ,
                stringify!($typ),
                $case
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
    Ia5String,
    VisibleString,
    Choice,
    Enum,
    Sequence,
    Utf8String,
    OctetString,
    ObjectIdentifier,
}
// impl Display for ASN1Types, lowercase
impl std::fmt::Display for ASN1Types {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let name = match self {
            ASN1Types::Integer => "integer",
            ASN1Types::BitString => "bitstring",
            ASN1Types::Ia5String => "ia5string",
            ASN1Types::VisibleString => "visiblestring",
            ASN1Types::Utf8String => "utf8string",
            ASN1Types::Sequence => "sequence",
            ASN1Types::Enum => "enum",
            ASN1Types::Choice => "choice",
            ASN1Types::OctetString => "octetstring",
            ASN1Types::ObjectIdentifier => "objectidentifier",
        };
        write!(f, "{name}")
    }
}

mod tests {
    use rasn::{
        examples::personnel::{
            ExtensiblePersonnelRecord, PersonnelRecord, PersonnelRecordWithConstraints,
        },
        types::{ObjectIdentifier, Oid},
    };

    use super::*;

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
        populate!(coer, ASN1Types::Sequence, Sequence1, data, 1);
    }
    #[test]
    fn test_bitstring() {
        let data1: BitString = [
            false, false, true, false, true, true, false, true, true, false, true, true,
        ]
        .iter()
        .collect::<BitString>();
        populate!(coer, ASN1Types::BitString, BitString, data1.clone(), 1);
        let data2: SingleSizeConstrainedBitString = SingleSizeConstrainedBitString(data1);
        populate!(
            coer,
            ASN1Types::BitString,
            SingleSizeConstrainedBitString,
            data2,
            1
        );
    }
    #[test]
    fn test_choice() {
        let data1: Choice1 = Choice1::Value1(1.into());
        populate!(coer, ASN1Types::Choice, Choice1, data1, 1);
        let data2: Choice1 = Choice1::Value2(2);
        populate!(coer, ASN1Types::Choice, Choice1, data2, 2);
        let data3: Choice1 = Choice1::Value3("str".to_string());
        populate!(coer, ASN1Types::Choice, Choice1, data3, 3);
        let data4: Choice1 = Choice1::Value4(SingleSizeConstrainedBitString(
            [
                false, false, true, false, true, true, false, true, true, false, true, true,
            ]
            .iter()
            .collect::<BitString>(),
        ));
        populate!(coer, ASN1Types::Choice, Choice1, data4, 4);
    }
    #[test]
    fn test_choice_in_choice() {
        let data1: Choice1 = Choice1::Value3("dang".to_string());
        let data2: ChoiceInChoice = ChoiceInChoice::Value1(data1);
        populate!(coer, ASN1Types::Choice, ChoiceInChoice, data2, 1);
    }
    #[test]
    fn test_object_identifier() {
        // 1.2.34567.88
        let data = Oid::const_new(&[1, 2, 34567, 88]);
        populate!(
            coer,
            ASN1Types::ObjectIdentifier,
            ObjectIdentifier,
            data.into(),
            1
        );
    }
    #[test]
    fn test_sequence_optional() {
        let data = SequenceOptionals {
            is: 1.into(),
            late: Some(OctetString::from_static(&[0x01, 0x02, 0x03, 0x04])),
            today: Some(1.into()),
        };
        populate!(coer, ASN1Types::Sequence, SequenceOptionals, data, 1);
    }
    #[test]
    fn test_extended_options() {
        log::set_logger(&LOGGER);
        log::set_max_level(LevelFilter::Debug);
        let data = ExtendedOptions {
            // a: HashedId3(OctetString::from_static(&[0x01, 0x02, 0x03])),
            b: Some(HashedId3(OctetString::from_static(&[0x06, 0x07, 0x08]))),
            // c: Some(MissingCrlIdentifier {
            //     craca_id: HashedId3(OctetString::from_static(&[0x01, 0x02, 0x03])),
            //     crl_series: Uint16(15),
            // }),
            // d: None, // d: Some(SequenceOf::from(vec![
            //     HashedId3(OctetString::from_static(&[0x01, 0x02, 0x03])),
            //     HashedId3(OctetString::from_static(&[0x01, 0x02, 0x03])),
            // ])),
        };
        populate!(coer, ASN1Types::Sequence, ExtendedOptions, data, 1);
        // let data2 = ExtendedOptions {
        //     a: HashedId3(OctetString::from_static(&[0x11, 0x22, 0x33])),
        //     b: None,
        // c: Some(MissingCrlIdentifier {
        //     craca_id: HashedId3(OctetString::from_static(&[0x11, 0x22, 0x33])),
        //     crl_series: Uint16(1),
        // }),
        // d: None,
        // };
        // populate!(coer, ASN1Types::Sequence, ExtendedOptions, data2, 2);
    }
    #[test]
    fn test_personnel_record() {
        let data = PersonnelRecord::default();
        populate!(coer, ASN1Types::Sequence, PersonnelRecord, data, 1);
    }
    #[test]
    fn test_personnel_record_with_constraints() {
        let data = PersonnelRecordWithConstraints::default();
        populate!(
            coer,
            ASN1Types::Sequence,
            PersonnelRecordWithConstraints,
            data,
            1
        );
    }
    #[test]
    fn test_personnel_record_extensible() {
        let data = ExtensiblePersonnelRecord::default();
        populate!(
            coer,
            ASN1Types::Sequence,
            ExtensiblePersonnelRecord,
            data,
            1
        );
    }
}

// temci short shell --sudo
