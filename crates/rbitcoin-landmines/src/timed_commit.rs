//! Timed commit/reveal seam for sealed auction bids.
//!
//! Stub stores plaintext in the sealed blob (same honesty class as
//! `InsecureStubCrypto`). The force-open clock gate is the modeled rule.

use bitcoin_hashes::{sha256, Hash};

use crate::error::{Error, Result};

/// Hash commitment over sealed payload bytes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TimedCommitment(pub [u8; 32]);

/// Opaque sealed payload. Stub = plaintext; real schemes hide until force time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SealedBlob {
    /// Earliest clock time `force_open` may run.
    pub force_open_at: u64,
    /// Payload bytes (plaintext under `StubTimedCommit`).
    pub payload: Vec<u8>,
}

/// Timed seal / self-open / force-open.
pub trait TimedCommit {
    /// Seal `payload` so it may be force-opened at `force_open_at`.
    fn seal(&self, payload: &[u8], force_open_at: u64) -> (TimedCommitment, SealedBlob);

    /// Check that `payload` matches a prior commitment.
    fn verify_open(&self, commitment: &TimedCommitment, payload: &[u8]) -> bool;

    /// Open after `force_open_at`. Fails earlier.
    fn force_open(&self, sealed: &SealedBlob, now: u64) -> Result<Vec<u8>>;
}

/// Insecure hash stub. Force gate is real; secrecy is not.
#[derive(Debug, Default, Clone, Copy)]
pub struct StubTimedCommit;

impl StubTimedCommit {
    fn digest(payload: &[u8]) -> [u8; 32] {
        sha256::Hash::hash(payload).to_byte_array()
    }
}

impl TimedCommit for StubTimedCommit {
    fn seal(&self, payload: &[u8], force_open_at: u64) -> (TimedCommitment, SealedBlob) {
        (
            TimedCommitment(Self::digest(payload)),
            SealedBlob {
                force_open_at,
                payload: payload.to_vec(),
            },
        )
    }

    fn verify_open(&self, commitment: &TimedCommitment, payload: &[u8]) -> bool {
        commitment.0 == Self::digest(payload)
    }

    fn force_open(&self, sealed: &SealedBlob, now: u64) -> Result<Vec<u8>> {
        if now < sealed.force_open_at {
            return Err(Error::ForceOpenTooEarly {
                now,
                force_open_at: sealed.force_open_at,
            });
        }
        Ok(sealed.payload.clone())
    }
}
