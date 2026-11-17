//! Sealed all-pay sats auction (no pre-qday stake).
//!
//! Fields + Narula floated a dollar auction. This crate picks sealed all-pay:
//! every opened bid pays; highest unique bid wins the challenge pot. No
//! second-place spiral.

use bitcoin::hashes::Hash;
use bitcoin::{Amount, OutPoint};

use crate::challenge::{ChallengeUtxo, PathAdmission, Phase, SpendPath};
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

impl SatsBid {
    fn encode(&self) -> Vec<u8> {
        let mut data = Vec::with_capacity(32 + 4 + 8 + 32);
        data.extend_from_slice(self.id.txid.as_byte_array());
        data.extend_from_slice(&self.id.vout.to_le_bytes());
        data.extend_from_slice(&self.amount.to_sat().to_le_bytes());
        data.extend_from_slice(&self.payout_key.0);
        data
    }

    fn decode(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != 32 + 4 + 8 + 32 {
            return Err(Error::InvalidTimedOpen);
        }
        let mut txid_bytes = [0u8; 32];
        txid_bytes.copy_from_slice(&bytes[0..32]);
        let vout = u32::from_le_bytes(
            bytes[32..36]
                .try_into()
                .map_err(|_| Error::InvalidTimedOpen)?,
        );
        let amount = Amount::from_sat(u64::from_le_bytes(
            bytes[36..44]
                .try_into()
                .map_err(|_| Error::InvalidTimedOpen)?,
        ));
        let mut key = [0u8; 32];
        key.copy_from_slice(&bytes[44..76]);
        Ok(Self {
            id: OutPoint {
                txid: bitcoin::Txid::from_byte_array(txid_bytes),
                vout,
            },
            amount,
            payout_key: QuantumSafePubKey(key),
        })
    }
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

fn require_auction_phase(clock: &impl ActivationClock, auction: &SatsAuction) -> Result<()> {
    match auction
        .challenge
        .admit_path(clock, &SpendPath::AuctionBid)?
    {
        PathAdmission::PathAllowed { .. } => Ok(()),
        PathAdmission::TimeoutSpend { .. } => {
            Err(Error::Unimplemented("sats auction produced timeout spend"))
        }
    }
}

/// Seal and register a bid during the auction window. Returns registration index.
pub fn register_sats_bid(
    clock: &impl ActivationClock,
    timed: &impl TimedCommit,
    auction: &mut SatsAuction,
    bid: &SatsBid,
) -> Result<usize> {
    require_auction_phase(clock, auction)?;
    let pot = auction.challenge.total_locked()?;
    if bid.amount > pot {
        return Err(Error::BidExceedsPot {
            bid: bid.amount,
            pot,
        });
    }
    let force_open_at = auction.challenge.schedule.auction_end;
    let (commitment, sealed) = timed.seal(&bid.encode(), force_open_at);
    let index = auction.bids.len();
    auction.bids.push(RegisteredSatsBid {
        commitment,
        sealed,
        status: SatsBidStatus::Sealed,
    });
    Ok(index)
}

/// Self-reveal a registered bid during the auction window.
pub fn self_reveal_sats_bid(
    clock: &impl ActivationClock,
    timed: &impl TimedCommit,
    auction: &mut SatsAuction,
    index: usize,
    bid: &SatsBid,
) -> Result<()> {
    require_auction_phase(clock, auction)?;
    let entry = auction
        .bids
        .get_mut(index)
        .ok_or(Error::UnknownBidIndex { index })?;
    match &entry.status {
        SatsBidStatus::Opened(_) => return Err(Error::AlreadyOpened { index }),
        SatsBidStatus::Sealed => {}
    }
    let payload = bid.encode();
    if !timed.verify_open(&entry.commitment, &payload) {
        return Err(Error::InvalidTimedOpen);
    }
    if entry.sealed.payload != payload {
        return Err(Error::InvalidTimedOpen);
    }
    entry.status = SatsBidStatus::Opened(bid.clone());
    Ok(())
}

/// Force-open a sealed bid after `auction_end`.
pub fn force_open_sats_bid(
    clock: &impl ActivationClock,
    timed: &impl TimedCommit,
    auction: &mut SatsAuction,
    index: usize,
) -> Result<SatsBid> {
    let now = clock.now();
    let entry = auction
        .bids
        .get_mut(index)
        .ok_or(Error::UnknownBidIndex { index })?;
    match &entry.status {
        SatsBidStatus::Opened(bid) => return Ok(bid.clone()),
        SatsBidStatus::Sealed => {}
    }
    let payload = timed.force_open(&entry.sealed, now)?;
    if !timed.verify_open(&entry.commitment, &payload) {
        return Err(Error::InvalidTimedOpen);
    }
    let bid = SatsBid::decode(&payload)?;
    entry.status = SatsBidStatus::Opened(bid.clone());
    Ok(bid)
}

/// Settle after auction timeout (`Phase::Timeout`).
///
/// Every registered bid must be opened. All opened bids pay; unique highest
/// wins the full challenge pot.
pub fn settle_sats_auction(
    clock: &impl ActivationClock,
    auction: &SatsAuction,
) -> Result<SatsSettlement> {
    if auction.challenge.phase(clock) != Phase::Timeout {
        return Err(Error::WrongPhase {
            at: auction.challenge.phase(clock),
            required: Phase::Timeout,
        });
    }
    for (index, entry) in auction.bids.iter().enumerate() {
        if matches!(entry.status, SatsBidStatus::Sealed) {
            return Err(Error::BidStillSealed { index });
        }
    }
    let pot = auction.challenge.total_locked()?;
    let opened: Vec<&SatsBid> = auction
        .bids
        .iter()
        .filter_map(|e| match &e.status {
            SatsBidStatus::Opened(b) => Some(b),
            SatsBidStatus::Sealed => None,
        })
        .collect();
    if opened.is_empty() {
        return Ok(SatsSettlement {
            winner: None,
            all_paid: Amount::ZERO,
            bid_destination: auction.settlement.unused_collateral,
        });
    }
    let mut all_paid = Amount::ZERO;
    for bid in &opened {
        all_paid = all_paid
            .checked_add(bid.amount)
            .ok_or(Error::AmountOverflow)?;
    }
    let mut best: Option<&SatsBid> = None;
    let mut tie = false;
    for bid in &opened {
        match best {
            None => {
                best = Some(bid);
                tie = false;
            }
            Some(cur) if bid.amount > cur.amount => {
                best = Some(bid);
                tie = false;
            }
            Some(cur) if bid.amount == cur.amount => {
                tie = true;
            }
            Some(_) => {}
        }
    }
    if tie {
        return Err(Error::BidTie {
            amount: best.map(|b| b.amount).unwrap_or(Amount::ZERO),
        });
    }
    let winner = best.map(|b| (b.payout_key.clone(), pot));
    Ok(SatsSettlement {
        winner,
        all_paid,
        bid_destination: auction.settlement.unused_collateral,
    })
}
