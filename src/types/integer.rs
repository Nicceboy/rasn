use crate::types::{constraints, AsnType, Constraints, Extensible};
use crate::Tag;
use core::ops::{Add, Sub};
use num_bigint::{BigInt, BigUint};
use num_traits::{Num, NumOps, Pow, PrimInt, Signed, ToBytes, ToPrimitive, Zero};

// #[cfg(any(target_pointer_width = "16", target_pointer_width = "32"))]

// It seems that just ~1% performance difference between i64 and i128 (at least on M2 Mac)
// If disabled, it should be meant for targets which cannot use i128

#[cfg(not(feature = "i128"))]
pub type StdInt = i64;

#[cfg(target_pointer_width = "64")]
#[cfg(feature = "i128")]
pub type StdInt = i128;

macro_rules! impl_from_integer {
    ($($t:ty),*) => {
        $(
            impl From<$t> for Integer {
                fn from(value: $t) -> Self {
                    #[allow(clippy::cast_possible_truncation)]
                    Integer::Primitive(PrimitiveInteger::<StdInt>(value as StdInt))
                }
            }
            impl From<&$t> for Integer {
                        fn from(value: &$t) -> Self {
                            #[allow(clippy::cast_possible_truncation)]
                            Integer::Primitive(PrimitiveInteger::<StdInt>(*value as StdInt))
                        }
                    }
        )*
    };
}
macro_rules! impl_from_primitive_integer {
    ($($t:ty),*) => {
        $(
            impl From<$t> for PrimitiveInteger<StdInt> {
fn from(value: $t) -> Self {
                    #[allow(clippy::cast_possible_truncation)]
                    PrimitiveInteger::<StdInt>(value as StdInt)
                }
            }
        )*
    };
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct TryFromIntegerError<T> {
    original: T,
}

impl<T> TryFromIntegerError<T> {
    fn new(original: T) -> Self {
        TryFromIntegerError { original }
    }
    fn __description(&self) -> &str {
        "out of range conversion regarding big integer attempted"
    }
    pub fn into_original(self) -> T {
        self.original
    }
}

impl<T> alloc::fmt::Display for TryFromIntegerError<T> {
    fn fmt(&self, f: &mut alloc::fmt::Formatter<'_>) -> alloc::fmt::Result {
        self.__description().fmt(f)
    }
}
macro_rules! impl_try_from_bigint {
    ($T:ty, $to_ty:path) => {
        impl TryFrom<&Integer> for $T {
            type Error = TryFromIntegerError<()>;

            #[inline]
            fn try_from(value: &Integer) -> Result<$T, TryFromIntegerError<()>> {
                match value {
                    Integer::Primitive(PrimitiveInteger::<StdInt>(value)) => {
                        $to_ty(value).ok_or(TryFromIntegerError::new(()))
                    }
                    Integer::Big(value) => {
                        value.try_into().map_err(|_| TryFromIntegerError::new(()))
                    }
                }
                // $to_ty(value).ok_or(TryFromIntegerError::new(()))
            }
        }

        impl TryFrom<Integer> for $T {
            type Error = TryFromIntegerError<Integer>;

            #[inline]
            fn try_from(value: Integer) -> Result<$T, TryFromIntegerError<Integer>> {
                <$T>::try_from(&value).map_err(|_| TryFromIntegerError::new(value))
            }
        }
    };
}

// TODO note the change here if inner primitive type should be changed

impl_try_from_bigint!(u8, ToPrimitive::to_u8);
impl_try_from_bigint!(u16, ToPrimitive::to_u16);
impl_try_from_bigint!(u32, ToPrimitive::to_u32);
impl_try_from_bigint!(u64, ToPrimitive::to_u64);
impl_try_from_bigint!(usize, ToPrimitive::to_usize);
// impl_try_from_bigint!(u128, ToPrimitive::to_u128);

impl_try_from_bigint!(i8, ToPrimitive::to_i8);
impl_try_from_bigint!(i16, ToPrimitive::to_i16);
impl_try_from_bigint!(i32, ToPrimitive::to_i32);
impl_try_from_bigint!(i64, ToPrimitive::to_i64);
impl_try_from_bigint!(isize, ToPrimitive::to_isize);
impl_try_from_bigint!(i128, ToPrimitive::to_i128);

impl From<BigInt> for Integer {
    fn from(value: BigInt) -> Self {
        Integer::Big(value)
    }
}

#[cfg(target_pointer_width = "64")]
#[cfg(feature = "i128")]
impl From<u128> for Integer {
    fn from(value: u128) -> Self {
        Integer::Big(value.into())
    }
}
impl_from_primitive_integer! {
    i8,
    i16,
    i32,
    isize,
    u8,
    u16,
    u32,
    usize
}
#[cfg(target_pointer_width = "64")]
#[cfg(feature = "i128")]
impl_from_primitive_integer! {
    i64,
    i128,
    u64
}
impl_from_integer! {
    i8,
    i16,
    i32,
    isize,
    u8,
    u16,
    u32,
    usize
}
#[cfg(target_pointer_width = "64")]
#[cfg(feature = "i128")]
impl_from_integer! {
    i64,
    i128,
    u64
}

pub trait PrimitiveIntegerTraits:
    PrimInt + Num + NumOps + Signed + ToBytes + Default + Sized
{
}
impl<T: PrimInt + Num + NumOps + Signed + ToBytes + Default + Sized> PrimitiveIntegerTraits for T {}

/// Primitive integers can bring significant performance improvements
/// As a result, native word size and i128/u128 are supported as smaller integers
/// Note: all architectures do not support i128/u128
/// 128-support
#[derive(Debug, Clone, Copy, Hash, Eq, PartialEq, PartialOrd, Ord)]
#[non_exhaustive]
pub struct PrimitiveInteger<T: PrimitiveIntegerTraits>(T);

impl<T: PrimitiveIntegerTraits> core::ops::Deref for PrimitiveInteger<T> {
    type Target = T;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<T: PrimitiveIntegerTraits> core::default::Default for PrimitiveInteger<T> {
    fn default() -> Self {
        PrimitiveInteger(T::from(0).unwrap_or_default())
    }
}

impl<T: PrimitiveIntegerTraits> PrimitiveInteger<T> {
    /// Byte with of the primitive integer. E.g. width for `i128` is 16 bytes.
    pub const BYTE_WIDTH: usize = core::mem::size_of::<T>();
    /// Optimized primitive integer bytes
    /// Returns pointer to the native endian presentation of the integer from the memory
    /// Pointer includes minimal amount of bytes to present unsigned or signed integer, based on `signed`
    // #[inline]
    // pub fn unsafe_minimal_ne_bytes(&self, signed: bool) -> &[u8] {
    //     let ptr = &self.0 as *const T as *const u8; // Cast to a pointer to the first byte
    //     let bytes_needed = if signed {
    //         self.signed_bytes_needed()
    //     } else {
    //         self.unsigned_bytes_needed()
    //     };
    //     debug_assert!(
    //         bytes_needed <= Self::BYTE_WIDTH,
    //         "Too many bytes needed in unsafe call! Would lead into reading more data than it is safe."
    //     );
    //     unsafe { core::slice::from_raw_parts(ptr, bytes_needed) }
    // }
    /// Minimal number of bytes needed to present unsigned integer
    fn unsigned_bytes_needed(&self) -> usize {
        if self.0.is_zero() {
            1
        } else {
            Self::BYTE_WIDTH - (self.0.leading_zeros() / 8) as usize
        }
    }

    /// Minimal number of bytes needed to present signed integer
    fn signed_bytes_needed(&self) -> usize {
        if self.0.is_negative() {
            let leading_ones = self.0.leading_ones() as usize;
            if leading_ones % 8 == 0 {
                Self::BYTE_WIDTH - leading_ones / 8 + 1
            } else {
                Self::BYTE_WIDTH - leading_ones / 8
            }
        } else {
            let leading_zeroes = self.0.leading_zeros() as usize;
            if leading_zeroes % 8 == 0 {
                Self::BYTE_WIDTH - leading_zeroes / 8 + 1
            } else {
                Self::BYTE_WIDTH - leading_zeroes / 8
            }
        }
    }

    fn to_le_bytes<const N: usize>(self) -> [u8; N] {
        self.0.to_le_bytes().as_ref().try_into().unwrap_or([0; N])
    }
    /// Calculate minimal number of bytes to show integer based on `signed` status
    /// Returns slice an with fixed-width `N` and number of needed bytes
    /// We need only some of the bytes, might be better than using `to_be_bytes`
    pub fn needed_as_be_bytes<const N: usize>(&self, signed: bool) -> ([u8; N], usize) {
        let bytes: [u8; N] = self.to_le_bytes();
        let needed = if signed {
            self.signed_bytes_needed()
        } else {
            self.unsigned_bytes_needed()
        };
        let mut slice_reversed: [u8; N] = [0; N];
        let len = needed;
        for i in 0..len {
            slice_reversed[i] = bytes[len - 1 - i];
        }
        (slice_reversed, needed)
    }
    /// Swap bytes order and copy to new `N` sized slice
    fn swap_bytes<const N: usize>(bytes: &[u8]) -> ([u8; N], usize) {
        let mut slice_reversed: [u8; N] = [0; N];
        for i in 0..bytes.len() {
            slice_reversed[i] = bytes[bytes.len() - 1 - i];
        }
        (slice_reversed, bytes.len())
    }
    /// Swap bytes order and copy to new `N` sized slice
    pub fn swap_to_be_bytes<const N: usize>(bytes: &[u8]) -> ([u8; N], usize) {
        #[cfg(target_endian = "little")]
        {
            Self::swap_bytes(bytes)
        }
        #[cfg(target_endian = "big")]
        {
            (bytes.try_into().unwrap_or([0; N]), bytes.len())
        }
    }
}

//
#[derive(Debug, Clone, Ord, Hash, Eq, PartialEq, PartialOrd)]
#[non_exhaustive]
pub enum Integer {
    Primitive(PrimitiveInteger<StdInt>),
    Big(BigInt),
}
impl Integer {
    /// Returns generic `PrimitiveInteger` wrapper type if that is the current variant
    #[must_use]
    pub fn as_primitive(&self) -> Option<&PrimitiveInteger<StdInt>> {
        match self {
            Integer::Primitive(ref inner) => Some(inner),
            _ => None,
        }
    }

    /// Returns inner stack-located `i128` if that is the current variant
    #[cfg(feature = "i128")]
    #[must_use]
    pub fn as_i128(&self) -> Option<i128> {
        match self {
            Integer::Primitive(value) => Some(**value),
            _ => None,
        }
    }
    /// Returns inner heap-allocated `BigInt` if that is the current variant
    #[must_use]
    pub fn as_big(&self) -> Option<&BigInt> {
        match self {
            Integer::Big(ref inner) => Some(inner),
            _ => None,
        }
    }
    /// Return Big-Endian presentation of `Integer`
    /// Will always create new heap allocation, use carefully
    #[must_use]
    #[inline]
    pub fn to_be_bytes(&self) -> alloc::vec::Vec<u8> {
        match self {
            Integer::Primitive(value) => value.to_be_bytes().to_vec(),
            Integer::Big(value) => value.to_signed_bytes_be(),
        }
    }

    /// Return `Integer` as unsigned bytes if possible
    /// Typically this means that resulting byte array does not include any padded zeros or 0xFF
    /// Will always create new heap allocation, use carefully
    /// Rather, if possible, match for underlying primitive integer type to get raw slice without conversions
    #[must_use]
    #[inline]
    pub fn to_unsigned_be_bytes(&self) -> Option<alloc::vec::Vec<u8>> {
        match self {
            Integer::Primitive(value) => {
                let (value, len) =
                    value.needed_as_be_bytes::<{ PrimitiveInteger::<StdInt>::BYTE_WIDTH }>(false);
                Some(value[..len].to_vec())
            }
            Integer::Big(value) => Some(value.to_biguint()?.to_bytes_be()),
        }
    }

    /// New `Integer` from signed bytes.
    /// Inner type is defined based on the amount of bytes.
    /// `Integer::Primitive` if less or equal defined primitive width, otherwise `Integer::Big`.
    #[must_use]
    pub fn from_signed_be_bytes(bytes: &[u8]) -> Self {
        let mut array = [0u8; PrimitiveInteger::<StdInt>::BYTE_WIDTH];
        if bytes.len() <= PrimitiveInteger::<StdInt>::BYTE_WIDTH {
            let pad = if bytes[0] & 0x80 == 0 { 0 } else { 0xff };

            array[..PrimitiveInteger::<StdInt>::BYTE_WIDTH - bytes.len()].fill(pad);
            array[PrimitiveInteger::<StdInt>::BYTE_WIDTH - bytes.len()..].copy_from_slice(bytes);

            Integer::Primitive(PrimitiveInteger::<StdInt>(StdInt::from_be_bytes(array)))
        } else {
            // If the byte slice is longer than the byte width, treat it as a BigInt
            Integer::Big(BigInt::from_signed_bytes_be(bytes))
        }
    }
    /// New `Integer` from unsigned bytes.
    /// Inner type is defined based on the amount of bytes.
    /// `Integer::Primitive` if less or equal defined primitive width, otherwise `Integer::Big`.
    #[must_use]
    pub fn from_be_bytes(bytes: &[u8]) -> Self {
        if bytes.len() <= PrimitiveInteger::<StdInt>::BYTE_WIDTH {
            let mut array = [0u8; PrimitiveInteger::<StdInt>::BYTE_WIDTH];
            for i in 0..bytes.len() {
                array[PrimitiveInteger::<StdInt>::BYTE_WIDTH - bytes.len() + i] = bytes[i];
            }
            Integer::Primitive(PrimitiveInteger::<StdInt>(StdInt::from_be_bytes(array)))
        } else {
            Integer::Big(BigUint::from_bytes_be(bytes).into())
        }
    }
    /// Returns the amount of bytes that integer **might** take to present
    /// Useful on pre-allocating vectors with minimum size
    /// *Not* free operation
    #[must_use]
    pub fn byte_length(&self) -> usize {
        match self {
            Integer::Primitive(value) => value.to_ne_bytes().len(),
            Integer::Big(value) => value.to_ne_bytes().len(),
        }
    }

    #[must_use]
    pub fn bits(&self) -> u64 {
        // TODO: will overflow and panic on very large numbers
        match self {
            Integer::Primitive(value) => value.to_be_bytes().len() as u64 * 8u64,
            Integer::Big(value) => value.bits(),
        }
    }
    #[must_use]
    pub fn signum(&self) -> Self {
        match self {
            Integer::Primitive(value) => {
                Integer::Primitive(PrimitiveInteger::<StdInt>(value.signum()))
            }
            Integer::Big(value) => Integer::Big(value.signum()),
        }
    }
    #[must_use]
    pub fn is_positive(&self) -> bool {
        match self {
            Integer::Primitive(value) => value.is_positive(),
            Integer::Big(value) => value.is_positive(),
        }
    }
    #[must_use]
    pub fn is_negative(&self) -> bool {
        match self {
            Integer::Primitive(value) => value.is_negative(),
            Integer::Big(value) => value.is_negative(),
        }
    }
    #[must_use]
    pub fn is_zero(&self) -> bool {
        match self {
            Integer::Primitive(value) => value.is_zero(),
            Integer::Big(value) => value.is_zero(),
        }
    }
}

impl Sub for Integer {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Integer::Primitive(lhs), Integer::Primitive(rhs)) => {
                Integer::Big(BigInt::from(*lhs) - *rhs)
            }
            (Integer::Big(lhs), Integer::Big(rhs)) => Integer::Big(lhs - rhs),
            (Integer::Primitive(lhs), Integer::Big(rhs)) => Integer::Big(*lhs - rhs),
            (Integer::Big(lhs), Integer::Primitive(rhs)) => Integer::Big(lhs - *rhs),
        }
    }
}
impl Add for Integer {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Integer::Primitive(lhs), Integer::Primitive(rhs)) => {
                Integer::Big(BigInt::from(*lhs) + *rhs)
            }
            (Integer::Big(lhs), Integer::Big(rhs)) => Integer::Big(lhs + rhs),
            (Integer::Primitive(lhs), Integer::Big(rhs)) => Integer::Big(*lhs + rhs),
            (Integer::Big(lhs), Integer::Primitive(rhs)) => Integer::Big(lhs + *rhs),
        }
    }
}

impl Pow<Integer> for Integer {
    type Output = Self;

    /// Will silently truncate too large exponents
    fn pow(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Integer::Primitive(lhs), Integer::Primitive(rhs)) => {
                Integer::Big(BigInt::from(*lhs).pow(rhs.to_u32().unwrap()))
            }
            (Integer::Big(lhs), Integer::Big(rhs)) => Integer::Big(lhs.pow(rhs.to_u32().unwrap())),
            (Integer::Primitive(lhs), Integer::Big(rhs)) => {
                Integer::Big(BigInt::from(*lhs).pow(rhs.to_u32().unwrap()))
            }
            (Integer::Big(lhs), Integer::Primitive(rhs)) => Integer::Big(lhs.pow(*rhs as u128)),
        }
    }
}

impl core::ops::Shl<Integer> for Integer {
    type Output = Self;

    fn shl(self, rhs: Self) -> Self::Output {
        match (self, rhs) {
            (Integer::Primitive(lhs), Integer::Primitive(rhs)) => {
                Integer::Primitive(PrimitiveInteger::<StdInt>(*lhs << *rhs))
            }
            (Integer::Big(lhs), Integer::Big(rhs)) => Integer::Big(lhs << rhs.to_i128().unwrap()),
            (Integer::Primitive(lhs), Integer::Big(rhs)) => {
                Integer::Big((*lhs << rhs.to_i128().unwrap()).into())
            }
            (Integer::Big(lhs), Integer::Primitive(rhs)) => Integer::Big(lhs << *rhs),
        }
    }
}

impl alloc::fmt::Display for Integer {
    fn fmt(&self, f: &mut alloc::fmt::Formatter<'_>) -> alloc::fmt::Result {
        match self {
            Integer::Primitive(value) => write!(f, "{}", **value),
            Integer::Big(value) => write!(f, "{value}"),
        }
    }
}

impl core::default::Default for Integer {
    fn default() -> Self {
        Integer::Primitive(PrimitiveInteger::<StdInt>(0))
    }
}

///
/// A integer which has encoded constraint range between `START` and `END`.
#[derive(Debug, Clone, Eq, Hash, PartialEq, PartialOrd, Ord)]
pub struct ConstrainedInteger<const START: StdInt, const END: StdInt>(pub(crate) Integer);

impl<const START: StdInt, const END: StdInt> AsnType for ConstrainedInteger<START, END> {
    const TAG: Tag = Tag::INTEGER;
    const CONSTRAINTS: Constraints<'static> =
        Constraints::new(&[constraints::Constraint::Value(Extensible::new(
            constraints::Value::new(constraints::Bounded::const_new(START as i128, END as i128)),
        ))]);
}

impl<const START: StdInt, const END: StdInt> core::ops::Deref for ConstrainedInteger<START, END> {
    type Target = Integer;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<T: Into<Integer>, const START: StdInt, const END: StdInt> From<T>
    for ConstrainedInteger<START, END>
{
    fn from(value: T) -> Self {
        Self(value.into())
    }
}
