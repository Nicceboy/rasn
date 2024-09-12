use super::IntegerType;
use num_bigint::BigInt;

const SUPPORTED_CONSTRAINTS_COUNT: usize = 5;
pub const DEFAULT_CONSTRAINTS: Constraints = Constraints::NONE;

#[derive(Debug, Clone)]
pub struct Constraints(pub &'static [Constraint]);

impl Constraints {
    pub const NONE: Self = Self(&[]);

    pub const fn new(constraints: &'static [Constraint]) -> Self {
        Self(constraints)
    }
    pub const fn inner(&self) -> &'static [Constraint] {
        self.0
    }
    /// We currently only support 5 different constraint types, which includes the empty constraint.
    /// `usize` is used to drop the empty ones, which are needed to initialize the array.
    pub const fn from_fixed_size(
        merged: &'static ([Constraint; SUPPORTED_CONSTRAINTS_COUNT], usize),
    ) -> Self {
        struct ConstraintArray<const N: usize>(&'static [Constraint; N]);

        impl<const N: usize> ConstraintArray<N> {
            const fn as_slice(&self) -> &'static [Constraint] {
                self.0
            }
        }
        let array = ConstraintArray(&merged.0);
        Self::new(array.as_slice())
    }

    pub const fn default() -> Self {
        Self::NONE
    }

    /// Merges a set of constraints with another set, if they don't exist.
    /// This function should be only used on compile-time.
    #[inline(always)]
    pub const fn merge(self, rhs: Self) -> ([Constraint; SUPPORTED_CONSTRAINTS_COUNT], usize) {
        let mut si = 0;
        let rhs = rhs.0;
        let mut ri = 0;
        let mut array: [Constraint; SUPPORTED_CONSTRAINTS_COUNT] =
            [Constraint::Empty; SUPPORTED_CONSTRAINTS_COUNT];
        if rhs.len() > SUPPORTED_CONSTRAINTS_COUNT {
            panic!("Overriding constraint is larger than we have different constraint types")
        }
        // Copy rhs first to the array
        let mut copy_index = 0;
        while copy_index < rhs.len() {
            array[copy_index] = rhs[copy_index];
            copy_index += 1;
        }
        while si < self.0.len() {
            while ri < rhs.len() {
                if !self.0[si].kind().eq(&rhs[ri].kind()) {
                    if copy_index > SUPPORTED_CONSTRAINTS_COUNT {
                        panic!("attempting to copy into greater index than we have different constraint types")
                    }
                    array[copy_index] = self.0[si];
                    copy_index += 1;
                    ri += 1;
                } else {
                    ri += 1;
                }
            }
            si += 1;
        }
        (array, copy_index)
    }
    /// Overrides a set of constraints with another set.
    #[inline(always)]
    pub fn override_constraints(self, rhs: Constraints) -> Constraints {
        // self.merge(rhs)
        rhs
    }

    pub const fn size(&self) -> Option<&Extensible<Size>> {
        let mut i = 0;
        while i < self.0.len() {
            if let Some(size) = self.0[i].to_size() {
                return Some(size);
            }
            i += 1;
        }
        None
    }

    pub const fn permitted_alphabet(&self) -> Option<&Extensible<PermittedAlphabet>> {
        let mut i = 0;
        while i < self.0.len() {
            if let Some(alpha) = self.0[i].as_permitted_alphabet() {
                return Some(alpha);
            }
            i += 1;
        }
        None
    }

    pub const fn extensible(&self) -> bool {
        let mut i = 0;
        while i < self.0.len() {
            if self.0[i].is_extensible() {
                return true;
            }
            i += 1;
        }
        false
    }

    pub const fn value(&self) -> Option<&Extensible<Value>> {
        let mut i = 0;
        while i < self.0.len() {
            if let Some(value) = self.0[i].as_value() {
                return Some(value);
            }
            i += 1;
        }
        None
    }
}

impl From<&'static [Constraint]> for Constraints {
    fn from(constraints: &'static [Constraint]) -> Self {
        Self::new(constraints)
    }
}

impl<const N: usize> From<&'static [Constraint; N]> for Constraints {
    fn from(constraints: &'static [Constraint; N]) -> Self {
        Self::new(constraints)
    }
}

/// A constraint that can be applied to a type.
///
/// Do not change the amount of variants in this enum without changing the `SUPPORTED_CONSTRAINTS_COUNT` constant.
#[derive(Debug, Copy, Clone, PartialEq)]
pub enum Constraint {
    Value(Extensible<Value>),
    Size(Extensible<Size>),
    PermittedAlphabet(Extensible<PermittedAlphabet>),
    /// The value itself is extensible, only valid for constructed types,
    /// choices, or enumerated values.
    Extensible,
    Empty,
}

#[derive(Debug, Clone, Copy)]
pub enum ConstraintDiscriminant {
    Value,
    Size,
    PermittedAlphabet,
    Extensible,
    Empty,
}
impl ConstraintDiscriminant {
    pub const fn eq(&self, other: &ConstraintDiscriminant) -> bool {
        *self as isize == *other as isize
    }
}

impl Constraint {
    pub const fn kind(&self) -> ConstraintDiscriminant {
        match self {
            Self::Value(_) => ConstraintDiscriminant::Value,
            Self::Size(_) => ConstraintDiscriminant::Size,
            Self::PermittedAlphabet(_) => ConstraintDiscriminant::PermittedAlphabet,
            Self::Extensible => ConstraintDiscriminant::Extensible,
            Self::Empty => ConstraintDiscriminant::Empty,
        }
    }
    pub const fn default() -> Self {
        Self::Empty
    }
    pub const fn variant_as_isize(&self) -> isize {
        match self {
            Self::Value(_) => 0,
            Self::Size(_) => 1,
            Self::PermittedAlphabet(_) => 2,
            Self::Extensible => 3,
            Self::Empty => 4,
        }
    }

    pub const fn as_value(&self) -> Option<&Extensible<Value>> {
        match self {
            Self::Value(integer) => Some(integer),
            _ => None,
        }
    }

    pub const fn as_permitted_alphabet(&self) -> Option<&Extensible<PermittedAlphabet>> {
        match self {
            Self::PermittedAlphabet(alphabet) => Some(alphabet),
            _ => None,
        }
    }

    pub const fn to_size(&self) -> Option<&Extensible<Size>> {
        match self {
            Self::Size(size) => Some(size),
            _ => None,
        }
    }

    pub const fn to_value(self) -> Option<Extensible<Value>> {
        match self {
            Self::Value(integer) => Some(integer),
            _ => None,
        }
    }

    /// Returns whether the type is extensible.
    pub const fn is_extensible(&self) -> bool {
        match self {
            Self::Value(value) => value.extensible.is_some(),
            Self::Size(size) => size.extensible.is_some(),
            Self::PermittedAlphabet(alphabet) => alphabet.extensible.is_some(),
            Self::Extensible => true,
            Self::Empty => false,
        }
    }
}

#[derive(Debug, Default, Copy, Clone, PartialEq)]
pub struct Extensible<T: 'static> {
    pub constraint: T,
    /// Whether the constraint is extensible, and if it is, a list of extensible
    /// constraints.
    pub extensible: Option<&'static [T]>,
}

impl<T> Extensible<T> {
    pub const fn new(constraint: T) -> Self {
        Self {
            constraint,
            extensible: None,
        }
    }

    pub const fn new_extensible(constraint: T, constraints: &'static [T]) -> Self {
        Self {
            constraint,
            extensible: Some(constraints),
        }
    }

    pub const fn set_extensible(self, extensible: bool) -> Self {
        let extensible = if extensible {
            let empty: &[T] = &[];
            Some(empty)
        } else {
            None
        };

        self.extensible_with_constraints(extensible)
    }

    pub const fn extensible_with_constraints(mut self, constraints: Option<&'static [T]>) -> Self {
        self.extensible = constraints;
        self
    }
}

impl From<Value> for Extensible<Value> {
    fn from(value: Value) -> Self {
        Self {
            constraint: value,
            extensible: None,
        }
    }
}

impl From<Size> for Extensible<Size> {
    fn from(size: Size) -> Self {
        Self {
            constraint: size,
            extensible: None,
        }
    }
}

impl From<PermittedAlphabet> for Extensible<PermittedAlphabet> {
    fn from(alphabet: PermittedAlphabet) -> Self {
        Self {
            constraint: alphabet,
            extensible: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Value {
    /// Bound of the value
    pub(crate) value: Bounded<i128>,
    /// Sign of the bound, used for numeric values
    pub(crate) signed: bool,
    /// Range of the bound in bytes, used for numeric values
    pub(crate) range: Option<u8>,
}

impl Value {
    pub const fn new(value: Bounded<i128>) -> Self {
        let (signed, range) = value.range_in_bytes();
        Self {
            value,
            signed,
            range,
        }
    }
    pub const fn get_sign(&self) -> bool {
        self.signed
    }
    pub const fn get_range(&self) -> Option<u8> {
        self.range
    }
}

impl core::ops::Deref for Value {
    type Target = Bounded<i128>;

    fn deref(&self) -> &Self::Target {
        &self.value
    }
}

// impl core::ops::DerefMut for Value {
//     fn deref_mut(&mut self) -> &mut Self::Target {
//         &mut self.value
//     }
// }

macro_rules! from_primitives {
    ($($int:ty),+ $(,)?) => {
        $(
            impl From<Bounded<$int>> for Value {
                fn from(bounded: Bounded<$int>) -> Self {
                    Self::new(match bounded {
                        Bounded::Range { start, end } => Bounded::Range {
                            start: start.map(From::from),
                            end: end.map(From::from),
                        },
                        Bounded::Single(value) => Bounded::Single(value.into()),
                        Bounded::None => Bounded::None,
                    })
                }
            }
        )+
    }
}

from_primitives! {
    u8, u16, u32, u64,
    i8, i16, i32, i64, i128,
}

impl TryFrom<Bounded<usize>> for Value {
    type Error = <i128 as TryFrom<usize>>::Error;

    fn try_from(bounded: Bounded<usize>) -> Result<Self, Self::Error> {
        Ok(Self::new(match bounded {
            Bounded::Range { start, end } => Bounded::Range {
                start: start.map(TryFrom::try_from).transpose()?,
                end: end.map(TryFrom::try_from).transpose()?,
            },
            Bounded::Single(value) => Bounded::Single(value.try_into()?),
            Bounded::None => Bounded::None,
        }))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Size(pub(crate) Bounded<usize>);

impl Size {
    #[must_use]
    /// Creates a varying range constraint.
    pub const fn new(range: Bounded<usize>) -> Self {
        Self(range)
    }

    #[must_use]
    /// Creates a fixed size constraint.
    pub const fn fixed(length: usize) -> Self {
        Self(Bounded::Single(length))
    }

    #[must_use]
    /// Returns whether the size is fixed.
    pub const fn is_fixed(&self) -> bool {
        matches!(self.0, Bounded::Single(_))
    }
    /// Returns whether the size has a varying range.
    #[must_use]
    pub const fn is_range(&self) -> bool {
        matches!(self.0, Bounded::Range { .. })
    }
}

impl core::ops::Deref for Size {
    type Target = Bounded<usize>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl core::ops::DerefMut for Size {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PermittedAlphabet(&'static [u32]);

impl PermittedAlphabet {
    pub const fn new(range: &'static [u32]) -> Self {
        Self(range)
    }

    pub const fn as_inner(&self) -> &'static [u32] {
        self.0
    }
}

impl core::ops::Deref for PermittedAlphabet {
    type Target = [u32];

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Bounded<T> {
    #[default]
    None,
    Single(T),
    Range {
        start: Option<T>,
        end: Option<T>,
    },
}

impl<T> Bounded<T> {
    /// Calculates the the amount of bytes that are required to represent integer `value`.
    /// Particularly useful for OER codec
    #[inline(always)]
    const fn octet_size_by_range(value: i128) -> Option<u8> {
        let abs_value = value.unsigned_abs();
        Some(if abs_value <= u8::MAX as u128 {
            1
        } else if abs_value <= u16::MAX as u128 {
            2
        } else if abs_value <= u32::MAX as u128 {
            4
        } else if abs_value <= u64::MAX as u128 {
            8
        } else {
            return None;
        })
    }

    pub const fn start_from(value: T) -> Self {
        Self::Range {
            start: Some(value),
            end: None,
        }
    }

    pub const fn up_to(value: T) -> Self {
        Self::Range {
            start: None,
            end: Some(value),
        }
    }

    pub const fn as_start(&self) -> Option<&T> {
        match &self {
            Self::Range { start, .. } => start.as_ref(),
            Self::Single(value) => Some(value),
            _ => None,
        }
    }

    pub const fn as_end(&self) -> Option<&T> {
        match &self {
            Self::Range { end, .. } => end.as_ref(),
            Self::Single(value) => Some(value),
            _ => None,
        }
    }

    pub const fn start_and_end(&self) -> (Option<&T>, Option<&T>) {
        match &self {
            Self::Range { start, end } => (start.as_ref(), end.as_ref()),
            Self::Single(value) => (Some(value), Some(value)),
            _ => (None, None),
        }
    }

    pub const fn single_value(value: T) -> Self {
        Self::Single(value)
    }
}

impl<T: Copy + IntegerType> Bounded<T> {
    pub const fn as_minimum(&self) -> Option<&T> {
        match self {
            Self::Single(value) => Some(value),
            Self::Range {
                start: Some(start), ..
            } => Some(start),
            _ => None,
        }
    }

    pub const fn minimum(&self) -> T {
        match self.as_minimum() {
            Some(value) => *value,
            None => IntegerType::ZERO,
        }
    }
}

impl<T: num_traits::WrappingSub<Output = T> + num_traits::SaturatingAdd<Output = T> + From<u8>>
    Bounded<T>
{
    pub fn range(&self) -> Option<T> {
        match self {
            Self::Single(_) => Some(T::from(1u8)),
            Self::Range {
                start: Some(start),
                end: Some(end),
            } => Some(end.wrapping_sub(start).saturating_add(&1.into())),
            _ => None,
        }
    }
}

impl<T: core::ops::Sub<Output = T> + core::fmt::Debug + Default + core::cmp::PartialOrd> Bounded<T>
where
    for<'a> &'a T: core::ops::Sub<Output = T>,
{
    /// Returns the effective value which is either the number, or the positive
    /// offset of that number from the start of the value range. `Either::Left`
    /// represents the positive offset, and `Either::Right` represents
    /// the number.
    pub fn effective_value(&self, value: T) -> either::Either<T, T> {
        match self {
            Self::Range {
                start: Some(start), ..
            } => {
                debug_assert!(&value >= start);
                either::Left(&value - start)
            }
            _ => either::Right(value),
        }
    }
}

impl<T> Bounded<T>
where
    T: core::ops::Sub<Output = T> + core::fmt::Debug + Default + PartialOrd<T>,
{
    /// The same as [`effective_value`] except using [`crate::types::Integer<I>`].
    pub fn effective_integer_value<I>(self, value: I) -> either::Either<I, I>
    where
        I: IntegerType + core::ops::Sub<Output = I>,
        I: From<T> + PartialOrd,
    {
        if let Bounded::Range {
            start: Some(start), ..
        } = self
        {
            let start = I::from(start);
            debug_assert!(value >= start);
            either::Left(value - start)
        } else {
            either::Right(value)
        }
    }
}

impl From<Value> for Constraint {
    fn from(size: Value) -> Self {
        Self::Value(size.into())
    }
}

impl From<Extensible<Value>> for Constraint {
    fn from(size: Extensible<Value>) -> Self {
        Self::Value(size)
    }
}

impl From<Size> for Constraint {
    fn from(size: Size) -> Self {
        Self::Size(size.into())
    }
}

impl From<Extensible<Size>> for Constraint {
    fn from(size: Extensible<Size>) -> Self {
        Self::Size(size)
    }
}

impl From<PermittedAlphabet> for Constraint {
    fn from(size: PermittedAlphabet) -> Self {
        Self::PermittedAlphabet(size.into())
    }
}

impl From<Extensible<PermittedAlphabet>> for Constraint {
    fn from(size: Extensible<PermittedAlphabet>) -> Self {
        Self::PermittedAlphabet(size)
    }
}

impl Bounded<i128> {
    const fn max(&self, a: u128, b: u128) -> u128 {
        [a, b][(a < b) as usize]
    }
    pub const fn range_in_bytes(&self) -> (bool, Option<u8>) {
        match self {
            Self::Single(value) => (*value < 0, Self::octet_size_by_range(*value)),
            Self::Range {
                start: Some(start),
                end: Some(end),
            } => {
                let is_signed = *start < 0;
                let end_abs = end.unsigned_abs();
                let start_abs = start.unsigned_abs();
                let max = self.max(start_abs, end_abs);
                let octets = Self::octet_size_by_range(max as i128);
                (is_signed, octets)
            }
            Self::Range {
                start: Some(start),
                end: None,
            } => (*start < 0, None),
            Self::Range { start: None, .. } | Self::None => (true, None),
        }
    }
    /// Returns `true` if the given element is within the bounds of the constraint.
    /// Constraint type is `i128` here, so we can make checks based on that.
    pub fn in_bound<I: IntegerType>(&self, element: &I) -> bool {
        match &self {
            Self::Single(value) => {
                if let Some(e) = element.to_i128() {
                    e == *value
                } else {
                    false
                }
            }
            Self::Range { start, end } => {
                start.as_ref().map_or(true, |&start| {
                    if let Some(e) = element.to_i128() {
                        e >= start
                    } else if let Some(e) = element.to_bigint() {
                        e >= BigInt::from(start)
                    } else {
                        false
                    }
                }) && end.as_ref().map_or(true, |&end| {
                    if let Some(e) = element.to_i128() {
                        e <= end
                    } else if let Some(e) = element.to_bigint() {
                        e <= BigInt::from(end)
                    } else {
                        false
                    }
                })
            }
            Self::None => true,
        }
    }
}

impl<T: PartialEq + PartialOrd> Bounded<T> {
    /// Creates a new range from `start` to `end`.
    ///
    /// # Panics
    /// When `start > end`.
    pub fn new(start: T, end: T) -> Self {
        debug_assert!(start <= end);
        Self::const_new(start, end)
    }

    /// Const compatible range constructor.
    ///
    /// # Safety
    /// Requires `start <= end` otherwise functions will return incorrect results..
    /// In general you should prefer [`Self::new`] which has debug assertions
    /// to ensure this.
    pub const fn const_new(start: T, end: T) -> Self {
        Self::Range {
            start: Some(start),
            end: Some(end),
        }
    }

    pub fn contains(&self, element: &T) -> bool {
        match &self {
            Self::Single(value) => value == element,
            Self::Range { start, end } => {
                start.as_ref().map_or(true, |start| element >= start)
                    && end.as_ref().map_or(true, |end| element <= end)
            }
            Self::None => true,
        }
    }

    pub fn contains_or<E>(&self, element: &T, error: E) -> Result<(), E> {
        self.contains_or_else(element, || error)
    }

    pub fn contains_or_else<E>(&self, element: &T, error: impl FnOnce() -> E) -> Result<(), E> {
        match self.contains(element) {
            true => Ok(()),
            false => Err((error)()),
        }
    }
}

impl<T: core::fmt::Display> core::fmt::Display for Bounded<T> {
    fn fmt(&self, f: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::Range { start, end } => match (start.as_ref(), end.as_ref()) {
                (Some(start), Some(end)) => write!(f, "{start}..{end}"),
                (Some(start), None) => write!(f, "{start}.."),
                (None, Some(end)) => write!(f, "..{end}"),
                (None, None) => write!(f, ".."),
            },
            Self::Single(value) => value.fmt(f),
            Self::None => write!(f, ".."),
        }
    }
}
/// Helper macro to create constant value constraints.
///
/// Usage:
/// ```rust
// Full range
// let full_range = value_constraint!(0, 100);
// Only start specified
// let start_only = value_constraint!(start: 0);
// Only end specified
// let end_only = value_constraint!(end: 100);
// Single value
// let single = value_constraint!(42);
// Full range with extensibility
// let ext_full_range = value_constraint!(0, 100, true);
// Only start with extensibility
// let ext_start_only = value_constraint!(start: 0, true);
// Only end with extensibility
// let ext_end_only = value_constraint!(end: 100, false);
// Single value with extensibility
/// let ext_single = value_constraint!(42, true);
/// ```
#[macro_export]
macro_rules! value_constraint {
    ($($args:tt)*) => {
        $crate::bounded_constraint!(Value, $($args)*)
    };
}

#[macro_export]
macro_rules! size_constraint {
    ($($args:tt)*) => {
        $crate::bounded_constraint!(Size, $($args)*)
    };
}

/// Helper macro to create an array of constant constraints.
///
/// Usage:
/// ```rust
/// use rasn::prelude::*;
/// const CONSTRAINTS: Constraints = macros::constraints!(macros::value_constraint!(0, 100), macros::size_constraint!(0, 100));
/// ```
/// See other macros about the parameter usage.
#[macro_export]
macro_rules! constraints {
    ($($constraint:expr),+ $(,)?) => {
        $crate::types::constraints::Constraints(&[$($constraint),+])
    };
}

#[macro_export]
macro_rules! permitted_alphabet_constraint {
    ( $alphabet:expr) => {
        $crate::types::constraints::Constraint::PermittedAlphabet(
            $crate::types::constraints::Extensible::new(
                $crate::types::constraints::PermittedAlphabet::new($alphabet),
            ),
        )
    };
}

#[macro_export]
macro_rules! bounded_constraint {
    // Start and end
    ($constraint_type:ident, $start:expr, $end:expr) => {
        $crate::types::constraints::Constraint::$constraint_type(
            $crate::types::constraints::Extensible::new(
                $crate::types::constraints::$constraint_type::new(
                    $crate::types::constraints::Bounded::const_new($start, $end),
                ),
            ),
        )
    };

    // Only start provided
    ($constraint_type:ident, start: $start:expr) => {
        $crate::types::constraints::Constraint::$constraint_type(
            $crate::types::constraints::Extensible::new(
                $crate::types::constraints::$constraint_type::new(
                    $crate::types::constraints::Bounded::start_from($start),
                ),
            ),
        )
    };

    // Only end provided
    ($constraint_type:ident, end: $end:expr) => {
        $crate::types::constraints::Constraint::$constraint_type(
            $crate::types::constraints::Extensible::new(
                $crate::types::constraints::$constraint_type::new(
                    $crate::types::constraints::Bounded::up_to($end),
                ),
            ),
        )
    };

    // Single value
    ($constraint_type:ident, $single:expr) => {
        $crate::types::constraints::Constraint::$constraint_type(
            $crate::types::constraints::Extensible::new(
                $crate::types::constraints::$constraint_type::new(
                    $crate::types::constraints::Bounded::Single($single),
                ),
            ),
        )
    };

    // Range with extensibility
    ($constraint_type:ident, $start:expr, $end:expr, $extensible:expr) => {
        $crate::types::constraints::Constraint::$constraint_type(
            $crate::types::constraints::Extensible::new(
                $crate::types::constraints::$constraint_type::new(
                    $crate::types::constraints::Bounded::const_new($start, $end),
                ),
            )
            .set_extensible($extensible),
        )
    };

    // Only start with extensibility
    ($constraint_type:ident, start: $start:expr, $extensible:expr) => {
        $crate::types::constraints::Constraint::$constraint_type(
            $crate::types::constraints::Extensible::new(
                $crate::types::constraints::$constraint_type::new(
                    $crate::types::constraints::Bounded::start_from($start),
                ),
            )
            .set_extensible($extensible),
        )
    };

    // Only end with extensibility
    ($constraint_type:ident, end: $end:expr, $extensible:expr) => {
        $crate::types::constraints::Constraint::$constraint_type(
            $crate::types::constraints::Extensible::new(
                $crate::types::constraints::$constraint_type::new(
                    $crate::types::constraints::Bounded::up_to($end),
                ),
            )
            .set_extensible($extensible),
        )
    };

    // Single value with extensibility
    ($constraint_type:ident, $single:expr, $extensible:expr) => {
        $crate::types::constraints::Constraint::$constraint_type(
            $crate::types::constraints::Extensible::new(
                $crate::types::constraints::$constraint_type::new(
                    $crate::types::constraints::Bounded::Single($single),
                ),
            )
            .set_extensible($extensible),
        )
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn range() {
        let constraints = Bounded::new(0, 255);
        assert_eq!(256, constraints.range().unwrap());
    }
    // #[test]
    // fn test_concat_constraints() {
    //     let a = [Constraint::Value(Extensible::new(Value::new(Bounded::Single(0)))); 1];
    //     let b = [Constraint::Size(Extensible::new(Size::new(Bounded::Single(0)))); 1];
    //     let c = concat_constraints(a, b);

    //     const COMBINED_CONSTRAINTS: [Constraint; 2] = concat_constraints(a, b);

    //     const RESULT: &'static [Constraint] = &COMBINED_CONSTRAINTS;
    // }
}
