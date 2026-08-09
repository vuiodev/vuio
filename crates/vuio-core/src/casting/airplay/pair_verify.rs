use anyhow::{Context, Result};
use hap_crypto::{
    aead::{chacha20poly1305_open, chacha20poly1305_seal},
    verify_ed25519, AccessoryPairing, ControllerKeypair, EphemeralKeypair, SessionKeys,
};
use hap_tlv8::{Tlv8Map, Tlv8Writer};
use hkdf::Hkdf;
use sha2::Sha512;

const IDENTIFIER: u8 = 0x01;
const PUBLIC_KEY: u8 = 0x03;
const ENCRYPTED_DATA: u8 = 0x05;
const STATE: u8 = 0x06;
const ERROR: u8 = 0x07;
const SIGNATURE: u8 = 0x0A;

pub struct PairVerifier {
    controller: ControllerKeypair,
    accessory: AccessoryPairing,
    ephemeral: EphemeralKeypair,
    shared: Option<[u8; 32]>,
}

pub struct VerifiedSession {
    pub keys: SessionKeys,
    pub shared_secret: [u8; 32],
}

impl PairVerifier {
    pub fn new(controller: ControllerKeypair, accessory: AccessoryPairing) -> Self {
        Self {
            controller,
            accessory,
            ephemeral: EphemeralKeypair::generate(),
            shared: None,
        }
    }

    pub fn start(&self) -> Vec<u8> {
        let mut body = Vec::new();
        let mut writer = Tlv8Writer::new(&mut body);
        writer.push_u8(STATE, 1);
        writer.push(PUBLIC_KEY, &self.ephemeral.public());
        body
    }

    pub fn handle_m2(&mut self, response: &[u8]) -> Result<Vec<u8>> {
        let map = checked_map(response, 2)?;
        let accessory_public: [u8; 32] = map
            .get(PUBLIC_KEY)
            .context("AirPlay Pair Verify M2 omitted the public key")?
            .try_into()
            .map_err(|_| anyhow::anyhow!("AirPlay Pair Verify public key has invalid length"))?;
        let encrypted = map
            .get(ENCRYPTED_DATA)
            .context("AirPlay Pair Verify M2 omitted encrypted data")?;
        let controller_public = self.ephemeral.public();
        let shared = self.ephemeral.diffie_hellman(&accessory_public);
        let verification_key = derive_key(
            &shared,
            b"Pair-Verify-Encrypt-Salt",
            b"Pair-Verify-Encrypt-Info",
        )?;
        let plaintext =
            chacha20poly1305_open(&verification_key, &hap_nonce(b"PV-Msg02"), b"", encrypted)?;
        let inner = Tlv8Map::parse(&plaintext)?;
        let identifier = inner
            .get(IDENTIFIER)
            .context("AirPlay Pair Verify M2 omitted the receiver identity")?;
        anyhow::ensure!(
            identifier == self.accessory.pairing_id.as_bytes(),
            "AirPlay receiver identity does not match the saved pairing"
        );
        let signature: [u8; 64] = inner
            .get(SIGNATURE)
            .context("AirPlay Pair Verify M2 omitted its signature")?
            .try_into()
            .map_err(|_| anyhow::anyhow!("AirPlay receiver signature has invalid length"))?;
        let mut signed = Vec::with_capacity(64 + identifier.len());
        signed.extend_from_slice(&accessory_public);
        signed.extend_from_slice(identifier);
        signed.extend_from_slice(&controller_public);
        verify_ed25519(&self.accessory.ltpk, &signed, &signature)
            .context("AirPlay receiver signature verification failed")?;

        let controller_id = self.controller.id.as_bytes();
        let mut controller_signed = Vec::with_capacity(64 + controller_id.len());
        controller_signed.extend_from_slice(&controller_public);
        controller_signed.extend_from_slice(controller_id);
        controller_signed.extend_from_slice(&accessory_public);
        let signature = self.controller.sign(&controller_signed);
        let mut inner_body = Vec::new();
        let mut inner_writer = Tlv8Writer::new(&mut inner_body);
        inner_writer.push(IDENTIFIER, controller_id);
        inner_writer.push(SIGNATURE, &signature);
        let encrypted =
            chacha20poly1305_seal(&verification_key, &hap_nonce(b"PV-Msg03"), b"", &inner_body)?;
        let mut body = Vec::new();
        let mut writer = Tlv8Writer::new(&mut body);
        writer.push_u8(STATE, 3);
        writer.push(ENCRYPTED_DATA, &encrypted);
        self.shared = Some(shared);
        Ok(body)
    }

    pub fn finish(self, response: &[u8]) -> Result<VerifiedSession> {
        checked_map(response, 4)?;
        let shared = self
            .shared
            .context("AirPlay Pair Verify finished before M2")?;
        Ok(VerifiedSession {
            keys: SessionKeys {
                read_key: derive_key(&shared, b"Control-Salt", b"Control-Read-Encryption-Key")?,
                write_key: derive_key(&shared, b"Control-Salt", b"Control-Write-Encryption-Key")?,
            },
            shared_secret: shared,
        })
    }
}

pub fn derive_key(shared: &[u8; 32], salt: &[u8], info: &[u8]) -> Result<[u8; 32]> {
    let mut output = [0u8; 32];
    Hkdf::<Sha512>::new(Some(salt), shared)
        .expand(info, &mut output)
        .map_err(|_| anyhow::anyhow!("AirPlay key derivation failed"))?;
    Ok(output)
}

fn checked_map(response: &[u8], expected_state: u8) -> Result<Tlv8Map> {
    let map = Tlv8Map::parse(response)?;
    anyhow::ensure!(
        map.get(ERROR).is_none_or(|value| value.is_empty()),
        "AirPlay receiver rejected Pair Verify"
    );
    anyhow::ensure!(
        map.get(STATE) == Some(&[expected_state][..]),
        "AirPlay Pair Verify response had an unexpected state"
    );
    Ok(map)
}

fn hap_nonce(label: &[u8; 8]) -> [u8; 12] {
    let mut nonce = [0u8; 12];
    nonce[4..].copy_from_slice(label);
    nonce
}
