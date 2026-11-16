//! Pre-qday proof-of-knowledge commit/reveal.

use bitcoin::OutPoint;

use crate::crypto::QuantumSafePubKey;

/// Hidden commit embedded in a quantum-safe output (taproot-like leaf TBD).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PokCommitment(pub [u8; 32]);

/// Revealed pre-qday knowledge bound to a recovery key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PokReveal {
    /// Vulnerable outpoint this proof covers.
    pub vulnerable: OutPoint,
    /// Quantum-safe key the challenge must pay to on successful claim.
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

impl PreQdayPok for StubPok {
    fn commit(&self, _reveal: &PokReveal) -> PokCommitment {
        PokCommitment([0u8; 32])
    }

    fn verify(&self, _commitment: &PokCommitment, _reveal: &PokReveal) -> bool {
        false
    }
}
