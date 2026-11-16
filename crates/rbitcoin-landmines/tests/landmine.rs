use bitcoin::hashes::Hash;
use bitcoin::{Amount, OutPoint, Txid};
use rbitcoin_landmines::{
    fund_landmine, reclaim_with_landmine, ChallengeSchedule, ChallengeUtxo, CollateralPolicy,
    CreateChallenge, Error, OutputKind, Phase, PokReveal, QuantumSafePubKey, SettlementPolicy,
    StubClock, StubPok,
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

fn reveal(vulnerable: OutPoint, key_byte: u8, secret: u8) -> PokReveal {
    PokReveal {
        vulnerable,
        recovery_key: QuantumSafePubKey([key_byte; 32]),
        proof: vec![secret],
    }
}

fn attacker_challenge(vulnerable_id: OutPoint) -> ChallengeUtxo {
    ChallengeUtxo::create(CreateChallenge {
        id: op(7),
        vulnerable_id,
        vulnerable_value: Amount::from_sat(100_000_000),
        collateral_value: Amount::from_sat(100_000_000),
        collateral_policy: CollateralPolicy::default(),
        schedule: ChallengeSchedule::new(30, 60).expect("schedule"),
        settlement: SettlementPolicy::default(),
    })
    .expect("challenge")
}

#[test]
fn alice_reclaims_attacker_collateral() {
    let pok = StubPok;
    let vulnerable = op(9);
    let secret = reveal(vulnerable, 0xA1, 0x42);
    let mine = fund_landmine(
        &pre_qday(0),
        &pok,
        op(3),
        Amount::from_sat(546),
        &secret,
    )
    .expect("fund");
    assert_eq!(mine.kind, OutputKind::QuantumSafe);
    let challenge = attacker_challenge(vulnerable);

    let out =
        reclaim_with_landmine(&post_qday(10), &pok, &mine, &challenge, &secret).expect("reclaim");
    assert_eq!(out.phase, Phase::Landmine);
    assert_eq!(out.total, Amount::from_sat(200_000_000));
    assert_eq!(out.payout_key, secret.recovery_key);
}

#[test]
fn fund_rejected_post_qday() {
    let pok = StubPok;
    let vulnerable = op(9);
    let secret = reveal(vulnerable, 0xA1, 0x42);
    let err = fund_landmine(
        &post_qday(0),
        &pok,
        op(3),
        Amount::from_sat(546),
        &secret,
    )
    .expect_err("post-qday");
    assert_eq!(err, Error::PostQdayForbidden);
}

#[test]
fn attacker_without_pok_fails() {
    let pok = StubPok;
    let vulnerable = op(9);
    let alice = reveal(vulnerable, 0xA1, 0x42);
    let mine = fund_landmine(
        &pre_qday(0),
        &pok,
        op(3),
        Amount::from_sat(546),
        &alice,
    )
    .expect("fund");
    let challenge = attacker_challenge(vulnerable);
    let forged = reveal(vulnerable, 0xEE, 0x00);

    let err = reclaim_with_landmine(&post_qday(10), &pok, &mine, &challenge, &forged)
        .expect_err("forged");
    assert_eq!(err, Error::InvalidPok);
}

#[test]
fn wrong_vulnerable_binding_fails() {
    let pok = StubPok;
    let alice_vulnerable = op(9);
    let other_vulnerable = op(8);
    let alice = reveal(alice_vulnerable, 0xA1, 0x42);
    let mine = fund_landmine(
        &pre_qday(0),
        &pok,
        op(3),
        Amount::from_sat(546),
        &alice,
    )
    .expect("fund");
    let challenge = attacker_challenge(other_vulnerable);

    let err = reclaim_with_landmine(&post_qday(10), &pok, &mine, &challenge, &alice)
        .expect_err("mismatch");
    assert_eq!(err, Error::VulnerableMismatch);
}

#[test]
fn reclaim_rejected_after_landmine_window() {
    let pok = StubPok;
    let vulnerable = op(9);
    let alice = reveal(vulnerable, 0xA1, 0x42);
    let mine = fund_landmine(
        &pre_qday(0),
        &pok,
        op(3),
        Amount::from_sat(546),
        &alice,
    )
    .expect("fund");
    let challenge = attacker_challenge(vulnerable);

    let err = reclaim_with_landmine(&post_qday(30), &pok, &mine, &challenge, &alice)
        .expect_err("late");
    assert!(matches!(err, Error::WrongPhase { .. }));
}
