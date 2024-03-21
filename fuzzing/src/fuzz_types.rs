//! This module contains collections types that are used for fuzzing.
//! They help on reaching the most branches of the code with feedback-driven fuzzing.
//!
//! Currently they have been created on COER/OER codec on mind,
//! so especialy some extension variants and constraints are missing.
//!
//!

use rasn::prelude::*;

// On top of unconstrained `Integer`, we also test some value constrained types.
type IntegerA = ConstrainedInteger<0, { u64::MAX as i128 }>;
type IntegerB = ConstrainedInteger<{ i64::MIN as i128 }, { i64::MAX as i128 }>;
type IntegerC = ConstrainedInteger<{ i64::MIN as i128 }, 0>;

// Variations of BitString

#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq)]
#[rasn(delegate, size("12"))]
pub struct SingleSizeConstrainedBitString(pub BitString);

// No effect in OER/COER
#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq)]
#[rasn(delegate, size("0..12"))]
pub struct RangeSizeConstrainedBitString(pub BitString);

// Variations of OctetString
// TODO permitted alphabet constraints

#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq)]
#[rasn(delegate, size("12"))]
pub struct SingleSizeConstrainedOctectString(pub OctetString);

#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq)]
#[rasn(delegate, size("0..12"))]
pub struct RangeSizeConstrainedOctectString(pub OctetString);

// Variations of OctetString

#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq)]
#[rasn(delegate, size("12"))]
pub struct SingleSizeConstrainedUtf8String(pub Utf8String);
#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq)]
#[rasn(delegate, size("0..12"))]
pub struct RangeSizeConstrainedUtf8String(pub Utf8String);

#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq)]
#[rasn(automatic_tags)]
pub struct Integers {
    pub int1: Integer,
    pub int2: IntegerA,
    pub int3: IntegerB,
    pub int4: IntegerC,
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
#[derive(AsnType, Decode, Encode, Clone, Copy, Debug, PartialEq)]
#[rasn(enumerated, automatic_tags)]
pub enum Fuel {
    Solid,
    Liquid,
    Gas,
}

#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq)]
#[rasn(automatic_tags)]
pub struct Speed {
    pub mph: Integer,
}

#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq)]
#[rasn(automatic_tags)]
pub struct Rocket {
    pub name: Utf8String,
    pub fuel: Fuel,
    pub speed: Speed,
    pub payload: Vec<Utf8String>,
}

let rocket = Rocket {
    name: "Falcon".to_string(),
    fuel: Fuel::Solid,
    speed: Speed { mph: 18000 },
    payload: vec!["Car".to_string(), "GPS".to_string()],
};

#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq)]
#[rasn(automatic_tags)]
pub struct Strings {
    pub str1: Utf8String,
    pub str3: BitString,
    pub str5: OctetString,
}

#[derive(AsnType, Decode, Encode, Clone, Copy, Debug, PartialEq)]
#[rasn(enumerated, automatic_tags)]
pub enum Enum1 {
    Value1 = -1,
    Value2 = 0,
    Value3 = 1,
}
#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq)]
#[rasn(choice, automatic_tags)]
pub enum Choice1 {
    Value1(Integer),
    Value2(usize),
    Value3(Utf8String),
    Value4(ConstrainedOctectString),
}

#[derive(AsnType, Decode, Encode, Clone, Debug, PartialEq)]
#[rasn(automatic_tags)]
pub struct Sequence1 {
    pub int1: Integers,
    pub str5: Strings,
    pub enum1: Enum1,
    pub choice1: Choice1,
}

mod tests {
    use super::*;
    #[test]
    fn test_coer() {
        let data: Sequence1 = Sequence1 {
            int1: Integers {
                int1: 1,
                int2: 2,
                int3: 3,
                int4: 4,
            },
            str5: Strings {
                str1: "str1".to_string(),
                str2: "str2".to_string(),
                str3: BitString::from_bytes(&[0x01, 0x02, 0x03, 0x04]),
                str4: BitString::from_bytes(&[0x01, 0x02, 0x03, 0x04]),
                str5: OctetString::from_bytes(&[0x01, 0x02, 0x03, 0x04]),
                str6: OctetString::from_bytes(&[0x01, 0x02, 0x03, 0x04]),
            },
            enum1: Enum1::Value1,
            choice1: Choice1::Value1(1),
        };
        // fuzz_coer::<Sequence1>( );
    }
}
