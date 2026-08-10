//! Transient Pair Setup (HAP M1-M4 with the fixed 3939 code).
//!
//! Receivers that advertise `SupportsSystemPairing` (bit 43) or
//! `SupportsCoreUtilsPairingAndEncryption` (bit 48) -- which includes every
//! third-party AirPlay 2 TV tested so far -- accept a session that is
//! established without any stored pairing at all: SRP runs only through M1-M4
//! against the well-known code 3939, and the resulting SRP session key `K` is
//! the shared secret every channel key is derived from.
//!
//! `hap-crypto` keeps its SRP client crate-private and its `PairSetupClient`
//! always runs the full M1-M6 exchange, so the SRP-6a client is implemented
//! here against the same HAP conventions: RFC 5054 Appendix A 3072-bit group,
//! `g = 5`, SHA-512, username `Pair-Setup`.

use anyhow::{Context, Result};
use hap_tlv8::{Tlv8Map, Tlv8Writer};
use num_bigint::BigUint;
use sha2::{Digest, Sha512};

const METHOD: u8 = 0x00;
const PUBLIC_KEY: u8 = 0x03;
const PROOF: u8 = 0x04;
const STATE: u8 = 0x06;
const ERROR: u8 = 0x07;
const SALT: u8 = 0x02;
const FLAGS: u8 = 0x13;

const TRANSIENT_PAIRING: u8 = 0x10;
/// The fixed setup code a transient exchange authenticates against.
const TRANSIENT_SETUP_CODE: &[u8] = b"3939";
const USERNAME: &[u8] = b"Pair-Setup";

/// RFC 5054 Appendix A 3072-bit group modulus, as used by HAP.
const N_3072: [u8; 384] = [
    0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xC9, 0x0F, 0xDA, 0xA2, 0x21, 0x68, 0xC2, 0x34,
    0xC4, 0xC6, 0x62, 0x8B, 0x80, 0xDC, 0x1C, 0xD1, 0x29, 0x02, 0x4E, 0x08, 0x8A, 0x67, 0xCC, 0x74,
    0x02, 0x0B, 0xBE, 0xA6, 0x3B, 0x13, 0x9B, 0x22, 0x51, 0x4A, 0x08, 0x79, 0x8E, 0x34, 0x04, 0xDD,
    0xEF, 0x95, 0x19, 0xB3, 0xCD, 0x3A, 0x43, 0x1B, 0x30, 0x2B, 0x0A, 0x6D, 0xF2, 0x5F, 0x14, 0x37,
    0x4F, 0xE1, 0x35, 0x6D, 0x6D, 0x51, 0xC2, 0x45, 0xE4, 0x85, 0xB5, 0x76, 0x62, 0x5E, 0x7E, 0xC6,
    0xF4, 0x4C, 0x42, 0xE9, 0xA6, 0x37, 0xED, 0x6B, 0x0B, 0xFF, 0x5C, 0xB6, 0xF4, 0x06, 0xB7, 0xED,
    0xEE, 0x38, 0x6B, 0xFB, 0x5A, 0x89, 0x9F, 0xA5, 0xAE, 0x9F, 0x24, 0x11, 0x7C, 0x4B, 0x1F, 0xE6,
    0x49, 0x28, 0x66, 0x51, 0xEC, 0xE4, 0x5B, 0x3D, 0xC2, 0x00, 0x7C, 0xB8, 0xA1, 0x63, 0xBF, 0x05,
    0x98, 0xDA, 0x48, 0x36, 0x1C, 0x55, 0xD3, 0x9A, 0x69, 0x16, 0x3F, 0xA8, 0xFD, 0x24, 0xCF, 0x5F,
    0x83, 0x65, 0x5D, 0x23, 0xDC, 0xA3, 0xAD, 0x96, 0x1C, 0x62, 0xF3, 0x56, 0x20, 0x85, 0x52, 0xBB,
    0x9E, 0xD5, 0x29, 0x07, 0x70, 0x96, 0x96, 0x6D, 0x67, 0x0C, 0x35, 0x4E, 0x4A, 0xBC, 0x98, 0x04,
    0xF1, 0x74, 0x6C, 0x08, 0xCA, 0x18, 0x21, 0x7C, 0x32, 0x90, 0x5E, 0x46, 0x2E, 0x36, 0xCE, 0x3B,
    0xE3, 0x9E, 0x77, 0x2C, 0x18, 0x0E, 0x86, 0x03, 0x9B, 0x27, 0x83, 0xA2, 0xEC, 0x07, 0xA2, 0x8F,
    0xB5, 0xC5, 0x5D, 0xF0, 0x6F, 0x4C, 0x52, 0xC9, 0xDE, 0x2B, 0xCB, 0xF6, 0x95, 0x58, 0x17, 0x18,
    0x39, 0x95, 0x49, 0x7C, 0xEA, 0x95, 0x6A, 0xE5, 0x15, 0xD2, 0x26, 0x18, 0x98, 0xFA, 0x05, 0x10,
    0x15, 0x72, 0x8E, 0x5A, 0x8A, 0xAA, 0xC4, 0x2D, 0xAD, 0x33, 0x17, 0x0D, 0x04, 0x50, 0x7A, 0x33,
    0xA8, 0x55, 0x21, 0xAB, 0xDF, 0x1C, 0xBA, 0x64, 0xEC, 0xFB, 0x85, 0x04, 0x58, 0xDB, 0xEF, 0x0A,
    0x8A, 0xEA, 0x71, 0x57, 0x5D, 0x06, 0x0C, 0x7D, 0xB3, 0x97, 0x0F, 0x85, 0xA6, 0xE1, 0xE4, 0xC7,
    0xAB, 0xF5, 0xAE, 0x8C, 0xDB, 0x09, 0x33, 0xD7, 0x1E, 0x8C, 0x94, 0xE0, 0x4A, 0x25, 0x61, 0x9D,
    0xCE, 0xE3, 0xD2, 0x26, 0x1A, 0xD2, 0xEE, 0x6B, 0xF1, 0x2F, 0xFA, 0x06, 0xD9, 0x8A, 0x08, 0x64,
    0xD8, 0x76, 0x02, 0x73, 0x3E, 0xC8, 0x6A, 0x64, 0x52, 0x1F, 0x2B, 0x18, 0x17, 0x7B, 0x20, 0x0C,
    0xBB, 0xE1, 0x17, 0x57, 0x7A, 0x61, 0x5D, 0x6C, 0x77, 0x09, 0x88, 0xC0, 0xBA, 0xD9, 0x46, 0xE2,
    0x08, 0xE2, 0x4F, 0xA0, 0x74, 0xE5, 0xAB, 0x31, 0x43, 0xDB, 0x5B, 0xFC, 0xE0, 0xFD, 0x10, 0x8E,
    0x4B, 0x82, 0xD1, 0x20, 0xA9, 0x3A, 0xD2, 0xCA, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF,
];

/// The generator for the HAP group.
const GENERATOR: u8 = 5;

pub struct TransientPairing {
    a_private: BigUint,
    a_public: BigUint,
    state: Option<Pending>,
}

struct Pending {
    session_key: Vec<u8>,
    proof: Vec<u8>,
}

impl TransientPairing {
    pub fn new() -> Result<Self> {
        let mut seed = [0u8; 32];
        getrandom::fill(&mut seed).map_err(|error| anyhow::anyhow!("{error}"))?;
        let a_private = BigUint::from_bytes_be(&seed);
        let a_public = generator().modpow(&a_private, &modulus());
        anyhow::ensure!(
            !(&a_public % modulus()).eq(&BigUint::ZERO),
            "SRP public ephemeral A is zero mod N"
        );
        Ok(Self {
            a_private,
            a_public,
            state: None,
        })
    }

    /// M1: start a transient exchange. The `Flags` TLV is what distinguishes it
    /// from a regular PIN pairing.
    pub fn start(&self) -> Vec<u8> {
        let mut body = Vec::new();
        let mut writer = Tlv8Writer::new(&mut body);
        writer.push_u8(STATE, 1);
        writer.push_u8(METHOD, 0);
        writer.push_u8(FLAGS, TRANSIENT_PAIRING);
        body
    }

    /// M2 -> M3: consume the receiver's salt and `B`, return `A` plus our proof.
    pub fn handle_m2(&mut self, response: &[u8]) -> Result<Vec<u8>> {
        let map = checked_map(response, 2)?;
        let salt = map
            .get(SALT)
            .context("AirPlay transient pairing M2 omitted the salt")?
            .to_vec();
        let b_public = BigUint::from_bytes_be(
            map.get(PUBLIC_KEY)
                .context("AirPlay transient pairing M2 omitted the public key")?,
        );
        let n = modulus();
        anyhow::ensure!(
            !(&b_public % &n).eq(&BigUint::ZERO),
            "AirPlay receiver sent a zero SRP public key"
        );

        let k = compute_k();
        let x = compute_x(&salt, TRANSIENT_SETUP_CODE);
        let u = compute_u(&self.a_public, &b_public);
        anyhow::ensure!(u != BigUint::ZERO, "SRP scrambling parameter is zero");

        // base = (B - k * g^x) mod N, kept non-negative by adding N first.
        let gx = generator().modpow(&x, &n);
        let kgx = (&k * &gx) % &n;
        let base = (&b_public + &n - kgx) % &n;
        let premaster = base.modpow(&(&self.a_private + (&u * &x)), &n);
        let session_key = sha512(&[&pad(&premaster)]);
        let proof = proof_m1(&salt, &self.a_public, &b_public, &session_key);

        let mut body = Vec::new();
        let mut writer = Tlv8Writer::new(&mut body);
        writer.push_u8(STATE, 3);
        writer.push(PUBLIC_KEY, &pad(&self.a_public));
        writer.push(PROOF, &proof);
        self.state = Some(Pending { session_key, proof });
        Ok(body)
    }

    /// M4: verify the receiver's proof and yield the SRP session key.
    pub fn finish(self, response: &[u8]) -> Result<Vec<u8>> {
        let map = checked_map(response, 4)?;
        let pending = self
            .state
            .context("AirPlay transient pairing finished before M2")?;
        let received = map
            .get(PROOF)
            .context("AirPlay transient pairing M4 omitted the receiver proof")?;
        let expected = proof_m2(&self.a_public, &pending.proof, &pending.session_key);
        anyhow::ensure!(
            constant_time_eq(&expected, received),
            "AirPlay receiver proof did not verify"
        );
        Ok(pending.session_key)
    }
}

fn modulus() -> BigUint {
    BigUint::from_bytes_be(&N_3072)
}

fn generator() -> BigUint {
    BigUint::from(GENERATOR)
}

/// Left-pad to the width of N, which is what every HAP `PAD()` targets.
fn pad(value: &BigUint) -> Vec<u8> {
    let raw = value.to_bytes_be();
    if raw.len() >= N_3072.len() {
        return raw;
    }
    let mut padded = vec![0u8; N_3072.len() - raw.len()];
    padded.extend_from_slice(&raw);
    padded
}

fn sha512(parts: &[&[u8]]) -> Vec<u8> {
    let mut hasher = Sha512::new();
    for part in parts {
        hasher.update(part);
    }
    hasher.finalize().to_vec()
}

/// `k = H(N | PAD(g))`
fn compute_k() -> BigUint {
    BigUint::from_bytes_be(&sha512(&[&N_3072, &pad(&generator())]))
}

/// `x = H(s | H(I | ":" | P))`
fn compute_x(salt: &[u8], password: &[u8]) -> BigUint {
    let inner = sha512(&[USERNAME, b":", password]);
    BigUint::from_bytes_be(&sha512(&[salt, &inner]))
}

/// `u = H(PAD(A) | PAD(B))`
fn compute_u(a_public: &BigUint, b_public: &BigUint) -> BigUint {
    BigUint::from_bytes_be(&sha512(&[&pad(a_public), &pad(b_public)]))
}

/// `M1 = H( H(N) XOR H(g) | H(I) | s | PAD(A) | PAD(B) | K )`
fn proof_m1(salt: &[u8], a_public: &BigUint, b_public: &BigUint, session_key: &[u8]) -> Vec<u8> {
    let hash_n = sha512(&[&N_3072]);
    let hash_g = sha512(&[&generator().to_bytes_be()]);
    let xored: Vec<u8> = hash_n
        .iter()
        .zip(hash_g.iter())
        .map(|(left, right)| left ^ right)
        .collect();
    let hash_username = sha512(&[USERNAME]);
    sha512(&[
        &xored,
        &hash_username,
        salt,
        &pad(a_public),
        &pad(b_public),
        session_key,
    ])
}

/// `M2 = H( PAD(A) | M1 | K )`
fn proof_m2(a_public: &BigUint, proof: &[u8], session_key: &[u8]) -> Vec<u8> {
    sha512(&[&pad(a_public), proof, session_key])
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.iter()
        .zip(right.iter())
        .fold(0u8, |accumulator, (a, b)| accumulator | (a ^ b))
        == 0
}

fn checked_map(response: &[u8], expected_state: u8) -> Result<Tlv8Map> {
    let map = Tlv8Map::parse(response)?;
    if let Some(error) = map.get(ERROR).and_then(|value| value.first()) {
        anyhow::bail!("AirPlay receiver rejected transient pairing with error {error}");
    }
    anyhow::ensure!(
        map.get(STATE) == Some(&[expected_state][..]),
        "AirPlay transient pairing response had an unexpected state"
    );
    Ok(map)
}

#[cfg(test)]
mod tests {
    use super::*;
    use hap_crypto::HapPairSetupSrpServer;

    fn tlv(entries: &[(u8, Vec<u8>)]) -> Vec<u8> {
        let mut body = Vec::new();
        let mut writer = Tlv8Writer::new(&mut body);
        for (kind, value) in entries {
            writer.push(*kind, value);
        }
        body
    }

    /// Drive the client against `hap-crypto`'s own SRP server. If the hand-rolled
    /// SRP-6a here diverges from the HAP conventions in any detail -- the group,
    /// the hash, `PAD()`, the proof layout -- the proofs stop matching.
    #[test]
    fn srp_client_interoperates_with_a_hap_server() {
        let (server, salt) = HapPairSetupSrpServer::new("3939").unwrap();
        let mut client = TransientPairing::new().unwrap();

        let start = Tlv8Map::parse(&client.start()).unwrap();
        assert_eq!(start.get(STATE), Some(&[1u8][..]));
        assert_eq!(start.get(FLAGS), Some(&[TRANSIENT_PAIRING][..]));

        let m2 = tlv(&[
            (STATE, vec![2]),
            (SALT, salt.clone()),
            (PUBLIC_KEY, server.b_pub_bytes()),
        ]);
        let m3 = Tlv8Map::parse(&client.handle_m2(&m2).unwrap()).unwrap();
        assert_eq!(m3.get(STATE), Some(&[3u8][..]));
        let a_public = m3.get(PUBLIC_KEY).unwrap().to_vec();
        let proof = m3.get(PROOF).unwrap().to_vec();

        // The server only produces M2 if our M1 proof verifies.
        let server_proof = server.verify_m1_prove_m2(&a_public, &proof).unwrap();
        let m4 = tlv(&[(STATE, vec![4]), (PROOF, server_proof)]);
        let session_key = client.finish(&m4).unwrap();

        assert_eq!(session_key, server.session_key(&a_public).unwrap());
        assert_eq!(session_key.len(), 64);
    }

    #[test]
    fn wrong_setup_code_is_rejected() {
        let (server, salt) = HapPairSetupSrpServer::new("1234").unwrap();
        let mut client = TransientPairing::new().unwrap();
        let m2 = tlv(&[
            (STATE, vec![2]),
            (SALT, salt),
            (PUBLIC_KEY, server.b_pub_bytes()),
        ]);
        let m3 = Tlv8Map::parse(&client.handle_m2(&m2).unwrap()).unwrap();
        let a_public = m3.get(PUBLIC_KEY).unwrap().to_vec();
        let proof = m3.get(PROOF).unwrap().to_vec();
        assert!(server.verify_m1_prove_m2(&a_public, &proof).is_err());
    }

    #[test]
    fn receiver_errors_and_bad_states_are_surfaced() {
        let mut client = TransientPairing::new().unwrap();
        let error = tlv(&[(STATE, vec![2]), (ERROR, vec![0x02])]);
        assert!(client.handle_m2(&error).is_err());

        let mut client = TransientPairing::new().unwrap();
        let wrong_state = tlv(&[(STATE, vec![4]), (SALT, vec![0; 16])]);
        assert!(client.handle_m2(&wrong_state).is_err());
    }
}
