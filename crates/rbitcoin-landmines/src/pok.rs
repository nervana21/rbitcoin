//! Pre-qday proof-of-knowledge commit/reveal.

use bitcoin::OutPoint;
use bitcoin_hashes::{sha256, Hash};

use crate::crypto::QuantumSafePubKey;

/// Hidden commit embedded in a quantum-safe output (taproot-like leaf TBD).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PokCommitment(pub [u8; 32]);

/// Revealed pre-qday knowledge bound to a recovery key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PokReveal {
    /// Vulnerable outpoint this proof covers.
    pub vulnerable: OutPoint,
    /// Quantum-safe key the challenge must pay to on successful reclaim.
    pub recovery_key: QuantumSafePubKey,
    /// Secret preimage (real scheme TBD; stub hashes this).
    pub proof: Vec<u8>,
}

/// Verify commit/reveal. Real Merkle/Taproot leaf later.
pub trait PreQdayPok {
    /// Build a commitment (pre-qday funding).
    fn commit(&self, reveal: &PokReveal) -> PokCommitment;

    /// Check reveal against a prior commitment.
    fn verify(&self, commitment: &PokCommitment, reveal: &PokReveal) -> bool;
}

/// Hash-commitment stub. Not a consensus PoK / taproot scheme.
#[derive(Debug, Default, Clone, Copy)]
pub struct StubPok;

impl StubPok {
    fn digest(reveal: &PokReveal) -> [u8; 32] {
        let mut data = Vec::with_capacity(36 + 32 + reveal.proof.len());
        data.extend_from_slice(reveal.vulnerable.txid.as_byte_array());
        data.extend_from_slice(&reveal.vulnerable.vout.to_le_bytes());
        data.extend_from_slice(&reveal.recovery_key.0);
        data.extend_from_slice(&reveal.proof);
        sha256::Hash::hash(&data).to_byte_array()
    }
}

impl PreQdayPok for StubPok {
    fn commit(&self, reveal: &PokReveal) -> PokCommitment {
        PokCommitment(Self::digest(reveal))
    }

    fn verify(&self, commitment: &PokCommitment, reveal: &PokReveal) -> bool {
        commitment.0 == Self::digest(reveal)
    }
}
