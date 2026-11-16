use rbitcoin_store::spend_delta::{SpendPageLog, SPEND_DELTA_COMMIT_NAME};
use std::path::PathBuf;

fn dir() -> PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static N: AtomicU64 = AtomicU64::new(0);
    let n = N.fetch_add(1, Ordering::Relaxed);
    let p = std::env::temp_dir().join(format!("rbitcoin-spend-delta-{}-{n}", std::process::id()));
    let _ = std::fs::remove_dir_all(&p);
    std::fs::create_dir_all(&p).unwrap();
    p
}

fn commit(
    log: &SpendPageLog,
    height: u32,
    published: u64,
    keep: impl FnMut(&[u8; 8]) -> Result<bool, rbitcoin_store::error::StoreError>,
) {
    log.freeze_open();
    log.prepare_commit_in(
        height,
        published,
        |_, _| Ok(Vec::new()),
        keep,
        |_, _| Ok(()),
        |_| Ok(height),
    )
    .unwrap();
}

#[test]
fn noted_slot_survives_commit_and_reload() {
    let d = dir();
    let pub_len = 4096u64;
    let meta = [9u8, 1, 2, 3, 4, 5, 6, 7];
    {
        let log = SpendPageLog::create(&d);
        log.note_slots(&[(16, meta)]).unwrap();
        assert_eq!(log.slot(16).unwrap(), Some(meta));
        commit(&log, 1, pub_len, |_| Ok(true));
        assert_eq!(log.slot(16).unwrap(), Some(meta));
        assert_eq!(log.slots_at(&[16, 24]).unwrap(), vec![Some(meta), None]);
    }
    let log = SpendPageLog::load_clipped(&d, pub_len).unwrap();
    assert_eq!(log.slot(16).unwrap(), Some(meta));
    assert_eq!(log.slots_at(&[16, 24]).unwrap(), vec![Some(meta), None]);
}

#[test]
fn explicit_zero_is_served_and_survives_reload() {
    let d = dir();
    let pub_len = 4096u64;
    let zero = [0u8; 8];
    {
        let log = SpendPageLog::create(&d);
        log.note_slots(&[(16, zero)]).unwrap();
        commit(&log, 1, pub_len, |_| Ok(true));
        assert_eq!(log.slot(16).unwrap(), Some(zero));
        assert!(log.covers(16).unwrap());
    }
    let log = SpendPageLog::load_clipped(&d, pub_len).unwrap();
    assert_eq!(log.slot(16).unwrap(), Some(zero));
    assert!(log.covers(16).unwrap());
}

#[test]
fn later_note_wins_across_commits() {
    let d = dir();
    let pub_len = 4096u64;
    let full = [3u8, 1, 2, 3, 4, 5, 6, 7];
    let zero = [0u8; 8];
    {
        let log = SpendPageLog::create(&d);
        log.note_slots(&[(16, full)]).unwrap();
        commit(&log, 1, pub_len, |_| Ok(true));
        log.note_slots(&[(16, zero)]).unwrap();
        commit(&log, 2, pub_len, |_| Ok(true));
        assert_eq!(log.slot(16).unwrap(), Some(zero));
    }
    let log = SpendPageLog::load_clipped(&d, pub_len).unwrap();
    assert_eq!(log.slot(16).unwrap(), Some(zero));
}

#[test]
fn rejected_new_slot_overrides_the_previous_delta() {
    let d = dir();
    let pub_len = 4096u64;
    let full = [4u8, 1, 2, 3, 4, 5, 6, 7];
    let zero = [0u8; 8];
    {
        let log = SpendPageLog::create(&d);
        log.note_slots(&[(16, full)]).unwrap();
        commit(&log, 1, pub_len, |_| Ok(true));
        log.note_slots(&[(16, full)]).unwrap();
        commit(&log, 2, pub_len, |_| Ok(false));
        assert_eq!(log.slot(16).unwrap(), Some(zero));
    }
    let log = SpendPageLog::load_clipped(&d, pub_len).unwrap();
    assert_eq!(log.slot(16).unwrap(), Some(zero));
}

#[test]
fn later_slot_on_the_same_page_keeps_the_earlier_override() {
    let d = dir();
    let pub_len = 4096u64;
    let a = [5u8, 1, 2, 3, 4, 5, 6, 7];
    let b = [6u8, 1, 2, 3, 4, 5, 6, 7];
    {
        let log = SpendPageLog::create(&d);
        log.note_slots(&[(16, a)]).unwrap();
        commit(&log, 1, pub_len, |_| Ok(true));
        log.note_slots(&[(24, b)]).unwrap();
        commit(&log, 2, pub_len, |_| Ok(true));
        assert_eq!(log.slot(16).unwrap(), Some(a));
        assert_eq!(log.slot(24).unwrap(), Some(b));
    }
    let log = SpendPageLog::load_clipped(&d, pub_len).unwrap();
    assert_eq!(log.slot(16).unwrap(), Some(a));
    assert_eq!(log.slot(24).unwrap(), Some(b));
}

#[test]
fn page_directory_grows_across_pages() {
    let d = dir();
    let pub_len = 3 * 4096u64;
    let a = [7u8, 1, 2, 3, 4, 5, 6, 7];
    let b = [8u8, 1, 2, 3, 4, 5, 6, 7];
    {
        let log = SpendPageLog::create(&d);
        log.note_slots(&[(16, a), (4096 + 16, b)]).unwrap();
        commit(&log, 1, pub_len, |_| Ok(true));
        assert_eq!(log.slot(16).unwrap(), Some(a));
        assert_eq!(log.slot(4096 + 16).unwrap(), Some(b));
    }
    let log = SpendPageLog::load_clipped(&d, pub_len).unwrap();
    assert_eq!(log.slot(16).unwrap(), Some(a));
    assert_eq!(log.slot(4096 + 16).unwrap(), Some(b));
}

#[test]
fn missing_commit_ignores_uncommitted_files() {
    let d = dir();
    let pub_len = 4096u64;
    {
        let log = SpendPageLog::create(&d);
        log.note_slots(&[(16, [1u8, 2, 3, 4, 5, 6, 7, 8])]).unwrap();
        commit(&log, 1, pub_len, |_| Ok(true));
        std::fs::remove_file(d.join(SPEND_DELTA_COMMIT_NAME)).unwrap();
    }
    let log = SpendPageLog::load_clipped(&d, pub_len).unwrap();
    assert_eq!(log.slot(16).unwrap(), None);
}

#[test]
fn crash_before_commit_rename_keeps_the_old_commit_readable() {
    let d = dir();
    let pub_len = 4096u64;
    let a = [1u8, 2, 3, 4, 5, 6, 7, 8];
    let b = [8u8, 7, 6, 5, 4, 3, 2, 1];
    let log = SpendPageLog::create(&d);
    log.note_slots(&[(16, a)]).unwrap();
    commit(&log, 1, pub_len, |_| Ok(true));
    let old = std::fs::read(d.join(SPEND_DELTA_COMMIT_NAME)).unwrap();
    log.note_slots(&[(24, b)]).unwrap();
    commit(&log, 2, pub_len, |_| Ok(true));
    std::fs::write(d.join(SPEND_DELTA_COMMIT_NAME), old).unwrap();
    drop(log);
    let log = SpendPageLog::load_clipped(&d, pub_len).unwrap();
    assert_eq!(log.slot(16).unwrap(), Some(a));
    assert_eq!(log.slot(24).unwrap(), None);
}

#[test]
fn sealed_slots_stay_readable_until_the_commit_is_renamed() {
    let d = dir();
    let pub_len = 4096u64;
    let meta = [4u8, 4, 4, 4, 4, 4, 4, 4];
    let log = SpendPageLog::create(&d);
    log.note_slots(&[(16, meta)]).unwrap();
    log.freeze_open();
    log.prepare_commit_in(
        1,
        pub_len,
        |_, _| Ok(Vec::new()),
        |_| Ok(true),
        |_, _| Ok(()),
        |l| {
            assert_eq!(l.slot(16).unwrap(), Some(meta));
            assert!(l.covers(16).unwrap());
            Ok(1)
        },
    )
    .unwrap();
    assert_eq!(log.slot(16).unwrap(), Some(meta));
}

#[test]
fn keep_error_leaves_the_sealed_slots_in_ram() {
    let d = dir();
    let pub_len = 4096u64;
    let meta = [5u8, 5, 5, 5, 5, 5, 5, 5];
    let log = SpendPageLog::create(&d);
    log.note_slots(&[(16, meta)]).unwrap();
    log.freeze_open();
    let err = log
        .prepare_commit_in(
            1,
            pub_len,
            |_, _| Ok(Vec::new()),
            |_| Err(rbitcoin_store::error::StoreError::Corrupt("keep probe")),
            |_, _| Ok(()),
            |_| Ok(1),
        )
        .unwrap_err();
    assert!(err.to_string().contains("keep probe"));
    assert_eq!(log.slot(16).unwrap(), Some(meta));
}

#[test]
fn clipped_straddling_page_drops_slots_past_the_body() {
    let d = dir();
    let a = [2u8, 2, 2, 2, 2, 2, 2, 2];
    let b = [3u8, 3, 3, 3, 3, 3, 3, 3];
    let log = SpendPageLog::create(&d);
    log.note_slots(&[(16, a), (2048, b)]).unwrap();
    commit(&log, 1, 4096, |_| Ok(true));
    drop(log);
    let log = SpendPageLog::load_clipped(&d, 1024).unwrap();
    assert_eq!(log.slot(16).unwrap(), Some(a));
    assert_eq!(log.slot(2048).unwrap(), None);
    assert!(!log.covers(1500).unwrap());
    drop(log);
    let log = SpendPageLog::load_clipped(&d, 4096).unwrap();
    assert_eq!(log.slot(16).unwrap(), Some(a));
    assert_eq!(log.slot(2048).unwrap(), None);
}

#[test]
fn held_schema_29_log_is_not_folded_during_migration() {
    let d = dir();
    let old = rbitcoin_store::spend_page_log::SpendPageLog::create(&d);
    old.note_slots(&[(16, [1u8, 1, 1, 1, 1, 1, 1, 1])]).unwrap();
    old.commit_open(1, 4096, |_, len| Ok(vec![0u8; len]), |_| Ok(true))
        .unwrap();
    old.hold_fold_above(0).unwrap();
    let err = old.migrate_fold_all(|_, _| Ok(()), || Ok(())).unwrap_err();
    assert!(err.to_string().contains("held"));
}

#[test]
fn a_short_delta_record_is_an_error_not_an_unspent_slot() {
    let d = dir();
    let pub_len = 4096u64;
    let log = SpendPageLog::create(&d);
    log.note_slots(&[(16, [6u8, 6, 6, 6, 6, 6, 6, 6])]).unwrap();
    commit(&log, 1, pub_len, |_| Ok(true));
    drop(log);
    let log = SpendPageLog::load_clipped(&d, pub_len).unwrap();
    let delta = d.join(rbitcoin_store::spend_delta::SPEND_DELTA_NAME);
    let f = std::fs::OpenOptions::new()
        .write(true)
        .open(&delta)
        .unwrap();
    f.set_len(16).unwrap();
    assert!(log.slot(16).is_err());
    assert!(log.slots_at(&[16]).is_err());
}

#[test]
fn a_populated_schema_29_log_opens_under_schema_30() {
    use rbitcoin_primitives::Fk;
    use rbitcoin_store::header_table::HeaderRecord;
    use rbitcoin_store::tx_table::{InputRecord, OutputRecord, TxRecord};

    let d = dir();
    {
        let s = rbitcoin_store::Store::create_tiny(&d).unwrap();
        let hdr = HeaderRecord {
            prev_fk: Fk::NULL,
            version: 1,
            timestamp: 1,
            bits: 0x1d00ffff,
            nonce: 1,
            merkle_root: [1u8; 32],
            hash: [2u8; 32],
            size: 0,
            weight: 0,
        };
        s.put_header(&hdr).unwrap();
        let item = (
            TxRecord {
                txid: [3u8; 32],
                version: 1,
                locktime: 0,
                input_count: 1,
                output_count: 1,
            },
            vec![InputRecord::coinbase(u32::MAX, vec![0x01], vec![])],
            vec![OutputRecord::unspent(1, vec![0x51])],
        );
        s.put_tx_full_batch_indexed(&[item], false).unwrap();
        s.flush().unwrap();
    }
    let meta = [7u8, 7, 7, 7, 7, 7, 7, 7];
    {
        let old = rbitcoin_store::spend_page_log::SpendPageLog::create(&d);
        old.note_slots(&[(16, meta)]).unwrap();
        old.commit_open(1, 24, |_, len| Ok(vec![0u8; len]), |_| Ok(true))
            .unwrap();
    }
    let s = rbitcoin_store::Store::open_tiny(&d).unwrap();
    let published = s.txs.spent.body_published_len();
    let mut got = [0u8; 8];
    s.txs
        .spent
        .read_prefix_at_published(published, 16, 8, &mut got)
        .unwrap();
    assert_eq!(got, meta);
    assert!(!d
        .join(rbitcoin_store::spend_page_log::SPEND_PAGES_COMMIT_NAME)
        .exists());
}
