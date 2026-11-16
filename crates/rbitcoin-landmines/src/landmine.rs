//! Quantum landmine fund + reclaim.

use bitcoin::{Amount, OutPoint};

use crate::challenge::{ChallengeUtxo, PathAdmission, Phase, SpendPath};
use crate::clock::ActivationClock;
use crate::crypto::QuantumSafePubKey;
use crate::error::{Error, Result};
use crate::output::OutputKind;
use crate::pok::{PokCommitment, PokReveal, PreQdayPok};

/// Trivial quantum-safe output carrying a hidden PoK commitment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Landmine {
    /// Mine outpoint.
    pub id: OutPoint,
    /// Dust / trivial value.
    pub value: Amount,
    /// Output classification (always quantum-safe for a landmine).
    pub kind: OutputKind,
    /// Hidden pre-qday commitment.
    pub commitment: PokCommitment,
}

/// Successful landmine reclaim: challenge value pays to the committed recovery key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LandmineReclaim {
    /// Key committed in the pre-qday reveal.
    pub payout_key: QuantumSafePubKey,
    /// Full challenge lock (vulnerable + collateral).
    pub total: Amount,
    /// Phase that allowed the path.
    pub phase: Phase,
}

/// Fund a landmine pre-qday from a reveal the owner keeps secret.
pub fn fund_landmine<P: PreQdayPok>(
    clock: &impl ActivationClock,
    pok: &P,
    id: OutPoint,
    value: Amount,
    reveal: &PokReveal,
) -> Result<Landmine> {
    if clock.is_post_qday() {
        return Err(Error::PostQdayForbidden);
    }
    Ok(Landmine {
        id,
        value,
        kind: OutputKind::QuantumSafe,
        commitment: pok.commit(reveal),
    })
}

/// Reclaim a challenge with a landmine reveal (Alice takes attacker collateral).
pub fn reclaim_with_landmine<P: PreQdayPok>(
    clock: &impl ActivationClock,
    pok: &P,
    mine: &Landmine,
    challenge: &ChallengeUtxo,
    reveal: &PokReveal,
) -> Result<LandmineReclaim> {
    if !pok.verify(&mine.commitment, reveal) {
        return Err(Error::InvalidPok);
    }
    if reveal.vulnerable != challenge.vulnerable_id {
        return Err(Error::VulnerableMismatch);
    }
    match challenge.admit_path(clock, &SpendPath::PokReclaim)? {
        PathAdmission::PathAllowed { phase } => Ok(LandmineReclaim {
            payout_key: reveal.recovery_key.clone(),
            total: challenge.total_locked()?,
            phase,
        }),
        PathAdmission::TimeoutSpend { .. } => {
            Err(Error::Unimplemented("landmine produced timeout spend"))
        }
    }
}
