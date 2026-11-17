//! Typed errors for landmines validation.

use bitcoin::Amount;

use crate::challenge::Phase;

/// Landmines / challenge / spend-path failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Feature not wired yet.
    Unimplemented(&'static str),
    /// Offered collateral below policy.
    UnderCollateralized {
        /// Vulnerable value being spent.
        vulnerable: Amount,
        /// Collateral required by policy.
        required: Amount,
        /// Collateral offered.
        offered: Amount,
    },
    /// Sat arithmetic overflow.
    AmountOverflow,
    /// `landmine_end` must be <= `auction_end`.
    InvalidPhases {
        /// Landmine window end.
        landmine_end: u64,
        /// Auction window end.
        auction_end: u64,
    },
    /// Spend path not valid in this phase.
    WrongPhase {
        /// Phase at path time.
        at: Phase,
        /// Phase this path requires.
        required: Phase,
    },
    /// Pre-qday action attempted after q-day.
    PostQdayForbidden,
    /// PoK reveal does not match landmine commitment.
    InvalidPok,
    /// Reveal covers a different vulnerable outpoint than this challenge.
    VulnerableMismatch,
    /// Staked bid reveal does not match commitment.
    InvalidStakeCommit,
    /// Bid targets a different vulnerable outpoint than this challenge.
    TargetMismatch,
    /// Stake below bid amount.
    StakeTooSmall {
        /// Bid amount.
        bid: Amount,
        /// Stake locked.
        stake: Amount,
    },
    /// Bid exceeds remaining claimable collateral.
    BidTooHigh {
        /// Bid amount.
        bid: Amount,
        /// Collateral still available.
        remaining: Amount,
    },
    /// New bid is not strictly lower than current winner.
    BidNotLower {
        /// Attempted bid.
        bid: Amount,
        /// Current winning bid.
        current: Amount,
    },
    /// Too many successful reveals for configured depth.
    BidDepthExceeded {
        /// Configured max.
        limit: u32,
    },
    /// Highest bid leaves no non-negative winner payout from the pot.
    BidExceedsPot {
        /// Highest bid.
        bid: Amount,
        /// Challenge pot (vulnerable + collateral).
        pot: Amount,
    },
    /// Registered bid is still sealed; settle or abandon blocked.
    BidStillSealed {
        /// Registration index.
        index: usize,
    },
    /// Force-open attempted before `force_open_at`.
    ForceOpenTooEarly {
        /// Caller clock.
        now: u64,
        /// Earliest allowed force-open time.
        force_open_at: u64,
    },
    /// Bid at index was already opened.
    AlreadyOpened {
        /// Registration index.
        index: usize,
    },
    /// Unknown registration index.
    UnknownBidIndex {
        /// Registration index.
        index: usize,
    },
    /// Timed open payload does not match commitment or decode failed.
    InvalidTimedOpen,
    /// Two or more opened bids share the same highest amount.
    BidTie {
        /// Tied amount.
        amount: Amount,
    },
}

pub type Result<T> = core::result::Result<T, Error>;
