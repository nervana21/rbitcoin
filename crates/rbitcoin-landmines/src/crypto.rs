//! Quantum-safe crypto seam (stub).

/// Opaque quantum-safe public key bytes (scheme TBD).
///
/// Do not alias to classical `XOnlyPublicKey`. Width may change with the scheme.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct QuantumSafePubKey(pub [u8; 32]);

/// Opaque signature bytes (scheme TBD).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuantumSafeSig(pub Vec<u8>);

/// Plug-in for real PQ verify later.
///
/// Attach point: verify QS *spends* of challenge / auction payout keys once a
/// scheme exists. PoK and stake commit/reveal stay on `PreQdayPok` /
/// `StakeCommit` (hash leaves), not on this trait.
pub trait QuantumSafeCrypto {
    /// Verify a signature over `msg` for `pk`.
    fn verify(&self, pk: &QuantumSafePubKey, msg: &[u8], sig: &QuantumSafeSig) -> bool;
}

/// Insecure stub. Tests only. Never treat as production PQ.
#[derive(Debug, Default, Clone, Copy)]
pub struct InsecureStubCrypto;

impl QuantumSafeCrypto for InsecureStubCrypto {
    fn verify(&self, _pk: &QuantumSafePubKey, _msg: &[u8], _sig: &QuantumSafeSig) -> bool {
        false
    }
}
