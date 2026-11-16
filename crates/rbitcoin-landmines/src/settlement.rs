//! Open settlement knobs from the mailing-list design.

/// Where unused or loser value goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ValueDestination {
    /// Destroy coins.
    #[default]
    Burn,
    /// Pay miners.
    Miners,
    /// Shared pot for later rules.
    Pot,
    /// Return to a designated party.
    ReturnToClaimant,
}

/// Losing staked bid handling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StakeOutcome {
    /// Stake returned after lose.
    #[default]
    ReturnStake,
    /// Stake slashed.
    Slash,
}

/// Bundle of unresolved design knobs. Defaults are explicit, not consensus law.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SettlementPolicy {
    /// Unused collateral / loser funds.
    pub unused_collateral: ValueDestination,
    /// Losing staked bid.
    pub losing_stake: StakeOutcome,
}
