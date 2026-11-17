//! Sealed all-pay sats auction (no pre-qday stake).
//!
//! Fields + Narula floated a dollar auction. This crate picks sealed all-pay:
//! every opened bid pays; highest unique bid wins the challenge pot. No
//! second-place spiral.

use bitcoin::{Amount, OutPoint};

use crate::challenge::ChallengeUtxo;
use crate::clock::ActivationClock;
use crate::crypto::QuantumSafePubKey;
use crate::error::{Error, Result};
use crate::settlement::{SettlementPolicy, ValueDestination};
use crate::timed_commit::{SealedBlob, TimedCommit, TimedCommitment};

/// Opened sats bid (after self-reveal or force-open).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SatsBid {
    /// Bidder id (outpoint or ephemeral id).
    pub id: OutPoint,
    /// Bid amount (all opened bids pay this at settle).
    pub amount: Amount,
    /// Payout key if this bid wins.
    pub payout_key: QuantumSafePubKey,
}

/// Registration status for one sealed sats bid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SatsBidStatus {
    /// Still sealed.
    Sealed,
    /// Opened via self-reveal or force-open.
    Opened(SatsBid),
}

/// One registered sealed sats bid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredSatsBid {
    /// Timed commitment over the sealed payload.
    pub commitment: TimedCommitment,
    /// Sealed blob (force-open after auction end).
    pub sealed: SealedBlob,
    /// Open state.
    pub status: SatsBidStatus,
}

/// Live sealed all-pay auction over one challenge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SatsAuction {
    /// Challenge under auction.
    pub challenge: ChallengeUtxo,
    /// Registered bids in registration order.
    pub bids: Vec<RegisteredSatsBid>,
    /// Settlement knobs.
    pub settlement: SettlementPolicy,
}

/// Settlement after auction timeout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SatsSettlement {
    /// Winner key and full challenge pot, if any opened bid existed.
    pub winner: Option<(QuantumSafePubKey, Amount)>,
    /// Sum of all opened bid amounts (all-pay).
    pub all_paid: Amount,
    /// Where paid bid value goes.
    pub bid_destination: ValueDestination,
}

/// Open a sats auction once the landmine window has ended (caller enforces timing).
pub fn open_sats_auction(challenge: ChallengeUtxo) -> SatsAuction {
    let settlement = challenge.settlement;
    SatsAuction {
        challenge,
        bids: Vec::new(),
        settlement,
    }
}

/// Seal and register a bid during the auction window. Returns registration index.
pub fn register_sats_bid(
    _clock: &impl ActivationClock,
    _timed: &impl TimedCommit,
    _auction: &mut SatsAuction,
    _bid: &SatsBid,
) -> Result<usize> {
    Err(Error::Unimplemented("sats register"))
}

/// Self-reveal a registered bid during the auction window.
pub fn self_reveal_sats_bid(
    _clock: &impl ActivationClock,
    _timed: &impl TimedCommit,
    _auction: &mut SatsAuction,
    _index: usize,
    _bid: &SatsBid,
) -> Result<()> {
    Err(Error::Unimplemented("sats self_reveal"))
}

/// Force-open a sealed bid after `auction_end`.
pub fn force_open_sats_bid(
    _clock: &impl ActivationClock,
    _timed: &impl TimedCommit,
    _auction: &mut SatsAuction,
    _index: usize,
) -> Result<SatsBid> {
    Err(Error::Unimplemented("sats force_open"))
}

/// Settle after auction timeout (`Phase::Timeout`).
pub fn settle_sats_auction(
    _clock: &impl ActivationClock,
    _auction: &SatsAuction,
) -> Result<SatsSettlement> {
    Err(Error::Unimplemented("sats settle"))
}
