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
        assert_eq!(log.slot(16), Some(meta));
        commit(&log, 1, pub_len, |_| Ok(true));
        assert_eq!(log.slot(16), Some(meta));
        assert_eq!(log.slots_at(&[16, 24]), vec![Some(meta), None]);
    }
    let log = SpendPageLog::load_clipped(&d, pub_len).unwrap();
    assert_eq!(log.slot(16), Some(meta));
    assert_eq!(log.slots_at(&[16, 24]), vec![Some(meta), None]);
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
        assert_eq!(log.slot(16), Some(zero));
        assert!(log.covers(16));
    }
    let log = SpendPageLog::load_clipped(&d, pub_len).unwrap();
    assert_eq!(log.slot(16), Some(zero));
    assert!(log.covers(16));
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
        assert_eq!(log.slot(16), Some(zero));
    }
    let log = SpendPageLog::load_clipped(&d, pub_len).unwrap();
    assert_eq!(log.slot(16), Some(zero));
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
        assert_eq!(log.slot(16), Some(zero));
    }
    let log = SpendPageLog::load_clipped(&d, pub_len).unwrap();
    assert_eq!(log.slot(16), Some(zero));
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
        assert_eq!(log.slot(16), Some(a));
        assert_eq!(log.slot(24), Some(b));
    }
    let log = SpendPageLog::load_clipped(&d, pub_len).unwrap();
    assert_eq!(log.slot(16), Some(a));
    assert_eq!(log.slot(24), Some(b));
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
        assert_eq!(log.slot(16), Some(a));
        assert_eq!(log.slot(4096 + 16), Some(b));
    }
    let log = SpendPageLog::load_clipped(&d, pub_len).unwrap();
    assert_eq!(log.slot(16), Some(a));
    assert_eq!(log.slot(4096 + 16), Some(b));
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
    assert_eq!(log.slot(16), None);
}
