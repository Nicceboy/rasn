extern crate afl;

// Main function picked by AFL to fuzz when running `cargo afl fuzz`.
// Actual logic is in `lib.rs`.
fn main() {
    afl::fuzz!(|data: &[u8]| {
        fuzz::fuzz(data);
    });
}
