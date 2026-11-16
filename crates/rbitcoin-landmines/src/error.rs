//! Typed errors for landmines validation.

use bitcoin::Amount;

use crate::challenge::Phase;

/// Landmines / challenge / claim failure.
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
        /// Phase at claim time.
        at: Phase,
        /// Phase this claim kind requires.
        required: Phase,
    },
}

pub type Result<T> = core::result::Result<T, Error>;
