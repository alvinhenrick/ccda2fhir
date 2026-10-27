//! The converter as a WebAssembly module: the same code as the library and the service, running in
//! the browser or in Node, so a document never has to leave the machine.
//!
//! ```js
//! import init, { convert_ccda } from "./ccda2fhir_wasm.js";
//! await init();
//! const result = JSON.parse(convert_ccda(xmlText, "us-core"));
//! result.status;   // "complete" or "partial"
//! result.bundle;   // the FHIR R4 Bundle
//! result.report;   // [{ severity, code, message }]
//! ```
//!
//! The result has the same shape as `POST /convert?report=true`. Nothing is logged and nothing is
//! sent anywhere.

use ccda2fhir::{convert, Options};
use ccda_map::Profiles;
use wasm_bindgen::prelude::*;

/// Convert a C-CDA document (XML text) and return the response JSON as a string.
///
/// `profiles` is `"none"` to declare no profiles; anything else (or nothing) declares US Core 6.1.0.
///
/// # Errors
///
/// A message (never any document content) if the input is not a parseable C-CDA document.
pub fn convert_to_json(xml: &str, profiles: Option<&str>) -> Result<String, String> {
    let options = Options {
        profiles: if profiles == Some("none") {
            Profiles::None
        } else {
            Profiles::UsCore610
        },
        ..Options::default()
    };
    let conversion = convert(xml.as_bytes(), &options).map_err(|e| e.to_string())?;
    Ok(conversion.to_response_json().to_string())
}

/// JavaScript entry point: see the module documentation. Throws an `Error` with a content-free
/// message when the document cannot be parsed.
///
/// # Errors
///
/// As [`convert_to_json`].
#[wasm_bindgen]
pub fn convert_ccda(xml: &str, profiles: Option<String>) -> Result<String, JsError> {
    convert_to_json(xml, profiles.as_deref()).map_err(|m| JsError::new(&m))
}

/// The converter's version.
#[wasm_bindgen]
#[must_use]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_owned()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used)]
    use super::*;

    const DOC: &str = r#"<ClinicalDocument xmlns="urn:hl7-org:v3">
  <recordTarget><patientRole><patient>
    <name><given>Alice</given><family>Example</family></name>
    <administrativeGenderCode code="F"/><birthTime value="19700101"/>
  </patient></patientRole></recordTarget>
</ClinicalDocument>"#;

    #[test]
    fn returns_the_service_shape() {
        let out: serde_json::Value =
            serde_json::from_str(&convert_to_json(DOC, None).unwrap()).unwrap();
        assert!(["complete", "partial"].contains(&out["status"].as_str().unwrap()));
        assert_eq!(out["bundle"]["resourceType"], "Bundle");
        assert!(out["report"].is_array());
    }

    #[test]
    fn profiles_none_declares_no_profile() {
        let with: serde_json::Value =
            serde_json::from_str(&convert_to_json(DOC, Some("us-core")).unwrap()).unwrap();
        let without: serde_json::Value =
            serde_json::from_str(&convert_to_json(DOC, Some("none")).unwrap()).unwrap();
        assert!(with["bundle"]["entry"][0]["resource"]["meta"]["profile"].is_array());
        assert!(without["bundle"]["entry"][0]["resource"]["meta"].is_null());
    }

    #[test]
    fn a_bad_document_is_an_error_without_its_content() {
        let err = convert_to_json("<Secret-Name/>", None).unwrap_err();
        assert!(!err.contains("Secret"));
    }

    #[test]
    fn version_is_reported() {
        assert_eq!(version(), env!("CARGO_PKG_VERSION"));
    }
}
