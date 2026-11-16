//! Typed errors for landmines validation.

use bitcoin::Amount;

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
}

pub type Result<T> = core::result::Result<T, Error>;
