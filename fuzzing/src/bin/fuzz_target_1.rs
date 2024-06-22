#![no_main]

use fuzz::fuzz_oer;
use fuzz::fuzz_types::MissingCrlIdentifier;
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    // code to fuzz goes here
    fuzz_oer::<MissingCrlIdentifier>(data);
});
