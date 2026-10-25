//! Feed arbitrary bytes to the whole conversion. The only property: it must not panic, hang or
//! run out of memory. Anything else (an error, a report entry) is fine.

#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(conversion) = ccda2fhir::convert(data, &ccda2fhir::Options::default()) {
        // Serialization is part of the contract too.
        let _ = conversion.bundle.to_json().to_string();
    }
});
