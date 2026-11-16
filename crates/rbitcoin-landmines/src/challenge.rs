//! Shared challenge UTXO phase machine (feature-complete spine).

use bitcoin::{Amount, OutPoint};

use crate::clock::ActivationClock;
use crate::collateral::CollateralPolicy;
use crate::error::{Error, Result};
use crate::settlement::SettlementPolicy;

/// Challenge spend schedule after a collateralized vulnerable spend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Landmine / pre-qday PoK window.
    Landmine,
    /// Staked or sats auction window.
    Auction,
    /// Timeout spend after auction timeout.
    Timeout,
}

/// Timeout boundaries in clock units.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChallengeSchedule {
    /// End of landmine window (`now < landmine_end` => Landmine).
    pub landmine_end: u64,
    /// End of auction window (`now < auction_end` => Auction).
    pub auction_end: u64,
}

impl ChallengeSchedule {
    /// Validate ordering, then construct.
    pub fn new(landmine_end: u64, auction_end: u64) -> Result<Self> {
        // Stub: accept any order so InvalidPhases tests stay red.
        Ok(Self {
            landmine_end,
            auction_end,
        })
    }

    /// Phase at `now`.
    pub fn phase_at(&self, _now: u64) -> Phase {
        // Stub: wrong phase for every time.
        Phase::Timeout
    }
}

/// Inputs required to park a vulnerable spend in challenge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateChallenge {
    /// New challenge outpoint.
    pub id: OutPoint,
    /// Vulnerable outpoint captured into this challenge.
    pub vulnerable_id: OutPoint,
    /// Vulnerable value moved in.
    pub vulnerable_value: Amount,
    /// Quantum-safe collateral moved in.
    pub collateral_value: Amount,
    /// Collateral ratio policy.
    pub collateral_policy: CollateralPolicy,
    /// Phase timeouts.
    pub schedule: ChallengeSchedule,
    /// Settlement knobs for later modes.
    pub settlement: SettlementPolicy,
}

/// Collateralized vulnerable spend parked in challenge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChallengeUtxo {
    /// Id of this challenge output.
    pub id: OutPoint,
    /// Vulnerable outpoint captured into this challenge.
    pub vulnerable_id: OutPoint,
    /// Vulnerable vulnerable value captured into challenge.
    pub vulnerable_value: Amount,
    /// Attacker/owner collateral locked with it.
    pub collateral_value: Amount,
    /// Phase timeouts.
    pub schedule: ChallengeSchedule,
    /// Settlement knobs for later auction resolution.
    pub settlement: SettlementPolicy,
}

/// Spend path against a challenge UTXO.
///
/// Mode modules attach proofs later. Challenge only enforces phase membership and
/// value accounting for free spend.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpendPath {
    /// Allowed only in [`Phase::Landmine`].
    PokReclaim,
    /// Allowed only in [`Phase::Auction`].
    AuctionBid,
    /// Allowed only in [`Phase::Timeout`].
    TimeoutSpend,
}

/// Successful claim effect at the challenge layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathAdmission {
    /// Phase permits this claim class; landmine/auction settlement still TBD.
    PathAllowed {
        /// Phase that allowed the claim.
        phase: Phase,
    },
    /// Timeout-spend may move the full locked value out of challenge.
    TimeoutSpend {
        /// `vulnerable_value + collateral_value`.
        total: Amount,
    },
}

impl ChallengeUtxo {
    /// Create challenge after collateral + phase checks.
    pub fn create(input: CreateChallenge) -> Result<Self> {
        // Stub: skip collateral and phase checks.
        Ok(Self {
            id: input.id,
            vulnerable_id: input.vulnerable_id,
            vulnerable_value: input.vulnerable_value,
            collateral_value: input.collateral_value,
            schedule: input.schedule,
            settlement: input.settlement,
        })
    }

    /// Locked value: vulnerable + collateral.
    pub fn total_locked(&self) -> Result<Amount> {
        Err(Error::Unimplemented("challenge total_locked"))
    }

    /// Current phase from the activation clock.
    pub fn phase(&self, clock: &impl ActivationClock) -> Phase {
        self.schedule.phase_at(clock.now())
    }

    /// Required phase for a claim class.
    pub fn required_phase(claim: &SpendPath) -> Phase {
        match claim {
            SpendPath::PokReclaim => Phase::Landmine,
            SpendPath::AuctionBid => Phase::Auction,
            SpendPath::TimeoutSpend => Phase::Timeout,
        }
    }

    /// Accept or reject a claim by phase (and free-spend value lock).
    pub fn admit_path(&self, _clock: &impl ActivationClock, _claim: &SpendPath) -> Result<PathAdmission> {
        Err(Error::Unimplemented("challenge claim"))
    }
}
