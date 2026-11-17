//! Pre-qday staked auction bids with timed register / force-open.
//!
//! Pre-qday leaf commit stays. Auction registration seals the reveal so a
//! bidder cannot free-abandon: settle requires every registered bid opened
//! (self-reveal or force-open after auction end). Default loser stake = slash.

use bitcoin::hashes::Hash;
use bitcoin::{Amount, OutPoint};
use bitcoin_hashes::sha256;

use crate::challenge::{ChallengeUtxo, PathAdmission, Phase, SpendPath};
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

impl StakedBidReveal {
    fn encode(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(36 + 8 + 36 + 8 + 32 + 4 + self.proof.len());
        data.extend_from_slice(self.id.txid.as_byte_array());
        data.extend_from_slice(&self.id.vout.to_le_bytes());
        data.extend_from_slice(&self.stake.to_sat().to_le_bytes());
        data.extend_from_slice(self.target_vulnerable.txid.as_byte_array());
        data.extend_from_slice(&self.target_vulnerable.vout.to_le_bytes());
        data.extend_from_slice(&self.bid.to_sat().to_le_bytes());
        data.extend_from_slice(&self.payout_key.0);
        let proof_len = u32::try_from(self.proof.len()).unwrap_or(u32::MAX);
        data.extend_from_slice(&proof_len.to_le_bytes());
        data.extend_from_slice(&self.proof);
        data
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        const FIXED: usize = 32 + 4 + 8 + 32 + 4 + 8 + 32 + 4;
        if bytes.len() < FIXED {
            return Err(Error::InvalidTimedOpen);
        }
        let mut id_txid = [0u8; 32];
        id_txid.copy_from_slice(&bytes[0..32]);
        let id_vout = u32::from_le_bytes(
            bytes[32..36]
                .try_into()
                .map_err(|_| Error::InvalidTimedOpen)?,
        );
        let stake = Amount::from_sat(u64::from_le_bytes(
            bytes[36..44]
                .try_into()
                .map_err(|_| Error::InvalidTimedOpen)?,
        ));
        let mut target_txid = [0u8; 32];
        target_txid.copy_from_slice(&bytes[44..76]);
        let target_vout = u32::from_le_bytes(
            bytes[76..80]
                .try_into()
                .map_err(|_| Error::InvalidTimedOpen)?,
        );
        let bid = Amount::from_sat(u64::from_le_bytes(
            bytes[80..88]
                .try_into()
                .map_err(|_| Error::InvalidTimedOpen)?,
        ));
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes[88..120]);
        let proof_len = u32::from_le_bytes(
            bytes[120..124]
                .try_into()
                .map_err(|_| Error::InvalidTimedOpen)?,
        ) as usize;
        if bytes.len() != FIXED + proof_len {
            return Err(Error::InvalidTimedOpen);
        }
        Ok(Self {
            id: OutPoint {
                txid: bitcoin::Txid::from_byte_array(id_txid),
                vout: id_vout,
            },
            stake,
            target_vulnerable: OutPoint {
                txid: bitcoin::Txid::from_byte_array(target_txid),
                vout: target_vout,
            },
            bid,
            payout_key: QuantumSafePubKey(key),
            proof: bytes[124..].to_vec(),
        })
    }
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

fn require_auction_phase(clock: &impl ActivationClock, auction: &StakedAuction) -> Result<()> {
    match auction
        .challenge
        .admit_path(clock, &SpendPath::AuctionBid)?
    {
        PathAdmission::PathAllowed { .. } => Ok(()),
        PathAdmission::TimeoutSpend { .. } => {
            Err(Error::Unimplemented("auction produced timeout spend"))
        }
    }
}

fn check_reveal_against_committed(
    commit: &impl StakeCommit,
    committed: &StakedBid,
    reveal: &StakedBidReveal,
    auction: &StakedAuction,
) -> Result<()> {
    if !commit.verify(&committed.commitment, reveal) {
        return Err(Error::InvalidStakeCommit);
    }
    if reveal.id != committed.id || reveal.stake != committed.stake {
        return Err(Error::InvalidStakeCommit);
    }
    if reveal.target_vulnerable != auction.challenge.vulnerable_id {
        return Err(Error::TargetMismatch);
    }
    if reveal.stake < reveal.bid {
        return Err(Error::StakeTooSmall {
            bid: reveal.bid,
            stake: reveal.stake,
        });
    }
    Ok(())
}

/// Seal and register a pre-qday staked bid into the auction. Returns index.
pub fn register_staked_bid(
    clock: &impl ActivationClock,
    timed: &impl TimedCommit,
    commit: &impl StakeCommit,
    auction: &mut StakedAuction,
    committed: &StakedBid,
    reveal: &StakedBidReveal,
) -> Result<usize> {
    require_auction_phase(clock, auction)?;
    check_reveal_against_committed(commit, committed, reveal, auction)?;
    let force_open_at = auction.challenge.schedule.auction_end;
    let (timed_commitment, sealed) = timed.seal(&reveal.encode(), force_open_at);
    let index = auction.registered.len();
    auction.registered.push(RegisteredStakedBid {
        stake_commitment: committed.commitment.clone(),
        timed_commitment,
        sealed,
        stake: committed.stake,
        id: committed.id,
        status: StakedBidStatus::Sealed,
    });
    Ok(index)
}

fn claim_win(auction: &mut StakedAuction, reveal: &StakedBidReveal) -> Result<StakedWin> {
    if auction.reveals >= auction.depth.0 {
        return Err(Error::BidDepthExceeded {
            limit: auction.depth.0,
        });
    }
    if let Some(current) = &auction.winner {
        if reveal.bid >= current.reveal.bid {
            return Err(Error::BidNotLower {
                bid: reveal.bid,
                current: current.reveal.bid,
            });
        }
        auction.remaining_collateral = auction
            .remaining_collateral
            .checked_add(current.collateral_taken)
            .ok_or(Error::AmountOverflow)?;
    }
    if reveal.bid > auction.remaining_collateral {
        return Err(Error::BidTooHigh {
            bid: reveal.bid,
            remaining: auction.remaining_collateral,
        });
    }
    auction.remaining_collateral = auction
        .remaining_collateral
        .checked_sub(reveal.bid)
        .ok_or(Error::AmountOverflow)?;
    auction.vulnerable_burned = true;
    auction.reveals = auction
        .reveals
        .checked_add(1)
        .ok_or(Error::AmountOverflow)?;
    let win = StakedWin {
        reveal: reveal.clone(),
        collateral_taken: reveal.bid,
    };
    auction.winner = Some(win.clone());
    Ok(win)
}

/// Self-reveal a registered staked bid during the auction window.
pub fn self_reveal_staked_bid(
    clock: &impl ActivationClock,
    timed: &impl TimedCommit,
    auction: &mut StakedAuction,
    index: usize,
    reveal: &StakedBidReveal,
) -> Result<StakedWin> {
    require_auction_phase(clock, auction)?;
    let entry = auction
        .registered
        .get_mut(index)
        .ok_or(Error::UnknownBidIndex { index })?;
    match &entry.status {
        StakedBidStatus::Opened(_) => return Err(Error::AlreadyOpened { index }),
        StakedBidStatus::Sealed => {}
    }
    let payload = reveal.encode();
    if !timed.verify_open(&entry.timed_commitment, &payload) {
        return Err(Error::InvalidTimedOpen);
    }
    if entry.sealed.payload != payload {
        return Err(Error::InvalidTimedOpen);
    }
    if reveal.id != entry.id || reveal.stake != entry.stake {
        return Err(Error::InvalidStakeCommit);
    }
    entry.status = StakedBidStatus::Opened(reveal.clone());
    claim_win(auction, reveal)
}

/// Force-open a registered staked bid after `auction_end`, then claim if eligible.
pub fn force_open_staked_bid(
    clock: &impl ActivationClock,
    timed: &impl TimedCommit,
    auction: &mut StakedAuction,
    index: usize,
) -> Result<StakedWin> {
    let now = clock.now();
    let entry = auction
        .registered
        .get_mut(index)
        .ok_or(Error::UnknownBidIndex { index })?;
    let reveal = match &entry.status {
        StakedBidStatus::Opened(r) => r.clone(),
        StakedBidStatus::Sealed => {
            let payload = timed.force_open(&entry.sealed, now)?;
            if !timed.verify_open(&entry.timed_commitment, &payload) {
                return Err(Error::InvalidTimedOpen);
            }
            let reveal = StakedBidReveal::decode(&payload)?;
            if reveal.id != entry.id || reveal.stake != entry.stake {
                return Err(Error::InvalidStakeCommit);
            }
            entry.status = StakedBidStatus::Opened(reveal.clone());
            reveal
        }
    };
    if let Some(win) = &auction.winner {
        if win.reveal.id == reveal.id {
            return Ok(win.clone());
        }
    }
    claim_win(auction, &reveal)
}

/// Settle after auction timeout (`Phase::Timeout`).
pub fn settle_staked_auction(
    clock: &impl ActivationClock,
    auction: &StakedAuction,
) -> Result<StakedSettlement> {
    match auction
        .challenge
        .admit_path(clock, &SpendPath::TimeoutSpend)
    {
        Ok(_) | Err(Error::WrongPhase { .. }) => {}
        Err(e) => return Err(e),
    }
    if auction.challenge.phase(clock) != Phase::Timeout {
        return Err(Error::WrongPhase {
            at: auction.challenge.phase(clock),
            required: Phase::Timeout,
        });
    }
    for (index, entry) in auction.registered.iter().enumerate() {
        if matches!(entry.status, StakedBidStatus::Sealed) {
            return Err(Error::BidStillSealed { index });
        }
    }
    let winner_id = auction.winner.as_ref().map(|w| w.reveal.id);
    let mut slashed_stake = Amount::ZERO;
    if auction.settlement.losing_stake == StakeOutcome::Slash {
        for entry in &auction.registered {
            let StakedBidStatus::Opened(reveal) = &entry.status else {
                continue;
            };
            let is_winner = winner_id == Some(reveal.id);
            if !is_winner {
                slashed_stake = slashed_stake
                    .checked_add(reveal.stake)
                    .ok_or(Error::AmountOverflow)?;
            }
        }
    }
    let winner_payout = auction
        .winner
        .as_ref()
        .map(|w| (w.reveal.payout_key.clone(), w.collateral_taken));
    Ok(StakedSettlement {
        vulnerable_burned: auction.vulnerable_burned,
        winner_payout,
        unused_collateral: auction.remaining_collateral,
        unused_destination: auction.settlement.unused_collateral,
        slashed_stake,
        losing_stake: auction.settlement.losing_stake,
    })
}
