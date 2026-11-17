use bitcoin::hashes::Hash;
use bitcoin::{Amount, OutPoint, Txid};
use rbitcoin_landmines::{
    commit_staked_bid, force_open_staked_bid, open_staked_auction, register_staked_bid,
    self_reveal_staked_bid, settle_staked_auction, BidDepthLimit, ChallengeSchedule, ChallengeUtxo,
    CollateralPolicy, CreateChallenge, Error, OutputKind, QuantumSafePubKey, SettlementPolicy,
    StakeOutcome, StakedBidReveal, StubClock, StubStakeCommit, StubTimedCommit, ValueDestination,
};

fn op(byte: u8) -> OutPoint {
    OutPoint {
        txid: Txid::from_byte_array([byte; 32]),
        vout: 0,
    }
}

fn pre_qday(now: u64) -> StubClock {
    StubClock {
        post_qday: false,
        now,
    }
}

fn post_qday(now: u64) -> StubClock {
    StubClock {
        post_qday: true,
        now,
    }
}

fn challenge(vulnerable: OutPoint) -> ChallengeUtxo {
    ChallengeUtxo::create(CreateChallenge {
        id: op(7),
        vulnerable_id: vulnerable,
        vulnerable_value: Amount::from_sat(50),
        collateral_value: Amount::from_sat(50),
        collateral_policy: CollateralPolicy::default(),
        schedule: ChallengeSchedule::new(10, 20).expect("schedule"),
        settlement: SettlementPolicy::default(),
    })
    .expect("challenge")
}

fn reveal(vulnerable: OutPoint, id_byte: u8, bid: u64, key: u8) -> StakedBidReveal {
    StakedBidReveal {
        id: op(id_byte),
        stake: Amount::from_sat(50),
        target_vulnerable: vulnerable,
        bid: Amount::from_sat(bid),
        payout_key: QuantumSafePubKey([key; 32]),
        proof: vec![id_byte, key],
    }
}

#[test]
fn default_settlement_slashes_losing_stake() {
    assert_eq!(SettlementPolicy::default().losing_stake, StakeOutcome::Slash);
}

#[test]
fn bob_then_carol_lower_bid_wins_and_slashes_bob() {
    let vulnerable = op(9);
    let commit = StubStakeCommit;
    let timed = StubTimedCommit;
    let mut auction = open_staked_auction(challenge(vulnerable), BidDepthLimit::default());

    let bob_r = reveal(vulnerable, 1, 40, 0xB0);
    let bob = commit_staked_bid(&pre_qday(0), &commit, &bob_r).expect("bob commit");
    assert_eq!(bob.kind, OutputKind::QuantumSafe);
    let bob_i = register_staked_bid(
        &post_qday(15),
        &timed,
        &commit,
        &mut auction,
        &bob,
        &bob_r,
    )
    .expect("bob reg");
    let bob_win =
        self_reveal_staked_bid(&post_qday(15), &timed, &mut auction, bob_i, &bob_r).expect("bob");
    assert_eq!(bob_win.collateral_taken, Amount::from_sat(40));
    assert_eq!(auction.remaining_collateral, Amount::from_sat(10));
    assert!(auction.vulnerable_burned);

    let carol_r = reveal(vulnerable, 2, 30, 0xC0);
    let carol = commit_staked_bid(&pre_qday(0), &commit, &carol_r).expect("carol commit");
    let carol_i = register_staked_bid(
        &post_qday(16),
        &timed,
        &commit,
        &mut auction,
        &carol,
        &carol_r,
    )
    .expect("carol reg");
    let carol_win = self_reveal_staked_bid(&post_qday(16), &timed, &mut auction, carol_i, &carol_r)
        .expect("carol");
    assert_eq!(carol_win.collateral_taken, Amount::from_sat(30));
    assert_eq!(auction.remaining_collateral, Amount::from_sat(20));

    let settled = settle_staked_auction(&post_qday(20), &auction).expect("settle");
    assert!(settled.vulnerable_burned);
    assert_eq!(
        settled.winner_payout,
        Some((QuantumSafePubKey([0xC0; 32]), Amount::from_sat(30)))
    );
    assert_eq!(settled.unused_collateral, Amount::from_sat(20));
    assert_eq!(settled.unused_destination, ValueDestination::Burn);
    assert_eq!(settled.losing_stake, StakeOutcome::Slash);
    assert_eq!(settled.slashed_stake, Amount::from_sat(50));
}

#[test]
fn force_open_after_deadline_then_settle() {
    let vulnerable = op(9);
    let commit = StubStakeCommit;
    let timed = StubTimedCommit;
    let mut auction = open_staked_auction(challenge(vulnerable), BidDepthLimit::default());
    let bob_r = reveal(vulnerable, 1, 40, 0xB0);
    let bob = commit_staked_bid(&pre_qday(0), &commit, &bob_r).expect("bob");
    let bob_i = register_staked_bid(
        &post_qday(15),
        &timed,
        &commit,
        &mut auction,
        &bob,
        &bob_r,
    )
    .expect("reg");

    let sealed = settle_staked_auction(&post_qday(20), &auction).expect_err("sealed");
    assert_eq!(sealed, Error::BidStillSealed { index: 0 });

    let early = force_open_staked_bid(&post_qday(19), &timed, &mut auction, bob_i).expect_err("early");
    assert!(matches!(early, Error::ForceOpenTooEarly { .. }));

    force_open_staked_bid(&post_qday(20), &timed, &mut auction, bob_i).expect("force");
    let settled = settle_staked_auction(&post_qday(20), &auction).expect("settle");
    assert_eq!(
        settled.winner_payout,
        Some((QuantumSafePubKey([0xB0; 32]), Amount::from_sat(40)))
    );
    assert_eq!(settled.slashed_stake, Amount::ZERO);
}

#[test]
fn equal_or_higher_bid_rejected() {
    let vulnerable = op(9);
    let commit = StubStakeCommit;
    let timed = StubTimedCommit;
    let mut auction = open_staked_auction(challenge(vulnerable), BidDepthLimit::default());
    let bob_r = reveal(vulnerable, 1, 40, 0xB0);
    let bob = commit_staked_bid(&pre_qday(0), &commit, &bob_r).expect("bob");
    let bob_i = register_staked_bid(
        &post_qday(15),
        &timed,
        &commit,
        &mut auction,
        &bob,
        &bob_r,
    )
    .expect("bob reg");
    self_reveal_staked_bid(&post_qday(15), &timed, &mut auction, bob_i, &bob_r).expect("bob win");

    let copy_r = reveal(vulnerable, 2, 40, 0xCC);
    let copy = commit_staked_bid(&pre_qday(0), &commit, &copy_r).expect("copy");
    let copy_i = register_staked_bid(
        &post_qday(16),
        &timed,
        &commit,
        &mut auction,
        &copy,
        &copy_r,
    )
    .expect("copy reg");
    let err = self_reveal_staked_bid(&post_qday(16), &timed, &mut auction, copy_i, &copy_r)
        .expect_err("not lower");
    assert!(matches!(err, Error::BidNotLower { .. }));
}

#[test]
fn forged_reveal_rejected_at_register() {
    let vulnerable = op(9);
    let commit = StubStakeCommit;
    let timed = StubTimedCommit;
    let mut auction = open_staked_auction(challenge(vulnerable), BidDepthLimit::default());
    let bob_r = reveal(vulnerable, 1, 40, 0xB0);
    let bob = commit_staked_bid(&pre_qday(0), &commit, &bob_r).expect("bob");
    let mut forged = bob_r.clone();
    forged.bid = Amount::from_sat(10);
    let err = register_staked_bid(
        &post_qday(15),
        &timed,
        &commit,
        &mut auction,
        &bob,
        &forged,
    )
    .expect_err("forge");
    assert_eq!(err, Error::InvalidStakeCommit);
}

#[test]
fn register_before_auction_window_fails() {
    let vulnerable = op(9);
    let commit = StubStakeCommit;
    let timed = StubTimedCommit;
    let mut auction = open_staked_auction(challenge(vulnerable), BidDepthLimit::default());
    let bob_r = reveal(vulnerable, 1, 40, 0xB0);
    let bob = commit_staked_bid(&pre_qday(0), &commit, &bob_r).expect("bob");
    let err = register_staked_bid(&post_qday(5), &timed, &commit, &mut auction, &bob, &bob_r)
        .expect_err("early");
    assert!(matches!(err, Error::WrongPhase { .. }));
}

#[test]
fn commit_rejected_post_qday() {
    let vulnerable = op(9);
    let commit = StubStakeCommit;
    let bob_r = reveal(vulnerable, 1, 40, 0xB0);
    let err = commit_staked_bid(&post_qday(0), &commit, &bob_r).expect_err("post-qday");
    assert_eq!(err, Error::PostQdayForbidden);
}
