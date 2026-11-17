//! Experimental quantum landmines and vulnerable coin auctions.
//!
//! Speculative post-qday rules. Not part of default Core-parity validation.
//! Soft-fork wire, real PQ crypto, and connect integration are stub seams.
//!
//! # Public surface allowlist
//!
//! Crate-root `pub use` names below are the intentional experimental library
//! API for this model. No other workspace crate imports them yet. Owner:
//! `docs/landmines.md`.

pub mod clock;
pub mod collateral;
pub mod crypto;
pub mod error;
pub mod hook;
pub mod landmine;
pub mod challenge;
pub mod output;
pub mod pok;
pub mod settlement;
pub mod staked;
pub mod timed_commit;

pub use bitcoin::{Amount, OutPoint};

pub use clock::{ActivationClock, StubClock};
pub use collateral::CollateralPolicy;
pub use crypto::{InsecureStubCrypto, QuantumSafeCrypto, QuantumSafePubKey, QuantumSafeSig};
pub use error::{Error, Result};
pub use hook::{ConsensusHook, NullConsensusHook};
pub use output::OutputKind;
pub use pok::{PokCommitment, PokReveal, PreQdayPok, StubPok};
pub use settlement::{SettlementPolicy, StakeOutcome, ValueDestination};
pub use challenge::{SpendPath, PathAdmission, CreateChallenge, ChallengeSchedule, ChallengeUtxo, Phase};

pub use landmine::{fund_landmine, reclaim_with_landmine, Landmine, LandmineReclaim};

pub use timed_commit::{SealedBlob, StubTimedCommit, TimedCommit, TimedCommitment};

pub use staked::{
    commit_staked_bid, force_open_staked_bid, open_staked_auction, register_staked_bid,
    self_reveal_staked_bid, settle_staked_auction, BidDepthLimit, RegisteredStakedBid, StakeCommit,
    StakedAuction, StakedBid, StakedBidCommitment, StakedBidReveal, StakedBidStatus,
    StakedSettlement, StakedWin, StubStakeCommit,
};
