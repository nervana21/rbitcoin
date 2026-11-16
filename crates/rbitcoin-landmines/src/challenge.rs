//! Shared challenge UTXO phase machine (feature-complete spine).
//!
//! Fields called this the "in-limbo" output. Here the type names the rule:
//! a bonded, timed, multi-path spend under contest.

use bitcoin::{Amount, OutPoint};

use crate::clock::ActivationClock;
use crate::collateral::CollateralPolicy;
use crate::error::{Error, Result};
use crate::settlement::SettlementPolicy;

/// Challenge spend phases after a collateralized vulnerable spend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    /// Landmine / pre-qday PoK reclaim window.
    Landmine,
    /// Staked or sats auction window.
    Auction,
    /// Unrestricted spend after auction timeout (no proof required).
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
        if landmine_end > auction_end {
            return Err(Error::InvalidPhases {
                landmine_end,
                auction_end,
            });
        }
        Ok(Self {
            landmine_end,
            auction_end,
        })
    }

    /// Phase at `now`.
    pub fn phase_at(&self, now: u64) -> Phase {
        if now < self.landmine_end {
            Phase::Landmine
        } else if now < self.auction_end {
            Phase::Auction
        } else {
            Phase::Timeout
        }
    }
}

/// Inputs required to open a challenge over a vulnerable spend.
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

/// Bonded vulnerable spend under timed challenge paths.
///
/// Glossary alias: Fields' "in-limbo" output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChallengeUtxo {
    /// Id of this challenge output.
    pub id: OutPoint,
    /// Vulnerable outpoint captured into this challenge.
    pub vulnerable_id: OutPoint,
    /// Quantum-vulnerable value locked in the challenge.
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
/// Mode modules attach proofs later. The challenge layer only enforces phase
/// membership and value accounting for the timeout path.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpendPath {
    /// Allowed only in [`Phase::Landmine`].
    PokReclaim,
    /// Allowed only in [`Phase::Auction`].
    AuctionBid,
    /// Allowed only in [`Phase::Timeout`].
    TimeoutSpend,
}

/// Successful path admission at the challenge layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathAdmission {
    /// Phase permits this path class; landmine/auction settlement still TBD.
    PathAllowed {
        /// Phase that allowed the path.
        phase: Phase,
    },
    /// Timeout spend may move the full locked value out of the challenge.
    TimeoutSpend {
        /// `vulnerable_value + collateral_value`.
        total: Amount,
    },
}

impl ChallengeUtxo {
    /// Create a challenge after collateral + schedule checks.
    pub fn create(input: CreateChallenge) -> Result<Self> {
        let required = input.collateral_policy.required(input.vulnerable_value)?;
        if input.collateral_value < required {
            return Err(Error::UnderCollateralized {
                vulnerable: input.vulnerable_value,
                required,
                offered: input.collateral_value,
            });
        }
        let _ = ChallengeSchedule::new(input.schedule.landmine_end, input.schedule.auction_end)?;
        let _ = input.total_locked()?;
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
        self.vulnerable_value
            .checked_add(self.collateral_value)
            .ok_or(Error::AmountOverflow)
    }

    /// Current phase from the activation clock.
    pub fn phase(&self, clock: &impl ActivationClock) -> Phase {
        self.schedule.phase_at(clock.now())
    }

    /// Required phase for a spend path.
    pub fn required_phase(path: &SpendPath) -> Phase {
        match path {
            SpendPath::PokReclaim => Phase::Landmine,
            SpendPath::AuctionBid => Phase::Auction,
            SpendPath::TimeoutSpend => Phase::Timeout,
        }
    }

    /// Admit or reject a spend path by phase (and timeout value lock).
    pub fn admit_path(
        &self,
        clock: &impl ActivationClock,
        path: &SpendPath,
    ) -> Result<PathAdmission> {
        let at = self.phase(clock);
        let required = Self::required_phase(path);
        if at != required {
            return Err(Error::WrongPhase { at, required });
        }
        match path {
            SpendPath::PokReclaim | SpendPath::AuctionBid => {
                Ok(PathAdmission::PathAllowed { phase: at })
            }
            SpendPath::TimeoutSpend => Ok(PathAdmission::TimeoutSpend {
                total: self.total_locked()?,
            }),
        }
    }
}

impl CreateChallenge {
    fn total_locked(&self) -> Result<Amount> {
        self.vulnerable_value
            .checked_add(self.collateral_value)
            .ok_or(Error::AmountOverflow)
    }
}
