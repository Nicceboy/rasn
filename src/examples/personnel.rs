use crate as rasn;
use alloc::{string::String, vec, vec::Vec};
use rasn::prelude::*;
/// Personnel record is typical in ASN.1 examples. It is a sequence of several fields.
/// This example demonstrates how to define a sequence of fields in Rust.
/// This example is used in tests and fuzzing.

#[derive(AsnType, Decode, Debug, PartialEq)]
#[rasn(set, tag(application, 0))]
pub struct PersonnelRecord {
    pub name: Name,
    #[rasn(tag(explicit(0)))]
    pub title: VisibleString,
    pub number: EmployeeNumber,
    #[rasn(tag(explicit(1)))]
    pub date_of_hire: Date,
    #[rasn(tag(explicit(2)))]
    pub name_of_spouse: Name,
    #[rasn(tag(3), default)]
    pub children: Vec<ChildInformation>,
}

impl rasn::Encode for PersonnelRecord {
    fn encode_with_tag_and_constraints<EN: rasn::Encoder>(
        &self,
        encoder: &mut EN,
        tag: rasn::Tag,
        _: rasn::types::Constraints,
    ) -> core::result::Result<(), EN::Error> {
        #[allow(unused)]
        let name = &self.name;
        #[allow(unused)]
        let title = &self.title;
        #[allow(unused)]
        let number = &self.number;
        #[allow(unused)]
        let date_of_hire = &self.date_of_hire;
        #[allow(unused)]
        let name_of_spouse = &self.name_of_spouse;
        #[allow(unused)]
        let children = &self.children;
        encoder
            .encode_set::<Self, _>(tag, |encoder| {
                self.name.encode(encoder)?;
                encoder.encode_explicit_prefix(
                    rasn::Tag::new(rasn::types::Class::Context, 0),
                    &self.title,
                )?;
                self.number.encode(encoder)?;
                encoder.encode_explicit_prefix(
                    rasn::Tag::new(rasn::types::Class::Context, 1),
                    &self.date_of_hire,
                )?;
                encoder.encode_explicit_prefix(
                    rasn::Tag::new(rasn::types::Class::Context, 2),
                    &self.name_of_spouse,
                )?;
                encoder.encode_default_with_tag(
                    rasn::Tag::new(rasn::types::Class::Context, 3),
                    &self.children,
                    <Vec<ChildInformation>>::default,
                )?;
                Ok(())
            })
            .map(drop)
    }
}

impl Default for PersonnelRecord {
    fn default() -> Self {
        Self {
            name: Name::john(),
            title: String::from("Director").try_into().unwrap(),
            number: <_>::default(),
            date_of_hire: Date(String::from("19710917").try_into().unwrap()),
            name_of_spouse: Name::mary(),
            children: vec![ChildInformation::ralph(), ChildInformation::susan()],
        }
    }
}

#[derive(AsnType, Decode, Encode, Debug, PartialEq)]
#[rasn(set)]
pub struct ChildInformation {
    pub name: Name,
    #[rasn(tag(explicit(0)))]
    pub date_of_birth: Date,
}

impl ChildInformation {
    pub fn ralph() -> Self {
        Self {
            name: Name {
                given_name: String::from("Ralph").try_into().unwrap(),
                initial: String::from("T").try_into().unwrap(),
                family_name: String::from("Smith").try_into().unwrap(),
            },
            date_of_birth: Date(String::from("19571111").try_into().unwrap()),
        }
    }

    pub fn susan() -> Self {
        Self {
            name: Name {
                given_name: String::from("Susan").try_into().unwrap(),
                initial: String::from("B").try_into().unwrap(),
                family_name: String::from("Jones").try_into().unwrap(),
            },
            date_of_birth: Date(String::from("19590717").try_into().unwrap()),
        }
    }
}

#[derive(AsnType, Decode, Encode, Debug, PartialEq)]
#[rasn(tag(application, 1))]
pub struct Name {
    pub given_name: VisibleString,
    pub initial: VisibleString,
    pub family_name: VisibleString,
}

impl Name {
    pub fn john() -> Self {
        Self {
            given_name: String::from("John").try_into().unwrap(),
            initial: String::from("P").try_into().unwrap(),
            family_name: String::from("Smith").try_into().unwrap(),
        }
    }

    pub fn mary() -> Self {
        Self {
            given_name: String::from("Mary").try_into().unwrap(),
            initial: String::from("T").try_into().unwrap(),
            family_name: String::from("Smith").try_into().unwrap(),
        }
    }

    pub fn susan() -> Self {
        Self {
            given_name: String::from("Susan").try_into().unwrap(),
            initial: String::from("B").try_into().unwrap(),
            family_name: String::from("Jones").try_into().unwrap(),
        }
    }
}

#[derive(AsnType, Decode, Encode, Debug, PartialEq)]
#[rasn(tag(application, 2), delegate, value("0..=9999", extensible))]
pub struct ExtensibleEmployeeNumber(pub Integer);

impl From<EmployeeNumber> for ExtensibleEmployeeNumber {
    fn from(number: EmployeeNumber) -> Self {
        Self(number.0)
    }
}

#[derive(AsnType, Decode, Encode, Debug, PartialEq)]
#[rasn(tag(application, 2), delegate)]
pub struct EmployeeNumber(pub Integer);

impl Default for EmployeeNumber {
    fn default() -> Self {
        Self(51.into())
    }
}

#[derive(AsnType, Decode, Encode, Debug, PartialEq)]
#[rasn(tag(application, 3), delegate)]
pub struct Date(pub VisibleString);

#[derive(AsnType, Decode, Encode, Debug, PartialEq)]
#[rasn(set, tag(application, 0))]
pub struct PersonnelRecordWithConstraints {
    pub name: NameWithConstraints,
    #[rasn(tag(explicit(0)))]
    pub title: VisibleString,
    pub number: EmployeeNumber,
    #[rasn(tag(explicit(1)))]
    pub date_of_hire: DateWithConstraints,
    #[rasn(tag(explicit(2)))]
    pub name_of_spouse: NameWithConstraints,
    #[rasn(tag(3), default)]
    pub children: Vec<ChildInformationWithConstraints>,
}

impl Default for PersonnelRecordWithConstraints {
    fn default() -> Self {
        PersonnelRecord::default().into()
    }
}

impl From<PersonnelRecord> for PersonnelRecordWithConstraints {
    fn from(record: PersonnelRecord) -> Self {
        Self {
            name: record.name.into(),
            title: record.title,
            number: record.number,
            date_of_hire: record.date_of_hire.into(),
            name_of_spouse: record.name_of_spouse.into(),
            children: record.children.into_iter().map(From::from).collect(),
        }
    }
}

#[derive(AsnType, Decode, Encode, Debug, PartialEq)]
#[rasn(set, tag(application, 0))]
#[non_exhaustive]
pub struct ExtensiblePersonnelRecord {
    pub name: ExtensibleName,
    #[rasn(tag(explicit(0)))]
    pub title: VisibleString,
    pub number: ExtensibleEmployeeNumber,
    #[rasn(tag(explicit(1)))]
    pub date_of_hire: ExtensibleDate,
    #[rasn(tag(explicit(2)))]
    pub name_of_spouse: ExtensibleName,
    #[rasn(tag(3), default, size(2, extensible))]
    pub children: Option<Vec<ExtensibleChildInformation>>,
}

impl Default for ExtensiblePersonnelRecord {
    fn default() -> Self {
        Self {
            name: Name::john().into(),
            title: String::from("Director").try_into().unwrap(),
            number: ExtensibleEmployeeNumber(51.into()),
            date_of_hire: ExtensibleDate(VisibleString::try_from("19710917").unwrap()),
            name_of_spouse: Name::mary().into(),
            children: Some(vec![
                ChildInformation::ralph().into(),
                ExtensibleChildInformation::susan(),
            ]),
        }
    }
}

#[derive(AsnType, Decode, Encode, Debug, PartialEq)]
#[rasn(set)]
#[non_exhaustive]
pub struct ExtensibleChildInformation {
    name: ExtensibleName,
    #[rasn(tag(explicit(0)))]
    date_of_birth: ExtensibleDate,
    #[rasn(extension_addition, tag(1))]
    sex: Option<Sex>,
}

#[derive(AsnType, Decode, Encode, Debug, Clone, Copy, PartialEq)]
#[rasn(enumerated)]
pub enum Sex {
    Male = 1,
    Female = 2,
    Unknown = 3,
}

impl ExtensibleChildInformation {
    pub fn susan() -> Self {
        Self {
            name: Name::susan().into(),
            date_of_birth: ExtensibleDate(String::from("19590717").try_into().unwrap()),
            sex: Some(Sex::Female),
        }
    }
}

impl From<ChildInformation> for ExtensibleChildInformation {
    fn from(info: ChildInformation) -> Self {
        Self {
            name: info.name.into(),
            date_of_birth: info.date_of_birth.into(),
            sex: None,
        }
    }
}

#[derive(AsnType, Decode, Encode, Debug, PartialEq)]
#[rasn(set)]
pub struct ChildInformationWithConstraints {
    name: NameWithConstraints,
    #[rasn(tag(explicit(0)))]
    date_of_birth: DateWithConstraints,
}

impl From<ChildInformation> for ChildInformationWithConstraints {
    fn from(info: ChildInformation) -> Self {
        Self {
            name: info.name.into(),
            date_of_birth: info.date_of_birth.into(),
        }
    }
}

#[derive(AsnType, Decode, Encode, Debug, PartialEq)]
#[rasn(tag(application, 1))]
#[non_exhaustive]
pub struct ExtensibleName {
    pub given_name: ExtensibleNameString,
    #[rasn(size(1))]
    pub initial: ExtensibleNameString,
    pub family_name: ExtensibleNameString,
}

impl From<Name> for ExtensibleName {
    fn from(name: Name) -> Self {
        Self {
            given_name: name.given_name.into(),
            initial: name.initial.into(),
            family_name: name.family_name.into(),
        }
    }
}

#[derive(AsnType, Decode, Encode, Debug, PartialEq)]
#[rasn(tag(application, 1))]
pub struct NameWithConstraints {
    pub given_name: NameString,
    #[rasn(size(1))]
    pub initial: NameString,
    pub family_name: NameString,
}

impl From<Name> for NameWithConstraints {
    fn from(name: Name) -> Self {
        Self {
            given_name: name.given_name.into(),
            initial: name.initial.into(),
            family_name: name.family_name.into(),
        }
    }
}

#[derive(AsnType, Decode, Encode, Debug, PartialEq)]
#[rasn(
    tag(application, 3),
    delegate,
    from("0..=9"),
    size(8, extensible, "9..=20")
)]
pub struct ExtensibleDate(pub VisibleString);

impl From<Date> for ExtensibleDate {
    fn from(name: Date) -> Self {
        Self(name.0)
    }
}

#[derive(AsnType, Decode, Encode, Debug, PartialEq)]
#[rasn(tag(application, 3), delegate, from("0..=9"), size(8))]
pub struct DateWithConstraints(pub VisibleString);

impl From<Date> for DateWithConstraints {
    fn from(name: Date) -> Self {
        Self(name.0)
    }
}

#[derive(AsnType, Decode, Encode, Debug, PartialEq)]
#[rasn(delegate, from("a..=z", "A..=Z", "-", "."), size("1..=64", extensible))]
pub struct ExtensibleNameString(pub VisibleString);

impl From<VisibleString> for ExtensibleNameString {
    fn from(name: VisibleString) -> Self {
        Self(name)
    }
}

#[derive(AsnType, Decode, Encode, Debug, PartialEq)]
#[rasn(delegate, from("a..=z", "A..=Z", "-", "."), size("1..=64"))]
pub struct NameString(pub VisibleString);

impl From<VisibleString> for NameString {
    fn from(name: VisibleString) -> Self {
        Self(name)
    }
}

#[derive(AsnType, Decode, Encode, Debug, PartialEq)]
#[rasn(tag(application, 1))]
pub struct InitialString {
    #[rasn(size(1))]
    pub initial: NameString,
}

#[derive(AsnType, Decode, Encode, Debug, PartialEq)]
#[non_exhaustive]
pub struct Ax {
    #[rasn(value("250..=253"))]
    a: Integer,
    b: bool,
    c: AxChoice,
    #[rasn(extension_addition_group)]
    ext: Option<AxExtension>,
    i: Option<BmpString>,
    j: Option<PrintableString>,
}

#[derive(AsnType, Decode, Encode, Debug, PartialEq)]
#[rasn(choice)]
#[non_exhaustive]
pub enum AxChoice {
    D(Integer),
    #[rasn(extension_addition)]
    E(bool),
    #[rasn(extension_addition)]
    F(Ia5String),
}

#[derive(AsnType, Decode, Encode, Debug, PartialEq)]
pub struct AxExtension {
    #[rasn(size(3))]
    g: NumericString,
    h: Option<bool>,
}

impl Default for Ax {
    fn default() -> Self {
        Self {
            a: 253.into(),
            b: true,
            c: AxChoice::E(true),
            ext: Some(AxExtension {
                g: NumericString::try_from("123").unwrap(),
                h: Some(true),
            }),
            i: None,
            j: None,
        }
    }
}
