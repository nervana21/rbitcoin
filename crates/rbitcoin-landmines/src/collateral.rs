//! Collateral requirements for post-qday vulnerable spends.

use bitcoin::Amount;

use crate::error::{Error, Result};

/// Collateral ratio and checks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CollateralPolicy {
    /// Required collateral sats per vulnerable sat (default 1:1).
    pub collateral_per_vulnerable_sat: u64,
}

impl Default for CollateralPolicy {
    fn default() -> Self {
        Self {
            collateral_per_vulnerable_sat: 1,
        }
    }
}

impl CollateralPolicy {
    /// Required collateral for a vulnerable value.
    pub fn required(&self, vulnerable: Amount) -> Result<Amount> {
        vulnerable
            .checked_mul(self.collateral_per_vulnerable_sat)
            .ok_or(Error::AmountOverflow)
    }

    /// True if `offered` meets policy for `vulnerable`.
    pub fn is_fully_collateralized(&self, vulnerable: Amount, offered: Amount) -> Result<bool> {
        Ok(offered >= self.required(vulnerable)?)
    }
}
