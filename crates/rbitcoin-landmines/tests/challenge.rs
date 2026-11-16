use bitcoin::hashes::Hash;
use bitcoin::{Amount, OutPoint, Txid};
use rbitcoin_landmines::{
    ChallengeSchedule, ChallengeUtxo, CollateralPolicy, CreateChallenge, Error, PathAdmission,
    Phase, SettlementPolicy, SpendPath, StubClock,
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

fn schedule() -> ChallengeSchedule {
    ChallengeSchedule::new(10, 20).expect("schedule")
}

fn challenge(vulnerable: u64, collateral: u64) -> ChallengeUtxo {
    ChallengeUtxo::create(CreateChallenge {
        id: op(1),
        vulnerable_id: op(9),
        vulnerable_value: Amount::from_sat(vulnerable),
        collateral_value: Amount::from_sat(collateral),
        collateral_policy: CollateralPolicy::default(),
        schedule: schedule(),
        settlement: SettlementPolicy::default(),
    })
    .expect("create")
}

#[test]
fn phase_boundaries() {
    let p = schedule();
    assert_eq!(p.phase_at(0), Phase::Landmine);
    assert_eq!(p.phase_at(9), Phase::Landmine);
    assert_eq!(p.phase_at(10), Phase::Auction);
    assert_eq!(p.phase_at(19), Phase::Auction);
    assert_eq!(p.phase_at(20), Phase::Timeout);
}

#[test]
fn reject_under_collateralized() {
    let err = ChallengeUtxo::create(CreateChallenge {
        id: op(2),
        vulnerable_id: op(9),
        vulnerable_value: Amount::from_sat(100),
        collateral_value: Amount::from_sat(99),
        collateral_policy: CollateralPolicy::default(),
        schedule: schedule(),
        settlement: SettlementPolicy::default(),
    })
    .expect_err("under collateral");
    assert!(matches!(err, Error::UnderCollateralized { .. }));
}

#[test]
fn reject_invalid_phases() {
    let err = ChallengeSchedule::new(21, 20).expect_err("bad order");
    assert!(matches!(err, Error::InvalidPhases { .. }));
}

#[test]
fn wrong_phase_paths_fail() {
    let u = challenge(50, 50);
    assert!(matches!(
        u.admit_path(&clock(5), &SpendPath::AuctionBid),
        Err(Error::WrongPhase {
            at: Phase::Landmine,
            required: Phase::Auction
        })
    ));
    assert!(matches!(
        u.admit_path(&clock(5), &SpendPath::TimeoutSpend),
        Err(Error::WrongPhase {
            at: Phase::Landmine,
            required: Phase::Timeout
        })
    ));
    assert!(matches!(
        u.admit_path(&clock(15), &SpendPath::PokReclaim),
        Err(Error::WrongPhase {
            at: Phase::Auction,
            required: Phase::Landmine
        })
    ));
    assert!(matches!(
        u.admit_path(&clock(25), &SpendPath::AuctionBid),
        Err(Error::WrongPhase {
            at: Phase::Timeout,
            required: Phase::Auction
        })
    ));
}

#[test]
fn right_phase_paths_ok() {
    let u = challenge(50, 50);
    assert_eq!(
        u.admit_path(&clock(5), &SpendPath::PokReclaim)
            .expect("landmine"),
        PathAdmission::PathAllowed {
            phase: Phase::Landmine
        }
    );
    assert_eq!(
        u.admit_path(&clock(15), &SpendPath::AuctionBid)
            .expect("auction"),
        PathAdmission::PathAllowed {
            phase: Phase::Auction
        }
    );
    assert_eq!(
        u.admit_path(&clock(25), &SpendPath::TimeoutSpend)
            .expect("timeout"),
        PathAdmission::TimeoutSpend {
            total: Amount::from_sat(100)
        }
    );
}

#[test]
fn total_locked_conserved_on_timeout_spend() {
    let u = challenge(40, 60);
    assert_eq!(u.total_locked().expect("sum"), Amount::from_sat(100));
    match u
        .admit_path(&clock(20), &SpendPath::TimeoutSpend)
        .expect("timeout")
    {
        PathAdmission::TimeoutSpend { total } => assert_eq!(total, Amount::from_sat(100)),
        other => panic!("unexpected {other:?}"),
    }
}
