//! Optional bearer-token (JWT) authentication for `POST /convert`.
//!
//! Off unless a key source is configured. The service only *verifies* tokens that an identity
//! provider issued: it checks the signature against the provider's public keys (a JWKS), then
//! the issuer, audience, expiry and, if asked, a scope. It never mints tokens and holds no
//! secrets.
//!
//! Choices that matter for safety:
//!
//! * Only `RS256` and `ES256` exist. There is no code path for `none` or the `HS*` family, so the
//!   classic "downgrade to a shared secret" attacks have nothing to land on.
//! * The token's `alg` must be on the configured list *and* match the key's type, so a token
//!   cannot choose how it is checked.
//! * Failures say only 401 or 403. A token, or any part of one, is never logged or echoed.
//! * Keys are fetched over HTTPS (plain HTTP only for a loopback address), with a short timeout,
//!   a size cap and no redirects. A token naming an unknown `kid` triggers at most one refetch
//!   per refresh interval, so garbage tokens cannot be used to hammer the identity provider.

use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::{
    extract::{Request, State},
    http::{header, HeaderValue, StatusCode},
    middleware::Next,
    response::Response,
};
use ring::signature;
use serde_json::Value;

/// A token longer than this is refused before any parsing.
const MAX_TOKEN_BYTES: usize = 8 * 1024;
/// The largest JWKS document accepted.
const MAX_JWKS_BYTES: u64 = 1024 * 1024;

/// A signature algorithm the service can verify.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Algorithm {
    /// RSASSA-PKCS1-v1_5 with SHA-256.
    Rs256,
    /// ECDSA over P-256 with SHA-256.
    Es256,
}

impl Algorithm {
    fn parse(name: &str) -> Option<Self> {
        match name {
            "RS256" => Some(Self::Rs256),
            "ES256" => Some(Self::Es256),
            _ => None,
        }
    }
}

/// Everything needed to turn authentication on.
#[derive(Debug, Clone)]
pub struct Settings {
    /// Where to fetch the provider's keys (`https://…/.well-known/jwks.json`).
    pub jwks_url: Option<String>,
    /// The keys inline, as the JWKS JSON itself. Fixed for the life of the process.
    pub jwks: Option<String>,
    /// The expected `iss` claim.
    pub issuer: String,
    /// The expected `aud` claim; a token passes if it lists this audience.
    pub audience: String,
    /// A scope the token must carry (`scope` as a space separated string, or `scp`).
    pub required_scope: Option<String>,
    /// Allowed algorithms.
    pub algorithms: Vec<Algorithm>,
    /// Clock skew tolerated on `exp` and `nbf`.
    pub leeway: Duration,
    /// Shortest gap between two fetches of the key set.
    pub min_refresh: Duration,
    /// Keys older than this are refetched on the next request.
    pub ttl: Duration,
}

impl Settings {
    /// Parse a comma separated algorithm list such as `RS256,ES256`.
    ///
    /// # Errors
    ///
    /// Any name other than `RS256` or `ES256`, or an empty list.
    pub fn parse_algorithms(list: &str) -> Result<Vec<Algorithm>, String> {
        let mut out = Vec::new();
        for name in list.split(',').map(str::trim).filter(|n| !n.is_empty()) {
            let alg = Algorithm::parse(name)
                .ok_or_else(|| format!("unsupported algorithm {name:?}; use RS256 or ES256"))?;
            if !out.contains(&alg) {
                out.push(alg);
            }
        }
        if out.is_empty() {
            return Err("no algorithms listed".into());
        }
        Ok(out)
    }
}

#[derive(Debug, Clone)]
enum Material {
    Rsa {
        n: Vec<u8>,
        e: Vec<u8>,
    },
    /// An uncompressed P-256 point: `0x04 || x || y`.
    Ec {
        point: Vec<u8>,
    },
}

#[derive(Debug, Clone)]
struct Key {
    kid: Option<String>,
    alg: Algorithm,
    material: Material,
}

#[derive(Debug, Default)]
struct Cache {
    keys: Vec<Key>,
    /// When the keys were last stored.
    loaded: Option<Instant>,
    /// When a fetch was last started (successful or not).
    attempted: Option<Instant>,
}

/// Why a request was turned away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Denied {
    /// No usable credentials: 401.
    Unauthorized,
    /// A valid token that lacks the required scope: 403.
    Forbidden,
}

/// Verifies bearer tokens.
#[derive(Debug)]
pub struct Authenticator {
    settings: Settings,
    cache: Mutex<Cache>,
}

fn b64url(input: &str) -> Option<Vec<u8>> {
    let mut out = Vec::with_capacity(input.len() * 3 / 4);
    let mut acc = 0u32;
    let mut bits = 0u32;
    for c in input.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push(u8::try_from((acc >> bits) & 0xff).ok()?);
        }
    }
    // Unused trailing bits must be zero, and a single leftover sextet is not valid.
    (input.len() % 4 != 1 && acc & ((1 << bits) - 1) == 0).then_some(out)
}

/// Read a JWKS document. Keys this service cannot use are skipped, not fatal.
fn parse_jwks(text: &str) -> Result<Vec<Key>, &'static str> {
    let doc: Value = serde_json::from_str(text).map_err(|_| "the key set is not JSON")?;
    let keys = doc
        .get("keys")
        .and_then(Value::as_array)
        .ok_or("the key set has no keys array")?;
    Ok(keys.iter().filter_map(parse_jwk).collect())
}

fn parse_jwk(jwk: &Value) -> Option<Key> {
    if jwk.get("use").is_some_and(|u| u != "sig") {
        return None;
    }
    let text = |name: &str| jwk.get(name).and_then(Value::as_str);
    let (alg, material) = match text("kty")? {
        "RSA" => (
            Algorithm::Rs256,
            Material::Rsa {
                n: b64url(text("n")?)?,
                e: b64url(text("e")?)?,
            },
        ),
        "EC" if text("crv") == Some("P-256") => {
            let x = b64url(text("x")?)?;
            let y = b64url(text("y")?)?;
            if x.len() != 32 || y.len() != 32 {
                return None;
            }
            let mut point = vec![4u8];
            point.extend_from_slice(&x);
            point.extend_from_slice(&y);
            (Algorithm::Es256, Material::Ec { point })
        }
        _ => return None,
    };
    // A key that names its own algorithm must name the one its type allows.
    if let Some(declared) = text("alg") {
        if Algorithm::parse(declared) != Some(alg) {
            return None;
        }
    }
    Some(Key {
        kid: text("kid").map(str::to_owned),
        alg,
        material,
    })
}

fn verify_signature(key: &Key, message: &[u8], sig: &[u8]) -> bool {
    match &key.material {
        Material::Rsa { n, e } => signature::RsaPublicKeyComponents {
            n: n.as_slice(),
            e: e.as_slice(),
        }
        .verify(&signature::RSA_PKCS1_2048_8192_SHA256, message, sig)
        .is_ok(),
        Material::Ec { point } => {
            signature::UnparsedPublicKey::new(&signature::ECDSA_P256_SHA256_FIXED, point)
                .verify(message, sig)
                .is_ok()
        }
    }
}

/// A token split into its parts, not yet trusted.
struct Parsed<'a> {
    alg: Algorithm,
    kid: Option<String>,
    claims: Value,
    signed: &'a [u8],
    signature: Vec<u8>,
}

fn parse_token<'a>(token: &'a str, allowed: &[Algorithm]) -> Option<Parsed<'a>> {
    if token.len() > MAX_TOKEN_BYTES {
        return None;
    }
    let mut parts = token.split('.');
    let (header, payload, sig) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    let header: Value = serde_json::from_slice(&b64url(header)?).ok()?;
    let alg = Algorithm::parse(header.get("alg")?.as_str()?).filter(|a| allowed.contains(a))?;
    let claims: Value = serde_json::from_slice(&b64url(payload)?).ok()?;
    claims.as_object()?;
    Some(Parsed {
        alg,
        kid: header.get("kid").and_then(Value::as_str).map(str::to_owned),
        claims,
        signed: &token.as_bytes()[..header_payload_len(token)],
        signature: b64url(sig)?,
    })
}

/// Length of `header.payload` in a token known to have three parts.
fn header_payload_len(token: &str) -> usize {
    token.rfind('.').unwrap_or(token.len())
}

fn has_scope(claims: &Value, wanted: &str) -> bool {
    let in_string = |v: &Value| {
        v.as_str()
            .is_some_and(|s| s.split_whitespace().any(|x| x == wanted))
    };
    claims.get("scope").is_some_and(in_string)
        || claims.get("scp").is_some_and(|v| {
            in_string(v)
                || v.as_array()
                    .is_some_and(|a| a.iter().any(|x| x.as_str() == Some(wanted)))
        })
}

impl Authenticator {
    /// Validate the settings and read any inline keys.
    ///
    /// # Errors
    ///
    /// No key source, both key sources, an unusable key set, a non-HTTPS key URL (other than a
    /// loopback one), or an empty issuer or audience.
    pub fn new(settings: Settings) -> Result<Self, String> {
        if settings.issuer.trim().is_empty() || settings.audience.trim().is_empty() {
            return Err("authentication needs both an issuer and an audience".into());
        }
        let mut cache = Cache::default();
        match (&settings.jwks_url, &settings.jwks) {
            (None, None) => return Err("authentication needs a key set (URL or inline)".into()),
            (Some(_), Some(_)) => {
                return Err("set either a key set URL or an inline key set, not both".into())
            }
            (Some(url), None) => {
                let loopback = ["http://localhost", "http://127.0.0.1", "http://[::1]"]
                    .iter()
                    .any(|p| url.starts_with(p));
                if !url.starts_with("https://") && !loopback {
                    return Err("the key set URL must be https".into());
                }
            }
            (None, Some(inline)) => {
                cache.keys = parse_jwks(inline).map_err(str::to_owned)?;
                if cache.keys.is_empty() {
                    return Err("the inline key set has no usable RS256 or ES256 keys".into());
                }
                cache.loaded = Some(Instant::now());
            }
        }
        Ok(Self {
            settings,
            cache: Mutex::new(cache),
        })
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Cache> {
        // A poisoned lock only means a panic elsewhere; the cache is still a plain value.
        self.cache
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }

    /// Fetch the key set now, if a URL is configured. Called once at start so the first request
    /// does not pay for it.
    pub async fn warm(&self) {
        self.refresh(true).await;
    }

    /// Refetch the keys. `force` ignores the minimum interval (used once, at start).
    async fn refresh(&self, force: bool) {
        let Some(url) = self.settings.jwks_url.clone() else {
            return;
        };
        {
            let mut cache = self.lock();
            let recent = cache
                .attempted
                .is_some_and(|t| t.elapsed() < self.settings.min_refresh);
            if recent && !force {
                return;
            }
            cache.attempted = Some(Instant::now());
        }
        let fetched = tokio::task::spawn_blocking(move || fetch(&url))
            .await
            .unwrap_or(Err("the key fetch did not finish"));
        match fetched.and_then(|text| parse_jwks(&text)) {
            Ok(keys) if !keys.is_empty() => {
                let mut cache = self.lock();
                cache.keys = keys;
                cache.loaded = Some(Instant::now());
                tracing::info!("key set loaded");
            }
            Ok(_) => tracing::warn!("the key set had no usable RS256 or ES256 keys"),
            Err(reason) => tracing::warn!(reason, "could not load the key set"),
        }
    }

    fn candidates(&self, parsed: &Parsed<'_>) -> Vec<Key> {
        self.lock()
            .keys
            .iter()
            .filter(|k| k.alg == parsed.alg)
            .filter(|k| parsed.kid.is_none() || k.kid == parsed.kid)
            .cloned()
            .collect()
    }

    /// Check a bearer token.
    ///
    /// # Errors
    ///
    /// [`Denied::Unauthorized`] for anything wrong with the token itself, and
    /// [`Denied::Forbidden`] for a genuine token without the required scope.
    pub async fn check(&self, token: &str) -> Result<(), Denied> {
        let parsed = parse_token(token, &self.settings.algorithms).ok_or(Denied::Unauthorized)?;
        let stale = self
            .lock()
            .loaded
            .is_some_and(|t| t.elapsed() > self.settings.ttl);
        if stale {
            self.refresh(false).await;
        }
        let mut keys = self.candidates(&parsed);
        if keys.is_empty() {
            // An unknown key id: the provider may have rotated. Rate limited inside.
            self.refresh(false).await;
            keys = self.candidates(&parsed);
        }
        self.verify_with(&parsed, &keys, unix_now())
    }

    fn verify_with(&self, parsed: &Parsed<'_>, keys: &[Key], now: u64) -> Result<(), Denied> {
        if !keys
            .iter()
            .any(|k| verify_signature(k, parsed.signed, &parsed.signature))
        {
            return Err(Denied::Unauthorized);
        }
        let claims = &parsed.claims;
        #[allow(clippy::cast_precision_loss)]
        let (now, leeway) = (now as f64, self.settings.leeway.as_secs_f64());
        let exp = claims
            .get("exp")
            .and_then(Value::as_f64)
            .ok_or(Denied::Unauthorized)?;
        if now > exp + leeway {
            return Err(Denied::Unauthorized);
        }
        if claims
            .get("nbf")
            .is_some_and(|v| v.as_f64().is_none_or(|nbf| now + leeway < nbf))
        {
            return Err(Denied::Unauthorized);
        }
        if claims.get("iss").and_then(Value::as_str) != Some(self.settings.issuer.as_str()) {
            return Err(Denied::Unauthorized);
        }
        let audience_ok = match claims.get("aud") {
            Some(Value::String(a)) => *a == self.settings.audience,
            Some(Value::Array(list)) => list
                .iter()
                .any(|a| a.as_str() == Some(self.settings.audience.as_str())),
            _ => false,
        };
        if !audience_ok {
            return Err(Denied::Unauthorized);
        }
        match &self.settings.required_scope {
            Some(scope) if !has_scope(claims, scope) => Err(Denied::Forbidden),
            _ => Ok(()),
        }
    }
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

fn fetch(url: &str) -> Result<String, &'static str> {
    let agent: ureq::Agent = ureq::Agent::config_builder()
        .timeout_global(Some(Duration::from_secs(5)))
        .max_redirects(0)
        .build()
        .into();
    let mut response = agent
        .get(url)
        .call()
        .map_err(|_| "the key request failed")?;
    response
        .body_mut()
        .with_config()
        .limit(MAX_JWKS_BYTES)
        .read_to_string()
        .map_err(|_| "the key response could not be read")
}

/// Middleware: let a request through only with a valid bearer token.
pub async fn require(
    State(auth): State<Arc<Authenticator>>,
    request: Request,
    next: Next,
) -> Response {
    let token = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| {
            let (scheme, rest) = v.split_once(' ')?;
            scheme
                .eq_ignore_ascii_case("bearer")
                .then(|| rest.trim().to_owned())
        });
    let Some(token) = token else {
        return denial(
            StatusCode::UNAUTHORIZED,
            "Bearer",
            "authentication required",
        );
    };
    match auth.check(&token).await {
        Ok(()) => next.run(request).await,
        Err(Denied::Unauthorized) => denial(
            StatusCode::UNAUTHORIZED,
            "Bearer error=\"invalid_token\"",
            "the access token is not valid",
        ),
        Err(Denied::Forbidden) => denial(
            StatusCode::FORBIDDEN,
            "Bearer error=\"insufficient_scope\"",
            "the access token does not grant this operation",
        ),
    }
}

fn denial(status: StatusCode, challenge: &'static str, message: &str) -> Response {
    let code = if status == StatusCode::FORBIDDEN {
        "forbidden"
    } else {
        "login"
    };
    let mut response = crate::server::outcome(status, code, message);
    response.headers_mut().insert(
        header::WWW_AUTHENTICATE,
        HeaderValue::from_static(challenge),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    use ring::rand::SystemRandom;
    use ring::signature::{EcdsaKeyPair, KeyPair, ECDSA_P256_SHA256_FIXED_SIGNING};

    const ISS: &str = "https://issuer.example";
    const AUD: &str = "ccda2fhir";

    // A synthetic RS256 token (exp in 2100) and the public half of the key that signed it.
    const RSA_N: &str = "wU04KExNx8ld3CEu_kJadMun-_BYOG8WGkvldQTBF2Tz_qKQXs1CTiBsWRJ4J_rRZ4wlsRb5Y5fS-SGbWeXfScav9589YK13b9MCYMcAB24m594LLM2DSFRdl3Ffk9eAL8R0LOKaspHBpG0W1L0jqthe473nJWyS5J1G33CVxEp5wHuK9bKiQylPKEr83Rl3ui1seRpvryR9DKakSNcMm05fj7p7xjfAvDsQfuFeebsKZHaCFm5HthAplHC6xgTR-16-QiouDpQTwiyyy8BBE3r9j2d7-BwnGwDmgVZyBbgp3FQPPEOOj7LuhDo7d5StrjWc91lQ8u_EjIRdwySWoQ";
    const RSA_TOKEN: &str = "eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCIsImtpZCI6InJzYS10ZXN0In0.eyJpc3MiOiJodHRwczovL2lzc3Vlci5leGFtcGxlIiwiYXVkIjoiY2NkYTJmaGlyIiwiZXhwIjo0MTAyNDQ0ODAwLCJzY29wZSI6ImNvbnZlcnQgb3RoZXIifQ.eFG9baDx8lfj9vno18Y3IqXCRXG2mmh-KZqiA2gl4AfOT2A0MpAuo8lbjYMeuQkdmNSMAA8f4zSmoHGXUbYBz9PWVyRgmw2p7rBBKlBqSsfJ962eF2dP1ACTqdNjzaEEZoFqhpwZC7-gGJ5V5toJ9zwP5C59Hr7xL4EsmeBWSUs6uMziATe3AlknNxL5dBUHJrK7JSWqVoo3xpS8TbQJpWHS07kKyKIXfo_qM8s53uX-4WJT6XWHEpTyr2O_-mN-xzbMy7DRMPf7GcN5NUG-bD6vmLeb_2TjhOhGQ_xs6cqBGTDeI2SnupIFguwv6OSCPce25kdJzVSgXkUuhhjfaw";

    fn b64_encode(bytes: &[u8]) -> String {
        const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789-_";
        let mut out = String::new();
        for chunk in bytes.chunks(3) {
            let n = chunk
                .iter()
                .enumerate()
                .fold(0u32, |a, (i, b)| a | (u32::from(*b) << (16 - 8 * i)));
            for i in 0..=chunk.len() {
                out.push(T[((n >> (18 - 6 * i)) & 63) as usize] as char);
            }
        }
        out
    }

    fn settings(jwks: String) -> Settings {
        Settings {
            jwks_url: None,
            jwks: Some(jwks),
            issuer: ISS.into(),
            audience: AUD.into(),
            required_scope: None,
            algorithms: vec![Algorithm::Rs256, Algorithm::Es256],
            leeway: Duration::from_secs(60),
            min_refresh: Duration::from_secs(300),
            ttl: Duration::from_secs(3600),
        }
    }

    fn rsa_jwks() -> String {
        format!(
            r#"{{"keys":[{{"kty":"RSA","kid":"rsa-test","use":"sig","alg":"RS256","n":"{RSA_N}","e":"AQAB"}}]}}"#
        )
    }

    /// An ES256 key pair, its JWKS, and a signer for claims.
    struct Es {
        pair: EcdsaKeyPair,
        jwks: String,
    }

    impl Es {
        fn new(kid: &str) -> Self {
            let rng = SystemRandom::new();
            let pkcs8 = EcdsaKeyPair::generate_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, &rng)
                .expect("generate");
            let pair =
                EcdsaKeyPair::from_pkcs8(&ECDSA_P256_SHA256_FIXED_SIGNING, pkcs8.as_ref(), &rng)
                    .expect("load");
            let point = pair.public_key().as_ref().to_vec();
            let jwks = format!(
                r#"{{"keys":[{{"kty":"EC","crv":"P-256","kid":"{kid}","x":"{}","y":"{}"}}]}}"#,
                b64_encode(&point[1..33]),
                b64_encode(&point[33..65])
            );
            Self { pair, jwks }
        }

        fn sign(&self, header: &str, claims: &str) -> String {
            let input = format!(
                "{}.{}",
                b64_encode(header.as_bytes()),
                b64_encode(claims.as_bytes())
            );
            let sig = self
                .pair
                .sign(&SystemRandom::new(), input.as_bytes())
                .expect("sign");
            format!("{input}.{}", b64_encode(sig.as_ref()))
        }
    }

    fn claims(extra: &str) -> String {
        format!(r#"{{"iss":"{ISS}","aud":"{AUD}","exp":4102444800{extra}}}"#)
    }

    fn check(auth: &Authenticator, token: &str, now: u64) -> Result<(), Denied> {
        let parsed = parse_token(token, &auth.settings.algorithms).ok_or(Denied::Unauthorized)?;
        let keys = auth.candidates(&parsed);
        auth.verify_with(&parsed, &keys, now)
    }

    const NOW: u64 = 1_800_000_000;

    #[test]
    fn a_valid_rs256_token_passes_and_a_tampered_one_does_not() {
        let auth = Authenticator::new(settings(rsa_jwks())).unwrap();
        assert_eq!(check(&auth, RSA_TOKEN, NOW), Ok(()));
        // Change one payload character: the signature no longer matches.
        let tampered = RSA_TOKEN.replacen("eyJpc3Mi", "eyJpc3Ni", 1);
        assert_eq!(check(&auth, &tampered, NOW), Err(Denied::Unauthorized));
    }

    #[test]
    fn es256_tokens_verify() {
        let es = Es::new("k1");
        let auth = Authenticator::new(settings(es.jwks.clone())).unwrap();
        let ok = es.sign(r#"{"alg":"ES256","kid":"k1"}"#, &claims(""));
        assert_eq!(check(&auth, &ok, NOW), Ok(()));
        // Without a kid, any key of the right type is tried.
        let no_kid = es.sign(r#"{"alg":"ES256"}"#, &claims(""));
        assert_eq!(check(&auth, &no_kid, NOW), Ok(()));
        // A key id the set does not hold finds no key.
        let wrong_kid = es.sign(r#"{"alg":"ES256","kid":"other"}"#, &claims(""));
        assert_eq!(check(&auth, &wrong_kid, NOW), Err(Denied::Unauthorized));
        // A different key signing under the same kid is rejected.
        let other = Es::new("k1");
        let forged = other.sign(r#"{"alg":"ES256","kid":"k1"}"#, &claims(""));
        assert_eq!(check(&auth, &forged, NOW), Err(Denied::Unauthorized));
    }

    #[test]
    fn none_and_hmac_tokens_are_refused() {
        let es = Es::new("k1");
        let auth = Authenticator::new(settings(es.jwks.clone())).unwrap();
        for alg in ["none", "HS256", "HS512", "PS256", "RS512"] {
            let token = es.sign(&format!(r#"{{"alg":"{alg}","kid":"k1"}}"#), &claims(""));
            assert_eq!(
                check(&auth, &token, NOW),
                Err(Denied::Unauthorized),
                "{alg}"
            );
        }
        let unsigned = format!(
            "{}.{}.",
            b64_encode(br#"{"alg":"none"}"#),
            b64_encode(claims("").as_bytes())
        );
        assert_eq!(check(&auth, &unsigned, NOW), Err(Denied::Unauthorized));
    }

    #[test]
    fn the_token_alg_must_match_the_key_type() {
        // An RSA key must not verify a token that claims ES256, and the reverse.
        let auth = Authenticator::new(settings(rsa_jwks())).unwrap();
        let es = Es::new("rsa-test");
        let token = es.sign(r#"{"alg":"ES256","kid":"rsa-test"}"#, &claims(""));
        assert_eq!(check(&auth, &token, NOW), Err(Denied::Unauthorized));
    }

    #[test]
    fn a_disabled_algorithm_is_refused() {
        let mut s = settings(rsa_jwks());
        s.algorithms = vec![Algorithm::Es256];
        let auth = Authenticator::new(s).unwrap();
        assert_eq!(check(&auth, RSA_TOKEN, NOW), Err(Denied::Unauthorized));
    }

    #[test]
    fn time_issuer_and_audience_are_enforced() {
        let es = Es::new("k");
        let auth = Authenticator::new(settings(es.jwks.clone())).unwrap();
        let h = r#"{"alg":"ES256","kid":"k"}"#;
        let sign = |c: String| es.sign(h, &c);

        let expired = sign(format!(r#"{{"iss":"{ISS}","aud":"{AUD}","exp":1000}}"#));
        assert_eq!(check(&auth, &expired, NOW), Err(Denied::Unauthorized));
        // Just inside the leeway still passes.
        let just = sign(format!(
            r#"{{"iss":"{ISS}","aud":"{AUD}","exp":{}}}"#,
            NOW - 30
        ));
        assert_eq!(check(&auth, &just, NOW), Ok(()));
        let no_exp = sign(format!(r#"{{"iss":"{ISS}","aud":"{AUD}"}}"#));
        assert_eq!(check(&auth, &no_exp, NOW), Err(Denied::Unauthorized));
        let early = sign(claims(&format!(r#","nbf":{}"#, NOW + 600)));
        assert_eq!(check(&auth, &early, NOW), Err(Denied::Unauthorized));
        let wrong_iss = sign(format!(
            r#"{{"iss":"https://evil.example","aud":"{AUD}","exp":4102444800}}"#
        ));
        assert_eq!(check(&auth, &wrong_iss, NOW), Err(Denied::Unauthorized));
        let wrong_aud = sign(format!(
            r#"{{"iss":"{ISS}","aud":"someone-else","exp":4102444800}}"#
        ));
        assert_eq!(check(&auth, &wrong_aud, NOW), Err(Denied::Unauthorized));
        let list_aud = sign(format!(
            r#"{{"iss":"{ISS}","aud":["x","{AUD}"],"exp":4102444800}}"#
        ));
        assert_eq!(check(&auth, &list_aud, NOW), Ok(()));
    }

    #[test]
    fn scope_is_checked_when_required() {
        let es = Es::new("k");
        let mut s = settings(es.jwks.clone());
        s.required_scope = Some("convert".into());
        let auth = Authenticator::new(s).unwrap();
        let h = r#"{"alg":"ES256","kid":"k"}"#;
        let with = es.sign(h, &claims(r#","scope":"openid convert""#));
        let with_scp = es.sign(h, &claims(r#","scp":["convert"]"#));
        let without = es.sign(h, &claims(r#","scope":"openid converter""#));
        let none = es.sign(h, &claims(""));
        assert_eq!(check(&auth, &with, NOW), Ok(()));
        assert_eq!(check(&auth, &with_scp, NOW), Ok(()));
        assert_eq!(check(&auth, &without, NOW), Err(Denied::Forbidden));
        assert_eq!(check(&auth, &none, NOW), Err(Denied::Forbidden));
        // A bad signature is never reported as a scope problem.
        let forged = Es::new("k").sign(h, &claims(""));
        assert_eq!(check(&auth, &forged, NOW), Err(Denied::Unauthorized));
    }

    #[test]
    fn junk_and_oversized_tokens_are_unauthorized() {
        let auth = Authenticator::new(settings(rsa_jwks())).unwrap();
        for t in ["", "a", "a.b", "a.b.c.d", "!!!.???.***", "e30.e30.AAAA"] {
            assert_eq!(check(&auth, t, NOW), Err(Denied::Unauthorized), "{t}");
        }
        let huge = format!("{}.{}.{}", "a".repeat(9000), "b", "c");
        assert_eq!(check(&auth, &huge, NOW), Err(Denied::Unauthorized));
    }

    #[tokio::test]
    async fn keys_are_fetched_from_a_url_and_unknown_kids_do_not_hammer_it() {
        use std::io::{Read, Write};
        use std::sync::atomic::{AtomicUsize, Ordering};

        let es = Es::new("k1");
        let hits = Arc::new(AtomicUsize::new(0));
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (body, counter) = (es.jwks.clone(), Arc::clone(&hits));
        std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let mut buf = [0u8; 1024];
                let _ = stream.read(&mut buf);
                counter.fetch_add(1, Ordering::SeqCst);
                let reply = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
                let _ = stream.write_all(reply.as_bytes());
            }
        });

        let mut s = settings(String::new());
        s.jwks = None;
        s.jwks_url = Some(format!("http://127.0.0.1:{port}/jwks"));
        let auth = Authenticator::new(s).unwrap();

        // The first token finds no keys, so the set is fetched once.
        let good = es.sign(r#"{"alg":"ES256","kid":"k1"}"#, &claims(""));
        assert_eq!(auth.check(&good).await, Ok(()));
        assert_eq!(hits.load(Ordering::SeqCst), 1);

        // Tokens naming a kid the provider does not have must not trigger more fetches.
        for i in 0..5 {
            let bogus = es.sign(
                &format!(r#"{{"alg":"ES256","kid":"nope{i}"}}"#),
                &claims(""),
            );
            assert_eq!(auth.check(&bogus).await, Err(Denied::Unauthorized));
        }
        assert_eq!(hits.load(Ordering::SeqCst), 1);
        assert_eq!(auth.check(&good).await, Ok(()));
    }

    #[test]
    fn settings_are_validated() {
        let mut s = settings(rsa_jwks());
        s.issuer = String::new();
        assert!(Authenticator::new(s).is_err());
        let mut s = settings(rsa_jwks());
        s.jwks_url = Some("https://idp.example/jwks".into());
        assert!(Authenticator::new(s).is_err(), "both sources");
        let mut s = settings(rsa_jwks());
        s.jwks = None;
        assert!(Authenticator::new(s.clone()).is_err(), "no source");
        s.jwks_url = Some("http://idp.example/jwks".into());
        assert!(Authenticator::new(s.clone()).is_err(), "plain http");
        s.jwks_url = Some("http://localhost:9000/jwks".into());
        assert!(Authenticator::new(s.clone()).is_ok(), "loopback http");
        s.jwks_url = Some("https://idp.example/jwks".into());
        assert!(Authenticator::new(s).is_ok());
        assert!(Authenticator::new(settings(r#"{"keys":[]}"#.into())).is_err());
        assert!(Authenticator::new(settings("nope".into())).is_err());
    }

    #[test]
    fn algorithm_lists_reject_unsafe_names() {
        assert_eq!(
            Settings::parse_algorithms("RS256, ES256").unwrap(),
            vec![Algorithm::Rs256, Algorithm::Es256]
        );
        for bad in ["none", "HS256", "RS256,HS256", ""] {
            assert!(Settings::parse_algorithms(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn unusable_keys_are_skipped() {
        let jwks = format!(
            r#"{{"keys":[
              {{"kty":"oct","k":"c2VjcmV0"}},
              {{"kty":"RSA","use":"enc","n":"{RSA_N}","e":"AQAB"}},
              {{"kty":"RSA","alg":"ES256","n":"{RSA_N}","e":"AQAB"}},
              {{"kty":"EC","crv":"P-384","x":"AA","y":"AA"}},
              {{"kty":"RSA","kid":"ok","n":"{RSA_N}","e":"AQAB"}}]}}"#
        );
        let keys = parse_jwks(&jwks).unwrap();
        assert_eq!(keys.len(), 1);
        assert_eq!(keys[0].kid.as_deref(), Some("ok"));
    }

    #[test]
    fn base64url_decodes_and_rejects_bad_input() {
        assert_eq!(b64url("aGk").unwrap(), b"hi");
        assert_eq!(b64url("").unwrap(), b"");
        assert!(b64url("a").is_none());
        assert!(b64url("aG+k").is_none());
        assert!(b64url("aGk=").is_none());
        assert_eq!(b64_encode(b"hi"), "aGk");
    }

    mod http {
        use super::*;
        use crate::server::{router, Config};
        use axum::body::Body;
        use axum::http::Request;
        use http_body_util::BodyExt;
        use tower::ServiceExt;

        const XML: &str = r#"<ClinicalDocument xmlns="urn:hl7-org:v3"/>"#;

        fn config(scope: Option<&str>) -> Config {
            let mut s = settings(rsa_jwks());
            s.required_scope = scope.map(str::to_owned);
            Config {
                auth: Some(Arc::new(Authenticator::new(s).unwrap())),
                ..Config::default()
            }
        }

        async fn send(
            config: &Config,
            method: &str,
            uri: &str,
            bearer: Option<&str>,
        ) -> (StatusCode, Option<String>, String) {
            let mut b = Request::builder().method(method).uri(uri);
            if let Some(v) = bearer {
                b = b.header(header::AUTHORIZATION, v);
            }
            let response = router(config)
                .oneshot(b.body(Body::from(XML)).unwrap())
                .await
                .unwrap();
            let status = response.status();
            let challenge = response
                .headers()
                .get(header::WWW_AUTHENTICATE)
                .map(|v| v.to_str().unwrap().to_owned());
            let body = response.into_body().collect().await.unwrap().to_bytes();
            (
                status,
                challenge,
                String::from_utf8_lossy(&body).into_owned(),
            )
        }

        #[tokio::test]
        async fn convert_needs_a_valid_token_and_health_stays_open() {
            let cfg = config(None);
            let (status, challenge, _) = send(&cfg, "POST", "/convert", None).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
            assert_eq!(challenge.as_deref(), Some("Bearer"));

            let secret = "Bearer not.a.token-SECRET";
            let (status, challenge, body) = send(&cfg, "POST", "/convert", Some(secret)).await;
            assert_eq!(status, StatusCode::UNAUTHORIZED);
            assert_eq!(challenge.as_deref(), Some("Bearer error=\"invalid_token\""));
            assert!(!body.contains("SECRET"), "the token is never echoed");
            assert!(body.contains("OperationOutcome"));

            let ok = format!("bearer {RSA_TOKEN}");
            let (status, _, body) = send(&cfg, "POST", "/convert", Some(&ok)).await;
            assert_eq!(status, StatusCode::OK, "{body}");

            for path in ["/healthz", "/readyz"] {
                let (status, _, _) = send(&cfg, "GET", path, None).await;
                assert_eq!(status, StatusCode::OK, "{path}");
            }
        }

        #[tokio::test]
        async fn a_missing_scope_is_403() {
            let ok = format!("Bearer {RSA_TOKEN}");
            let (status, challenge, _) =
                send(&config(Some("convert")), "POST", "/convert", Some(&ok)).await;
            assert_eq!(status, StatusCode::OK);
            assert!(challenge.is_none());
            let (status, challenge, _) =
                send(&config(Some("admin")), "POST", "/convert", Some(&ok)).await;
            assert_eq!(status, StatusCode::FORBIDDEN);
            assert_eq!(
                challenge.as_deref(),
                Some("Bearer error=\"insufficient_scope\"")
            );
        }

        #[tokio::test]
        async fn without_auth_configured_convert_is_open() {
            let (status, _, _) = send(&Config::default(), "POST", "/convert", None).await;
            assert_eq!(status, StatusCode::OK);
        }
    }
}
