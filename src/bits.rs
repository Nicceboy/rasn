//! Module for different bit modification functions which are used in the library.

use alloc::vec::Vec;
use num_traits::{Signed, Zero};

pub(crate) fn range_from_len(bit_length: u32) -> i128 {
    2i128.pow(bit_length) - 1
}

// Workaround for https://github.com/ferrilab/bitvec/issues/228
pub(crate) fn to_vec(slice: &bitvec::slice::BitSlice<u8, bitvec::order::Msb0>) -> Vec<u8> {
    use bitvec::prelude::*;
    let mut vec = Vec::new();

    for slice in slice.chunks(8) {
        vec.push(slice.load_be());
    }

    vec
}

pub(crate) fn to_left_padded_vec(
    slice: &bitvec::slice::BitSlice<u8, bitvec::order::Msb0>,
) -> Vec<u8> {
    use bitvec::prelude::*;

    let mut vec = Vec::new();

    let missing_bits = 8 - slice.len() % 8;
    if missing_bits == 8 {
        to_vec(slice)
    } else {
        let mut padding = bitvec::bitvec![u8, bitvec::prelude::Msb0; 0; missing_bits];
        padding.append(&mut slice.to_bitvec());
        for s in padding.chunks(8) {
            vec.push(s.load_be());
        }
        vec
    }
}
pub fn integer_to_bitvec_bytes(
    value: &crate::prelude::Integer,
    signed: bool,
) -> Option<bitvec::vec::BitVec<u8, bitvec::order::Msb0>> {
    if signed {
        Some(bitvec::vec::BitVec::<u8, bitvec::order::Msb0>::from_slice(
            &(value.to_signed_bytes_be()),
        ))
    } else if !signed && (value.is_positive() || value.is_zero()) {
        Some(bitvec::vec::BitVec::<u8, bitvec::order::Msb0>::from_slice(
            &(value.to_biguint().unwrap().to_bytes_be()),
        ))
    } else {
        None
    }
}
pub fn integer_to_bytes(value: &crate::prelude::Integer, signed: bool) -> Option<Vec<u8>> {
    if signed {
        Some(value.to_signed_bytes_be())
    } else if !signed && (value.is_positive() || value.is_zero()) {
        Some(value.to_biguint().unwrap().to_bytes_be())
    } else {
        None
    }
}
