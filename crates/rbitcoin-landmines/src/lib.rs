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
pub mod challenge;
pub mod output;
pub mod settlement;

pub use bitcoin::{Amount, OutPoint};

pub use clock::{ActivationClock, StubClock};
pub use collateral::CollateralPolicy;
pub use crypto::{InsecureStubCrypto, QuantumSafeCrypto, QuantumSafePubKey, QuantumSafeSig};
pub use error::{Error, Result};
pub use hook::{ConsensusHook, NullConsensusHook};
pub use output::OutputKind;
pub use settlement::{SettlementPolicy, StakeOutcome, ValueDestination};
pub use challenge::{SpendPath, PathAdmission, CreateChallenge, ChallengeSchedule, ChallengeUtxo, Phase};
