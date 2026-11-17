//! Pre-qday staked auction bids with timed register / force-open.
//!
//! Pre-qday leaf commit stays. Auction registration seals the reveal so a
//! bidder cannot free-abandon: settle requires every registered bid opened
//! (self-reveal or force-open after auction end). Default loser stake = slash.

use bitcoin::{Amount, OutPoint};
use bitcoin_hashes::{sha256, Hash};

use crate::challenge::ChallengeUtxo;
use crate::clock::ActivationClock;
use crate::crypto::QuantumSafePubKey;
use crate::error::{Error, Result};
use crate::output::OutputKind;
use crate::settlement::{SettlementPolicy, StakeOutcome, ValueDestination};
use crate::timed_commit::{SealedBlob, TimedCommit, TimedCommitment};

/// Max successful winning reveals against one challenge (open design knob).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BidDepthLimit(pub u32);

impl Default for BidDepthLimit {
    fn default() -> Self {
        Self(8)
    }
}

/// Commitment embedded in a quantum-safe stake output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StakedBidCommitment(pub [u8; 32]);

/// Secret preimage + fields committed pre-qday.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StakedBidReveal {
    /// Stake outpoint.
    pub id: OutPoint,
    /// Stake locked (must be >= bid).
    pub stake: Amount,
    /// Vulnerable outpoint this bid targets.
    pub target_vulnerable: OutPoint,
    /// Bid amount (claims this much collateral on win).
    pub bid: Amount,
    /// Payout key.
    pub payout_key: QuantumSafePubKey,
    /// Secret preimage (real leaf scheme TBD).
    pub proof: Vec<u8>,
}

/// Committed staked bid waiting for auction registration.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StakedBid {
    /// Commitment published pre-qday.
    pub commitment: StakedBidCommitment,
    /// Stake outpoint.
    pub id: OutPoint,
    /// Stake locked.
    pub stake: Amount,
    /// Output classification.
    pub kind: OutputKind,
}

/// Commit/reveal for staked bids (pre-qday leaf).
pub trait StakeCommit {
    /// Build a commitment (pre-qday funding).
    fn commit(&self, reveal: &StakedBidReveal) -> StakedBidCommitment;

    /// Check reveal against a prior commitment.
    fn verify(&self, commitment: &StakedBidCommitment, reveal: &StakedBidReveal) -> bool;
}

/// Hash-commitment stub. Not a consensus leaf scheme.
#[derive(Debug, Default, Clone, Copy)]
pub struct StubStakeCommit;

impl StubStakeCommit {
    fn digest(reveal: &StakedBidReveal) -> [u8; 32] {
        let mut data = Vec::with_capacity(36 + 8 + 36 + 8 + 32 + reveal.proof.len());
        data.extend_from_slice(reveal.id.txid.as_byte_array());
        data.extend_from_slice(&reveal.id.vout.to_le_bytes());
        data.extend_from_slice(&reveal.stake.to_sat().to_le_bytes());
        data.extend_from_slice(reveal.target_vulnerable.txid.as_byte_array());
        data.extend_from_slice(&reveal.target_vulnerable.vout.to_le_bytes());
        data.extend_from_slice(&reveal.bid.to_sat().to_le_bytes());
        data.extend_from_slice(&reveal.payout_key.0);
        data.extend_from_slice(&reveal.proof);
        sha256::Hash::hash(&data).to_byte_array()
    }
}

impl StakeCommit for StubStakeCommit {
    fn commit(&self, reveal: &StakedBidReveal) -> StakedBidCommitment {
        StakedBidCommitment(Self::digest(reveal))
    }

    fn verify(&self, commitment: &StakedBidCommitment, reveal: &StakedBidReveal) -> bool {
        commitment.0 == Self::digest(reveal)
    }
}

/// Open state for a registered staked bid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StakedBidStatus {
    /// Sealed; must self-reveal or force-open before settle.
    Sealed,
    /// Opened reveal.
    Opened(StakedBidReveal),
}

/// One auction-registered sealed staked bid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegisteredStakedBid {
    /// Pre-qday stake commitment.
    pub stake_commitment: StakedBidCommitment,
    /// Timed commitment over the sealed reveal payload.
    pub timed_commitment: TimedCommitment,
    /// Sealed reveal blob.
    pub sealed: SealedBlob,
    /// Stake locked (denormalized from pre-qday bid).
    pub stake: Amount,
    /// Stake outpoint.
    pub id: OutPoint,
    /// Open state.
    pub status: StakedBidStatus,
}

/// Current auction winner (lowest bid so far).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StakedWin {
    /// Winning reveal.
    pub reveal: StakedBidReveal,
    /// Collateral slice awarded while winning.
    pub collateral_taken: Amount,
}

/// Live staked auction over one challenge.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StakedAuction {
    /// Challenge under auction.
    pub challenge: ChallengeUtxo,
    /// Remaining collateral available to the current winner.
    pub remaining_collateral: Amount,
    /// True once any successful reveal burns the vulnerable value.
    pub vulnerable_burned: bool,
    /// Current lowest bidder, if any.
    pub winner: Option<StakedWin>,
    /// Successful winning reveal count.
    pub reveals: u32,
    /// Depth limit.
    pub depth: BidDepthLimit,
    /// Settlement knobs.
    pub settlement: SettlementPolicy,
    /// Registered sealed bids (abandon impossible once listed).
    pub registered: Vec<RegisteredStakedBid>,
}

/// Final settlement after auction timeout.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StakedSettlement {
    /// Vulnerable value destroyed (true if any bid landed).
    pub vulnerable_burned: bool,
    /// Winner payout from collateral, if any.
    pub winner_payout: Option<(QuantumSafePubKey, Amount)>,
    /// Leftover challenge collateral amount.
    pub unused_collateral: Amount,
    /// Where leftover / slash value goes.
    pub unused_destination: ValueDestination,
    /// Sum of non-winner stakes when policy is `Slash`.
    pub slashed_stake: Amount,
    /// Applied losing-stake policy.
    pub losing_stake: StakeOutcome,
}

/// Fund a staked bid pre-qday (leaf commit only).
pub fn commit_staked_bid<C: StakeCommit>(
    clock: &impl ActivationClock,
    commit: &C,
    reveal: &StakedBidReveal,
) -> Result<StakedBid> {
    if clock.is_post_qday() {
        return Err(Error::PostQdayForbidden);
    }
    if reveal.stake < reveal.bid {
        return Err(Error::StakeTooSmall {
            bid: reveal.bid,
            stake: reveal.stake,
        });
    }
    Ok(StakedBid {
        commitment: commit.commit(reveal),
        id: reveal.id,
        stake: reveal.stake,
        kind: OutputKind::QuantumSafe,
    })
}

/// Open a staked auction on a challenge (call once landmine window ends).
pub fn open_staked_auction(challenge: ChallengeUtxo, depth: BidDepthLimit) -> StakedAuction {
    let remaining_collateral = challenge.collateral_value;
    let settlement = challenge.settlement;
    StakedAuction {
        challenge,
        remaining_collateral,
        vulnerable_burned: false,
        winner: None,
        reveals: 0,
        depth,
        settlement,
        registered: Vec::new(),
    }
}

/// Seal and register a pre-qday staked bid into the auction. Returns index.
pub fn register_staked_bid(
    _clock: &impl ActivationClock,
    _timed: &impl TimedCommit,
    _commit: &impl StakeCommit,
    _auction: &mut StakedAuction,
    _committed: &StakedBid,
    _reveal: &StakedBidReveal,
) -> Result<usize> {
    Err(Error::Unimplemented("staked register"))
}

/// Self-reveal a registered staked bid during the auction window.
pub fn self_reveal_staked_bid(
    _clock: &impl ActivationClock,
    _timed: &impl TimedCommit,
    _auction: &mut StakedAuction,
    _index: usize,
    _reveal: &StakedBidReveal,
) -> Result<StakedWin> {
    Err(Error::Unimplemented("staked self_reveal"))
}

/// Force-open a registered staked bid after `auction_end`, then claim if eligible.
pub fn force_open_staked_bid(
    _clock: &impl ActivationClock,
    _timed: &impl TimedCommit,
    _auction: &mut StakedAuction,
    _index: usize,
) -> Result<StakedWin> {
    Err(Error::Unimplemented("staked force_open"))
}

/// Settle after auction timeout (`Phase::Timeout`).
pub fn settle_staked_auction(
    _clock: &impl ActivationClock,
    _auction: &StakedAuction,
) -> Result<StakedSettlement> {
    Err(Error::Unimplemented("staked settle"))
}
