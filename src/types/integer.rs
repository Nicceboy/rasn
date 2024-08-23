use crate::types::{constraints, AsnType, Constraints, Extensible};
use crate::Tag;
use alloc::boxed::Box;
use num_bigint::{BigInt, BigUint, ToBigInt};
use num_traits::{identities::Zero, Signed, ToBytes, ToPrimitive};
use num_traits::{CheckedAdd, CheckedSub};

/// `Integer`` type is variable-sized non-constrained integer type which uses `isize` for lower values.
#[derive(Debug, Clone, Ord, Hash, Eq, PartialEq, PartialOrd)]
pub enum Integer {
    Primitive(isize),
    Variable(Box<BigInt>),
}

impl Default for Integer {
    fn default() -> Self {
        Self::Primitive(isize::default())
    }
}

impl core::fmt::Display for Integer {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::Primitive(value) => write!(f, "{}", value),
            Self::Variable(value) => write!(f, "{}", value),
        }
    }
}

impl num_traits::CheckedAdd for Integer {
    fn checked_add(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (Self::Primitive(lhs), Self::Primitive(rhs)) => {
                let value = lhs.checked_add(rhs);
                if value.is_some() {
                    return value.map(|value| Integer::from(value));
                } else {
                    Some(Self::Variable(Box::new(BigInt::from(*lhs) + *rhs)))
                }
            }
            (Self::Primitive(lhs), Self::Variable(rhs)) => {
                Some(Self::Variable(Box::new(BigInt::from(*lhs) + &**rhs)))
            }
            (Self::Variable(lhs), Self::Primitive(rhs)) => {
                Some(Self::Variable(Box::new(&**lhs + *rhs)))
            }
            (Self::Variable(lhs), Self::Variable(rhs)) => {
                Some(Self::Variable(Box::new(&**lhs + &**rhs)))
            }
        }
    }
}
// impl<I: IntegerType> core::ops::Add<I> for Integer {
//     type Output = Self;
//     fn add(self, rhs: I) -> Self::Output {
//         <Self as CheckedAdd>::checked_add(&self, &rhs.into()).unwrap_or_default()
//     }
// }

impl core::ops::Add for Integer {
    type Output = Self;
    fn add(self, rhs: Self) -> Self::Output {
        <Self as CheckedAdd>::checked_add(&self, &rhs).unwrap_or_default()
    }
}
macro_rules! impl_ops_integer {
    ($($t:ty),*) => {
        $(
            impl core::ops::Add<$t> for Integer {
                type Output = Self;
                fn add(self, rhs: $t) -> Self::Output {
                    match self {
                        Self::Primitive(lhs) => {
                            let result = lhs.checked_add(rhs as isize);
                            match result {
                                Some(value) => Self::Primitive(value),
                                None => Self::Variable(Box::new(BigInt::from(lhs) + rhs)),
                            }
                        }
                        Self::Variable(lhs) => {
                            Self::Variable(Box::new(*lhs + rhs))
                        }
                    }
                }
            }
            impl core::ops::Sub<$t> for Integer {
                type Output = Self;
                fn sub(self, rhs: $t) -> Self::Output {
                    match self {
                        Self::Primitive(lhs) => {
                            let result = lhs.checked_sub(rhs as isize);
                            match result {
                                Some(value) => Self::Primitive(value),
                                None => Self::Variable(Box::new(BigInt::from(lhs) - rhs)),
                            }
                        }
                        Self::Variable(lhs) => {
                            Self::Variable(Box::new(*lhs - rhs))
                        }
                    }
                }
            }
        )*
    };
}
macro_rules! impl_ops_integer_big {
    ($($t:ty),*) => {
        $(
            impl core::ops::Add<$t> for Integer {
                type Output = Self;
                fn add(self, rhs: $t) -> Self::Output {
                    match self {
                        Self::Primitive(lhs) => {
                            Self::Variable(Box::new(BigInt::from(lhs) + rhs))
                        }
                        Self::Variable(lhs) => {
                            Self::Variable(Box::new(*lhs + rhs))
                        }
                    }
                }
            }
            impl core::ops::Sub<$t> for Integer {
                type Output = Self;
                fn sub(self, rhs: $t) -> Self::Output {
                    match self {
                        Self::Primitive(lhs) => {
                            Self::Variable(Box::new(BigInt::from(lhs) - rhs))
                        }
                        Self::Variable(lhs) => {
                            Self::Variable(Box::new(*lhs - rhs))
                        }
                    }
                }
            }
        )*
    };
}

#[cfg(target_pointer_width = "32")]
impl_ops_integer!(u8, u16, i8, i16, i32, isize);
#[cfg(target_pointer_width = "32")]
impl_ops_integer_big!(u32, i64);
#[cfg(target_pointer_width = "64")]
impl_ops_integer!(u8, u16, u32, i8, i16, i32, i64, isize);

// Never fit for isize variant, used on all targets
impl_ops_integer_big!(u64, u128, usize, i128);

impl num_traits::CheckedSub for Integer {
    fn checked_sub(&self, other: &Self) -> Option<Self> {
        match (self, other) {
            (Self::Primitive(lhs), Self::Primitive(rhs)) => {
                let value = lhs.checked_sub(rhs);
                if value.is_some() {
                    return value.map(|value| Integer::from(value));
                } else {
                    Some(Self::Variable(Box::new(BigInt::from(*lhs) - *rhs)))
                }
            }
            (Self::Primitive(lhs), Self::Variable(rhs)) => {
                Some(Self::Variable(Box::new(BigInt::from(*lhs) - &**rhs)))
            }
            (Self::Variable(lhs), Self::Primitive(rhs)) => {
                Some(Self::Variable(Box::new(&**lhs - *rhs)))
            }
            (Self::Variable(lhs), Self::Variable(rhs)) => {
                Some(Self::Variable(Box::new(&**lhs - &**rhs)))
            }
        }
    }
}

impl core::ops::Sub for Integer {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self::Output {
        <Self as CheckedSub>::checked_sub(&self, &rhs).unwrap_or_default()
    }
}

impl ToPrimitive for Integer {
    fn to_i64(&self) -> Option<i64> {
        match self {
            Self::Primitive(value) => Some(*value as i64),
            Self::Variable(value) => value.to_i64(),
        }
    }
    fn to_u64(&self) -> Option<u64> {
        match self {
            Self::Primitive(value) => Some(*value as u64),
            Self::Variable(value) => value.to_u64(),
        }
    }
    fn to_i128(&self) -> Option<i128> {
        match self {
            Self::Primitive(value) => Some(*value as i128),
            Self::Variable(value) => value.to_i128(),
        }
    }
}
// impl core::cmp::PartialOrd for Integer {
//     fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
//         match (self, other) {
//             (Self::Primitive(lhs), Self::Primitive(rhs)) => lhs.partial_cmp(rhs),
//             (Self::Primitive(lhs), Self::Variable(rhs)) => BigInt::from(*lhs).partial_cmp(&**rhs),
//             (Self::Variable(lhs), Self::Primitive(rhs)) => (**lhs).partial_cmp(&BigInt::from(*rhs)),
//             (Self::Variable(lhs), Self::Variable(rhs)) => (**lhs).partial_cmp(&**rhs),
//         }
//     }
// }

macro_rules! impl_from_integer_as_prim {
    ($($t:ty),*) => {
        $(
            impl From<$t> for Integer {
                fn from(value: $t) -> Self {
                    Self::Primitive(value as isize)
                }
            }
        )*
    };
}

macro_rules! impl_from_integer_as_big {
    ($($t:ty),*) => {
        $(
            impl From<$t> for Integer {
                fn from(value: $t) -> Self {
                    if let Some(value) = value.to_isize() {
                        Self::Primitive(value)
                    } else {
                        Self::Variable(Box::new((BigInt::from(value))))
                    }
                }
            }
        )*
    };
}
#[cfg(target_pointer_width = "32")]
impl_from_integer_as_prim!(u8, u16, u32, i8, i16, i32, isize);
#[cfg(target_pointer_width = "32")]
impl_from_integer_as_big!(i64);
#[cfg(target_pointer_width = "64")]
impl_from_integer_as_prim!(u8, u16, u32, i8, i16, i32, i64, isize);
// Never fit for isize variant, used on all targets
impl_from_integer_as_big!(u64, u128, i128, usize, BigInt);

impl From<Integer> for BigInt {
    fn from(value: Integer) -> Self {
        match value {
            Integer::Primitive(value) => value.to_bigint().unwrap(),
            Integer::Variable(value) => *value,
        }
    }
}

impl ToBigInt for Integer {
    fn to_bigint(&self) -> Option<BigInt> {
        match self {
            Integer::Primitive(value) => Some(BigInt::from(*value)),
            Integer::Variable(value) => Some(*value.clone()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TryFromIntegerError {
    original: BigInt,
}

impl TryFromIntegerError {
    fn new(original: BigInt) -> Self {
        TryFromIntegerError { original }
    }
    fn __description(&self) -> &str {
        "out of range conversion regarding integer conversion attempted"
    }
    pub fn into_original(self) -> BigInt {
        self.original
    }
}

impl alloc::fmt::Display for TryFromIntegerError {
    fn fmt(&self, f: &mut alloc::fmt::Formatter<'_>) -> alloc::fmt::Result {
        self.__description().fmt(f)
    }
}
macro_rules! impl_try_into_integer {
    ($($t:ty),*) => {
        $(
            impl core::convert::TryFrom<Integer> for $t {
                type Error = TryFromIntegerError;
                fn try_from(value: Integer) -> Result<Self, Self::Error> {
                    Self::try_from(&value)
                }
            }
            impl core::convert::TryFrom<&Integer> for $t {
                type Error = TryFromIntegerError;
                fn try_from(value: &Integer) -> Result<Self, Self::Error> {
                    match value {
                        Integer::Primitive(value) => (*value).try_into().map_err(|_| TryFromIntegerError::new(value.to_bigint().unwrap_or_default())),
                        Integer::Variable(value) => (**value).clone().try_into().map_err(|_| TryFromIntegerError::new(*value.clone())),
                    }
                }
            }
        )*
    };
}
impl_try_into_integer!(u8, u16, u32, u64, u128, usize, i8, i16, i32, i64, i128, isize);

/// An integer which has encoded constraint range between `START` and `END`.
#[derive(Debug, Clone, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct ConstrainedInteger<const START: i128, const END: i128>(pub(crate) Integer);

impl<const START: i128, const END: i128> AsnType for ConstrainedInteger<START, END> {
    const TAG: Tag = Tag::INTEGER;
    const CONSTRAINTS: Constraints<'static> =
        Constraints::new(&[constraints::Constraint::Value(Extensible::new(
            constraints::Value::new(constraints::Bounded::const_new(START, END)),
        ))]);
}

impl<const START: i128, const END: i128> core::ops::Deref for ConstrainedInteger<START, END> {
    type Target = Integer;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Into<Integer>, const START: i128, const END: i128> From<T>
    for ConstrainedInteger<START, END>
{
    fn from(value: T) -> Self {
        Self(value.into())
    }
}

pub trait IntegerType:
    Sized
    + Clone
    + core::fmt::Debug
    + core::fmt::Display
    + Default
    + TryInto<i128>
    + TryInto<isize>
    + TryFrom<i128>
    + TryFrom<isize>
    + TryFrom<BigInt>
    + Into<BigInt>
    + ToBigInt
    + num_traits::CheckedAdd
    + num_traits::CheckedSub
    + core::cmp::PartialOrd
    + core::cmp::PartialEq
    + num_traits::ToPrimitive
{
    const WIDTH: u32;
    const BYTE_WIDTH: usize = Self::WIDTH as usize / 8;

    fn try_from_bytes(input: &[u8], codec: crate::Codec)
        -> Result<Self, crate::error::DecodeError>;

    fn try_from_signed_bytes(
        input: &[u8],
        codec: crate::Codec,
    ) -> Result<Self, crate::error::DecodeError>;

    fn try_from_unsigned_bytes(
        input: &[u8],
        codec: crate::Codec,
    ) -> Result<Self, crate::error::DecodeError>;

    /// Finds the minimum number of bytes needed to present the unsigned integer. (drops leading zeros or ones)
    fn unsigned_bytes_needed(&self) -> usize;

    /// Finds the minimum number of bytes needed to present the signed integer. (drops leading zeros or ones)
    fn signed_bytes_needed(&self) -> usize;

    /// Returns minimum number defined by `usize` of signed Big-endian bytes needed to encode the integer.
    fn to_signed_bytes_be(&self) -> (impl AsRef<[u8]>, usize);

    /// Returns minimum number defined by `usize` of unsigned Big-endian bytes needed to encode the integer.
    fn to_unsigned_bytes_be(&self) -> (impl AsRef<[u8]>, usize);

    // `num_traits::WrappingAdd` is not implemented for `BigInt`
    #[doc(hidden)]
    fn wrapping_add(self, other: Self) -> Self;

    fn is_negative(&self) -> bool;
}

/// Encode the given `N` sized integer as big-endian bytes and determine the number of bytes needed.
/// Needed bytes drops unnecessary leading zeros or ones.
fn needed_as_be_bytes<T: ToBytes + IntegerType, const N: usize>(
    value: T,
    signed: bool,
) -> ([u8; N], usize) {
    let bytes: [u8; N] = value.to_le_bytes().as_ref().try_into().unwrap_or([0; N]);
    let needed = if signed {
        value.signed_bytes_needed()
    } else {
        value.unsigned_bytes_needed()
    };
    let mut slice_reversed: [u8; N] = [0; N];
    // About 2.5x speed when compared to `copy_from_slice` and `.reverse()`, since we don't need all bytes in most cases
    for i in 0..needed {
        slice_reversed[i] = bytes[needed - 1 - i];
    }
    (slice_reversed, needed)
}
macro_rules! integer_type_impl {
    ((signed $t1:ty, $t2:ty), $($ts:tt)*) => {
        impl IntegerType for $t1 {
            const WIDTH: u32 = <$t1>::BITS;

            fn try_from_bytes(
                input: &[u8],
                codec: crate::Codec,
            ) -> Result<Self, crate::error::DecodeError> {
                Self::try_from_signed_bytes(input, codec)
            }

            fn try_from_signed_bytes(
                input: &[u8],
                codec: crate::Codec,
            ) -> Result<Self, crate::error::DecodeError> {
                const BYTE_SIZE: usize = (<$t1>::BITS / 8) as usize;
                if input.is_empty() {
                    return Err(crate::error::DecodeError::unexpected_empty_input(codec));
                }
                if input.len() > BYTE_SIZE {
                    return Err(crate::error::DecodeError::integer_overflow(<$t1>::BITS, codec));
                }

                let mut array = [0u8; BYTE_SIZE];
                let pad = if input[0] & 0x80 == 0 { 0 } else { 0xff };
                array[..BYTE_SIZE - input.len()].fill(pad);
                array[BYTE_SIZE - input.len()..].copy_from_slice(input);
                Ok(Self::from_be_bytes(array))
            }

            fn try_from_unsigned_bytes(
                input: &[u8],
                codec: crate::Codec,
            ) -> Result<Self, crate::error::DecodeError> {
                Ok(<$t2>::try_from_bytes(input, codec)? as $t1)
            }
            fn unsigned_bytes_needed(&self) -> usize {
                (*self as $t2).unsigned_bytes_needed()
            }
            fn signed_bytes_needed(&self) -> usize {
                let leading_bits = if Signed::is_negative(self) {
                    self.leading_ones() as usize
                } else {
                    self.leading_zeros() as usize
                };
                let full_bytes = Self::BYTE_WIDTH - leading_bits / 8;
                let extra_byte = (leading_bits % 8 == 0) as usize;
                full_bytes + extra_byte

            }

            fn to_signed_bytes_be(&self) -> (impl AsRef<[u8]>, usize) {
                const N: usize = core::mem::size_of::<$t1>();
                needed_as_be_bytes::<$t1, N>(*self, true)
            }
            fn to_unsigned_bytes_be(&self) -> (impl AsRef<[u8]>, usize) {
                const N: usize = core::mem::size_of::<$t2>();
                needed_as_be_bytes::<$t2, N>(*self as $t2, false)
            }

            fn wrapping_add(self, other: Self) -> Self {
                self.wrapping_add(other)
            }
            fn is_negative(&self) -> bool {
                <Self as Signed>::is_negative(self)
            }
        }

        integer_type_impl!($($ts)*);
    };
    ((unsigned $t1:ty, $t2:ty), $($ts:tt)*) => {



        impl IntegerType for $t1 {
            const WIDTH: u32 = <$t1>::BITS;

            fn try_from_bytes(
                input: &[u8],
                codec: crate::Codec,
            ) -> Result<Self, crate::error::DecodeError> {
                Self::try_from_unsigned_bytes(input, codec)
            }

            fn try_from_signed_bytes(
                input: &[u8],
                codec: crate::Codec,
            ) -> Result<Self, crate::error::DecodeError> {
                Ok(<$t2>::try_from_bytes(input, codec)? as $t1)
            }

            fn try_from_unsigned_bytes(
                input: &[u8],
                codec: crate::Codec,
            ) -> Result<Self, crate::error::DecodeError> {
                const BYTE_SIZE: usize = (<$t1>::BITS / 8) as usize;
                if input.is_empty() {
                    return Err(crate::error::DecodeError::unexpected_empty_input(codec));
                }
                if input.len() > BYTE_SIZE {
                    return Err(crate::error::DecodeError::integer_overflow(<$t1>::BITS, codec));
                }

                let mut array = [0u8; BYTE_SIZE];
                array[BYTE_SIZE - input.len()..].copy_from_slice(input);
                Ok(Self::from_be_bytes(array))
            }
            fn unsigned_bytes_needed(&self) -> usize {
                if self.is_zero() {
                    1
                } else {
                    let significant_bits = Self::WIDTH as usize - self.leading_zeros() as usize;
                    (significant_bits + 7) / 8
                }
            }
            fn signed_bytes_needed(&self) -> usize {
                (*self as $t2).signed_bytes_needed()
            }

            fn to_signed_bytes_be(&self) -> (impl AsRef<[u8]>, usize) {
                const N: usize = core::mem::size_of::<$t2>();
                needed_as_be_bytes::<$t2, N>(*self as $t2, true)
            }
            fn to_unsigned_bytes_be(&self) -> (impl AsRef<[u8]>, usize) {
                const N: usize = core::mem::size_of::<$t1>();
                needed_as_be_bytes::<$t1, N>(*self, false)
            }

            fn wrapping_add(self, other: Self) -> Self {
                self.wrapping_add(other)
            }
            fn is_negative(&self) -> bool {
                false
            }
        }

        integer_type_impl!($($ts)*);
    };
    (,) => {};
    () => {};
}

integer_type_impl!(
    (unsigned u8, i16),
    (signed i8, u8),
    (unsigned u16, i32),
    (signed i16, u16),
    (unsigned u32, i64),
    (signed i32, u32),
    (unsigned u64, i128),
    (signed i64, u64),
    // Will truncate on i128 on large numbers
    (unsigned u128, i128),
    (signed i128, u128),
    (unsigned usize, i128),
    (signed isize, usize),
);

impl IntegerType for BigInt {
    const WIDTH: u32 = u32::MAX;

    fn try_from_bytes(
        input: &[u8],
        codec: crate::Codec,
    ) -> Result<Self, crate::error::DecodeError> {
        if input.is_empty() {
            return Err(crate::error::DecodeError::unexpected_empty_input(codec));
        }

        Ok(BigInt::from_signed_bytes_be(input))
    }

    fn try_from_signed_bytes(
        input: &[u8],
        codec: crate::Codec,
    ) -> Result<Self, crate::error::DecodeError> {
        Self::try_from_bytes(input, codec)
    }

    fn try_from_unsigned_bytes(
        input: &[u8],
        codec: crate::Codec,
    ) -> Result<Self, crate::error::DecodeError> {
        if input.is_empty() {
            return Err(crate::error::DecodeError::unexpected_empty_input(codec));
        }

        Ok(BigUint::from_bytes_be(input).into())
    }
    // Not needed for BigInt
    fn unsigned_bytes_needed(&self) -> usize {
        unreachable!()
    }

    // Not needed for BigInt
    fn signed_bytes_needed(&self) -> usize {
        unreachable!()
    }
    fn to_signed_bytes_be(&self) -> (impl AsRef<[u8]>, usize) {
        let bytes = self.to_signed_bytes_be();
        let len = bytes.len();
        (bytes, len)
    }
    fn to_unsigned_bytes_be(&self) -> (impl AsRef<[u8]>, usize) {
        let bytes = self.to_biguint().unwrap_or_default().to_bytes_be();
        let len = bytes.len();
        (bytes, len)
    }

    fn wrapping_add(self, other: Self) -> Self {
        self + other
    }
    fn is_negative(&self) -> bool {
        <Self as Signed>::is_negative(self)
    }
}
enum BytesRef {
    Stack([u8; core::mem::size_of::<isize>()]),
    // Heap(&'a [u8]),
    Heap(alloc::vec::Vec<u8>),
}

impl AsRef<[u8]> for BytesRef {
    fn as_ref(&self) -> &[u8] {
        match self {
            BytesRef::Stack(arr) => arr,
            BytesRef::Heap(slice) => slice,
        }
    }
}

impl IntegerType for Integer {
    const WIDTH: u32 = u32::MAX;

    fn try_from_bytes(
        input: &[u8],
        codec: crate::Codec,
    ) -> Result<Self, crate::error::DecodeError> {
        if input.is_empty() {
            return Err(crate::error::DecodeError::unexpected_empty_input(codec));
        }
        let value = isize::try_from_bytes(input, codec);
        if let Ok(value) = value {
            Ok(Integer::Primitive(value))
        } else {
            Ok(Integer::Variable(Box::new(BigInt::try_from_bytes(
                input, codec,
            )?)))
        }
    }

    fn try_from_signed_bytes(
        input: &[u8],
        codec: crate::Codec,
    ) -> Result<Self, crate::error::DecodeError> {
        Self::try_from_bytes(input, codec)
    }

    fn try_from_unsigned_bytes(
        input: &[u8],
        codec: crate::Codec,
    ) -> Result<Self, crate::error::DecodeError> {
        if input.is_empty() {
            return Err(crate::error::DecodeError::unexpected_empty_input(codec));
        }
        let value = isize::try_from_unsigned_bytes(input, codec);
        if let Ok(value) = value {
            Ok(Integer::Primitive(value))
        } else {
            Ok(Integer::Variable(Box::new(
                BigInt::try_from_unsigned_bytes(input, codec)?.into(),
            )))
        }
    }
    // Not needed for Integer
    fn unsigned_bytes_needed(&self) -> usize {
        unreachable!()
    }

    // Not needed for Integer
    fn signed_bytes_needed(&self) -> usize {
        unreachable!()
    }

    fn to_signed_bytes_be(&self) -> (impl AsRef<[u8]>, usize) {
        match self {
            Integer::Primitive(value) => {
                let (bytes, len) = <isize as IntegerType>::to_signed_bytes_be(value);
                let mut arr = [0u8; core::mem::size_of::<isize>()];
                // arr[..len].copy_from_slice(bytes.as_ref());
                for i in 0..len {
                    arr[i] = bytes.as_ref()[i];
                }
                (BytesRef::Stack(arr), len)
            }
            Integer::Variable(value) => {
                // let (bytes, len) = <BigInt as IntegerType>::to_signed_bytes_be(value);
                let bigint_bytes = value.to_signed_bytes_be();
                let len = bigint_bytes.len();
                (BytesRef::Heap(bigint_bytes), len)
            }
        }
    }
    fn to_unsigned_bytes_be(&self) -> (impl AsRef<[u8]>, usize) {
        match self {
            Integer::Primitive(value) => {
                let (bytes, len) = <isize as IntegerType>::to_unsigned_bytes_be(value);
                let mut arr = [0u8; core::mem::size_of::<isize>()];
                for i in 0..len {
                    arr[i] = bytes.as_ref()[i];
                }
                (BytesRef::Stack(arr), len)
            }
            Integer::Variable(value) => {
                // let (bytes, len) = <BigInt as IntegerType>::to_signed_bytes_be(value);
                let bigint_bytes = value.to_biguint().unwrap().to_bytes_be();
                let len = bigint_bytes.len();
                (BytesRef::Heap(bigint_bytes), len)
            }
        }
    }

    fn wrapping_add(self, other: Self) -> Self {
        self + other
    }
    fn is_negative(&self) -> bool {
        match self {
            Integer::Primitive(value) => <isize as IntegerType>::is_negative(value),
            Integer::Variable(value) => <BigInt as IntegerType>::is_negative(value),
        }
    }
}
