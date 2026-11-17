use bitcoin::hashes::Hash;
use bitcoin::{Amount, OutPoint, Txid};
use rbitcoin_landmines::{
    force_open_sats_bid, open_sats_auction, register_sats_bid, self_reveal_sats_bid,
    settle_sats_auction, ChallengeSchedule, ChallengeUtxo, CollateralPolicy, CreateChallenge,
    Error, QuantumSafePubKey, SatsBid, SettlementPolicy, StubClock, StubTimedCommit,
    ValueDestination,
};

fn op(byte: u8) -> OutPoint {
    OutPoint {
        txid: Txid::from_byte_array([byte; 32]),
        vout: 0,
    }
}

fn clock(now: u64) -> StubClock {
    StubClock {
        post_qday: true,
        now,
    }
}

fn challenge() -> ChallengeUtxo {
    ChallengeUtxo::create(CreateChallenge {
        id: op(7),
        vulnerable_id: op(9),
        vulnerable_value: Amount::from_sat(50),
        collateral_value: Amount::from_sat(50),
        collateral_policy: CollateralPolicy::default(),
        schedule: ChallengeSchedule::new(10, 20).expect("schedule"),
        settlement: SettlementPolicy::default(),
    })
    .expect("challenge")
}

fn bid(id: u8, amount: u64, key: u8) -> SatsBid {
    SatsBid {
        id: op(id),
        amount: Amount::from_sat(amount),
        payout_key: QuantumSafePubKey([key; 32]),
    }
}

#[test]
fn sealed_all_pay_highest_wins_pot_all_pay() {
    let timed = StubTimedCommit;
    let mut auction = open_sats_auction(challenge());
    let a = bid(1, 10, 0xA1);
    let b = bid(2, 20, 0xB2);
    let c = bid(3, 30, 0xC3);
    let i0 = register_sats_bid(&clock(15), &timed, &mut auction, &a).expect("reg a");
    let i1 = register_sats_bid(&clock(15), &timed, &mut auction, &b).expect("reg b");
    let i2 = register_sats_bid(&clock(15), &timed, &mut auction, &c).expect("reg c");
    self_reveal_sats_bid(&clock(16), &timed, &mut auction, i0, &a).expect("open a");
    self_reveal_sats_bid(&clock(16), &timed, &mut auction, i1, &b).expect("open b");
    self_reveal_sats_bid(&clock(16), &timed, &mut auction, i2, &c).expect("open c");

    let settled = settle_sats_auction(&clock(20), &auction).expect("settle");
    assert_eq!(
        settled.winner,
        Some((QuantumSafePubKey([0xC3; 32]), Amount::from_sat(100)))
    );
    assert_eq!(settled.all_paid, Amount::from_sat(60));
    assert_eq!(settled.bid_destination, ValueDestination::Burn);
}

#[test]
fn tied_highest_rejected_at_settle() {
    let timed = StubTimedCommit;
    let mut auction = open_sats_auction(challenge());
    let a = bid(1, 25, 0xA1);
    let b = bid(2, 25, 0xB2);
    let i0 = register_sats_bid(&clock(15), &timed, &mut auction, &a).expect("a");
    let i1 = register_sats_bid(&clock(15), &timed, &mut auction, &b).expect("b");
    self_reveal_sats_bid(&clock(16), &timed, &mut auction, i0, &a).expect("open a");
    self_reveal_sats_bid(&clock(16), &timed, &mut auction, i1, &b).expect("open b");
    let err = settle_sats_auction(&clock(20), &auction).expect_err("tie");
    assert!(matches!(err, Error::BidTie { .. }));
}

#[test]
fn sealed_bid_blocks_settle_until_force_open() {
    let timed = StubTimedCommit;
    let mut auction = open_sats_auction(challenge());
    let a = bid(1, 10, 0xA1);
    let i0 = register_sats_bid(&clock(15), &timed, &mut auction, &a).expect("reg");
    let err = settle_sats_auction(&clock(20), &auction).expect_err("sealed");
    assert_eq!(err, Error::BidStillSealed { index: 0 });

    let early = force_open_sats_bid(&clock(19), &timed, &mut auction, i0).expect_err("early");
    assert!(matches!(early, Error::ForceOpenTooEarly { .. }));

    force_open_sats_bid(&clock(20), &timed, &mut auction, i0).expect("force");
    let settled = settle_sats_auction(&clock(20), &auction).expect("settle");
    assert_eq!(
        settled.winner,
        Some((QuantumSafePubKey([0xA1; 32]), Amount::from_sat(100)))
    );
    assert_eq!(settled.all_paid, Amount::from_sat(10));
}

#[test]
fn register_before_auction_window_fails() {
    let timed = StubTimedCommit;
    let mut auction = open_sats_auction(challenge());
    let err = register_sats_bid(&clock(5), &timed, &mut auction, &bid(1, 10, 0xA1))
        .expect_err("early");
    assert!(matches!(err, Error::WrongPhase { .. }));
}

#[test]
fn settle_before_timeout_fails() {
    let timed = StubTimedCommit;
    let mut auction = open_sats_auction(challenge());
    let a = bid(1, 10, 0xA1);
    let i0 = register_sats_bid(&clock(15), &timed, &mut auction, &a).expect("reg");
    self_reveal_sats_bid(&clock(15), &timed, &mut auction, i0, &a).expect("open");
    let err = settle_sats_auction(&clock(15), &auction).expect_err("early settle");
    assert!(matches!(err, Error::WrongPhase { .. }));
}

#[test]
fn double_reveal_rejected() {
    let timed = StubTimedCommit;
    let mut auction = open_sats_auction(challenge());
    let a = bid(1, 10, 0xA1);
    let i0 = register_sats_bid(&clock(15), &timed, &mut auction, &a).expect("reg");
    self_reveal_sats_bid(&clock(15), &timed, &mut auction, i0, &a).expect("open");
    let err = self_reveal_sats_bid(&clock(16), &timed, &mut auction, i0, &a).expect_err("again");
    assert_eq!(err, Error::AlreadyOpened { index: 0 });
}
