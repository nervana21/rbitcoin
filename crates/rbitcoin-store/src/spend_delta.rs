//! Schema-30 spend delta overlay over `spent.body`.
//!
//! Confirm notes slots in RAM. A checkpoint seals the open map, writes one
//! cumulative page record for every page it touched to `spent.delta`, updates
//! the dense page directory `spent.delta.idx`, and publishes
//! `spent.delta.commit`. Readers consult the open/sealed maps, one directory
//! entry per page, and then `spent.body`. There is no generation list on the
//! read path and no full-page image on the confirm path.
//!
//! A record holds the complete override set for one page since the body base,
//! including explicit zeroes. The latest directory entry is authoritative.
//! Old records are garbage and are never merged by a reader. This slice does
//! not compact them; `docs/operator/storage.md` carries the growth bound.

use crate::error::StoreError;
use crate::file::{handle_pread_all, write_file_synced_tmp_rename, TableFile};
use crate::tx_table::BODY_PAGE_SIZE;
use rbitcoin_primitives::{schema_file_openable, TableKind, SCHEMA_VERSION, STORE_MAGIC};
use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Condvar, Mutex};

pub const SPEND_DELTA_NAME: &str = "spent.delta";
pub const SPEND_DELTA_IDX_NAME: &str = "spent.delta.idx";
pub const SPEND_DELTA_COMMIT_NAME: &str = "spent.delta.commit";

/// Open slot deltas that make the next confirm insert wait for a checkpoint.
pub const OPEN_EDGE_CAP: u64 = 2_000_000;
/// Distinct open pages that ask for an earlier checkpoint.
pub const OPEN_PAGE_BUDGET: u64 = 65_536;
const SLOT: usize = 8;
const PAGE: u64 = BODY_PAGE_SIZE;
const COMMIT_HEAD: usize = 32;
const DIR_ENTRY: u64 = 16;
const DIR_STRIDE: u64 = DIR_ENTRY * 2;
/// Record header: page_off + count.
const REC_HEAD: usize = 10;
const DIR_MAGIC_OFF: u64 = 16;

#[inline]
fn rd_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}
#[inline]
fn rd_u32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}
#[inline]
fn rd_u64(b: &[u8], o: usize) -> u64 {
    let mut a = [0u8; 8];
    a.copy_from_slice(&b[o..o + 8]);
    u64::from_le_bytes(a)
}

type Slot = [u8; SLOT];
type PageSlots = BTreeMap<u16, Slot>;
type Layer = HashMap<u64, PageSlots>;

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
struct DeltaLoc {
    generation: u32,
    offset: u64,
    len: u32,
}

impl DeltaLoc {
    fn is_some(self) -> bool {
        self.len != 0
    }
}

/// `spend_durable` sync thread wakes on this.
pub type SpendSyncWake = Arc<(Mutex<bool>, Condvar)>;

struct Pace {
    early: AtomicBool,
    sync_wake: Mutex<Option<SpendSyncWake>>,
    room: Condvar,
    room_mu: Mutex<()>,
}

impl Pace {
    fn new() -> Self {
        Self {
            early: AtomicBool::new(false),
            sync_wake: Mutex::new(None),
            room: Condvar::new(),
            room_mu: Mutex::new(()),
        }
    }

    fn arm(&self) {
        self.early.store(true, Ordering::Release);
        let wake = self
            .sync_wake
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone();
        let Some(wake) = wake else {
            return;
        };
        let (lock, cv) = &*wake;
        let _g = lock.lock().unwrap_or_else(|e| e.into_inner());
        cv.notify_one();
    }

    fn notify_room(&self) {
        let _g = self.room_mu.lock().unwrap_or_else(|e| e.into_inner());
        self.room.notify_all();
    }
}

struct Inner {
    open: Layer,
    sealed: Option<Arc<Layer>>,
    directory: Vec<DeltaLoc>,
    dir: Arc<TableFile>,
    delta: Arc<TableFile>,
    delta_len: u64,
    generation: u32,
    height: Option<u32>,
    sealed_tip: Option<u32>,
    fold_held: bool,
    open_edges: u64,
}

pub struct SpendPageLog {
    dir_path: PathBuf,
    inner: Mutex<Inner>,
    pace: Arc<Pace>,
}

/// True when the 8-byte slot lies inside the published body.
pub fn is_historical(published: u64, abs: u64) -> bool {
    abs.saturating_add(SLOT as u64) <= published
}

fn page_of(abs: u64) -> u64 {
    abs & !(PAGE - 1)
}

fn rel_of(abs: u64) -> u16 {
    ((abs - page_of(abs)) / SLOT as u64) as u16
}

fn open_or_create(path: &Path, kind: TableKind) -> Result<TableFile, StoreError> {
    if path.exists() {
        TableFile::open(path, kind)
    } else {
        TableFile::create(path, kind)
    }
}

fn decode_delta(buf: &[u8], page_off: u64) -> Result<PageSlots, StoreError> {
    if buf.len() < REC_HEAD {
        return Err(StoreError::Corrupt("spent.delta short record"));
    }
    let got_page = rd_u64(buf, 0);
    if got_page != page_off {
        return Err(StoreError::Corrupt("spent.delta page mismatch"));
    }
    let n = rd_u16(buf, 8) as usize;
    if REC_HEAD.saturating_add(n.saturating_mul(10)) != buf.len() {
        return Err(StoreError::Corrupt("spent.delta bad record length"));
    }
    let mut out = BTreeMap::new();
    let mut at = REC_HEAD;
    let mut prev = None;
    for _ in 0..n {
        let rel = u16::from_le_bytes([buf[at], buf[at + 1]]);
        if prev.is_some_and(|p| rel <= p) {
            return Err(StoreError::Corrupt("spent.delta rel order"));
        }
        prev = Some(rel);
        let mut meta = [0u8; SLOT];
        meta.copy_from_slice(&buf[at + 2..at + 10]);
        out.insert(rel, meta);
        at += 10;
    }
    Ok(out)
}

fn self_path(path: &Path) -> PathBuf {
    path.join(SPEND_DELTA_NAME)
}

fn read_delta_file(
    delta: &TableFile,
    path: &Path,
    page_off: u64,
    loc: DeltaLoc,
) -> Result<PageSlots, StoreError> {
    let mut buf = vec![0u8; loc.len as usize];
    handle_pread_all(&delta.read_file, path, loc.offset, &mut buf)?;
    decode_delta(&buf, page_off)
}

#[allow(clippy::too_many_arguments)]
fn finish_load(
    dir_path: PathBuf,
    directory: Vec<DeltaLoc>,
    dir: TableFile,
    delta: TableFile,
    delta_len: u64,
    generation: u32,
    height: u32,
    sealed_tip: u32,
    fold_held: bool,
) -> Result<SpendPageLog, StoreError> {
    Ok(SpendPageLog {
        dir_path,
        inner: Mutex::new(Inner {
            open: HashMap::new(),
            sealed: None,
            directory,
            dir: Arc::new(dir),
            delta: Arc::new(delta),
            delta_len,
            generation,
            height: Some(height),
            sealed_tip: Some(sealed_tip),
            fold_held,
            open_edges: 0,
        }),
        pace: Arc::new(Pace::new()),
    })
}

fn encode_delta(page_off: u64, slots: &PageSlots) -> Vec<u8> {
    let mut buf = Vec::with_capacity(REC_HEAD + slots.len().saturating_mul(10));
    buf.extend_from_slice(&page_off.to_le_bytes());
    buf.extend_from_slice(&(slots.len() as u16).to_le_bytes());
    for (&rel, meta) in slots {
        buf.extend_from_slice(&rel.to_le_bytes());
        buf.extend_from_slice(meta);
    }
    buf
}

fn entry_bytes(loc: DeltaLoc) -> [u8; DIR_ENTRY as usize] {
    let mut b = [0u8; DIR_ENTRY as usize];
    b[0..4].copy_from_slice(&loc.generation.to_le_bytes());
    b[4..12].copy_from_slice(&loc.offset.to_le_bytes());
    b[12..16].copy_from_slice(&loc.len.to_le_bytes());
    b
}

fn parse_entry(buf: &[u8]) -> DeltaLoc {
    DeltaLoc {
        generation: rd_u32(buf, 0),
        offset: rd_u64(buf, 4),
        len: rd_u32(buf, 12),
    }
}

/// Pick the newest entry at or below the commit generation. The two slots are
/// written alternately, so an older commit keeps reading the slot it named.
fn select_loc(slots: &[DeltaLoc], generation: u32) -> DeltaLoc {
    let mut best = DeltaLoc::default();
    for &loc in slots {
        if loc.is_some() && loc.generation <= generation && loc.generation >= best.generation {
            best = loc;
        }
    }
    best
}

fn encode_commit(
    height: u32,
    generation: u32,
    delta_len: u64,
    dir_entries: u64,
    sealed_tip: u32,
    fold_held: bool,
) -> Vec<u8> {
    let mut b = [0u8; COMMIT_HEAD];
    b[0..4].copy_from_slice(&STORE_MAGIC);
    b[4..6].copy_from_slice(&SCHEMA_VERSION.to_le_bytes());
    if fold_held {
        b[6..8].copy_from_slice(&1u16.to_le_bytes());
    }
    b[8..12].copy_from_slice(&height.to_le_bytes());
    b[12..16].copy_from_slice(&generation.to_le_bytes());
    b[16..24].copy_from_slice(&delta_len.to_le_bytes());
    b[24..32 - 4].copy_from_slice(&dir_entries.to_le_bytes()[..4]);
    b[28..32].copy_from_slice(&sealed_tip.to_le_bytes());
    b.to_vec()
}

struct Commit {
    height: u32,
    generation: u32,
    delta_len: u64,
    dir_entries: u32,
    sealed_tip: u32,
    fold_held: bool,
}

fn parse_commit(buf: &[u8]) -> Result<Commit, StoreError> {
    if buf.len() < COMMIT_HEAD {
        return Err(StoreError::Corrupt("spent.delta.commit short"));
    }
    if buf[0..4] != STORE_MAGIC {
        return Err(StoreError::BadMagic);
    }
    let ver = rd_u16(buf, 4);
    if !schema_file_openable(ver) {
        return Err(StoreError::BadSchema(ver));
    }
    let mut dirb = [0u8; 4];
    dirb.copy_from_slice(&buf[24..28]);
    Ok(Commit {
        fold_held: rd_u16(buf, 6) != 0,
        height: rd_u32(buf, 8),
        generation: rd_u32(buf, 12),
        delta_len: rd_u64(buf, 16),
        dir_entries: u32::from_le_bytes(dirb),
        sealed_tip: rd_u32(buf, 28),
    })
}

impl SpendPageLog {
    pub fn create(dir: &Path) -> Self {
        let path = dir.to_path_buf();
        let delta = TableFile::create(dir.join(SPEND_DELTA_NAME), TableKind::SpendDelta)
            .expect("create spent.delta");
        let dir_file = TableFile::create(dir.join(SPEND_DELTA_IDX_NAME), TableKind::SpendDeltaIdx)
            .expect("create spent.delta.idx");
        Self {
            dir_path: path,
            inner: Mutex::new(Inner {
                open: HashMap::new(),
                sealed: None,
                directory: Vec::new(),
                dir: Arc::new(dir_file),
                delta: Arc::new(delta),
                delta_len: 16,
                generation: 0,
                height: None,
                sealed_tip: None,
                fold_held: false,
                open_edges: 0,
            }),
            pace: Arc::new(Pace::new()),
        }
    }

    pub fn load(dir: &Path) -> Result<Self, StoreError> {
        Self::load_clipped(dir, u64::MAX)
    }

    pub fn load_clipped(dir: &Path, body_end: u64) -> Result<Self, StoreError> {
        let path = dir.to_path_buf();
        let delta = open_or_create(&dir.join(SPEND_DELTA_NAME), TableKind::SpendDelta)?;
        let dir_file = open_or_create(&dir.join(SPEND_DELTA_IDX_NAME), TableKind::SpendDeltaIdx)?;
        let commit_path = dir.join(SPEND_DELTA_COMMIT_NAME);
        if !commit_path.exists() {
            delta.set_logical_len(16)?;
            dir_file.set_logical_len(16)?;
            return Ok(Self {
                dir_path: path,
                inner: Mutex::new(Inner {
                    open: HashMap::new(),
                    sealed: None,
                    directory: Vec::new(),
                    dir: Arc::new(dir_file),
                    delta: Arc::new(delta),
                    delta_len: 16,
                    generation: 0,
                    height: None,
                    sealed_tip: None,
                    fold_held: false,
                    open_edges: 0,
                }),
                pace: Arc::new(Pace::new()),
            });
        }
        let raw = std::fs::read(&commit_path).map_err(|e| StoreError::io(&commit_path, e))?;
        let commit = parse_commit(&raw)?;
        let delta_len = commit.delta_len.max(16);
        if delta.logical_len() < delta_len {
            return Err(StoreError::Corrupt("spent.delta short"));
        }
        let mut directory = Vec::with_capacity(commit.dir_entries as usize * 2);
        if commit.dir_entries > 0 {
            let mut buf = vec![0u8; commit.dir_entries as usize * DIR_STRIDE as usize];
            let end = DIR_MAGIC_OFF + buf.len() as u64;
            if dir_file.logical_len() < end {
                return Err(StoreError::Corrupt("spent.delta.idx short"));
            }
            dir_file.pread_at(DIR_MAGIC_OFF, &mut buf)?;
            for chunk in buf.chunks_exact(DIR_ENTRY as usize) {
                directory.push(parse_entry(chunk));
            }
        }
        let mut clipped = false;
        let mut new_generation = commit.generation;
        let mut new_delta_len = delta_len;
        let mut new_dir_entries = commit.dir_entries as u64;
        for page in 0..(directory.len() / 2) {
            let page_off = page as u64 * PAGE;
            let loc = select_loc(&directory[page * 2..page * 2 + 2], commit.generation);
            if page_off >= body_end {
                if loc.is_some() {
                    clipped = true;
                }
                directory[page * 2] = DeltaLoc::default();
                directory[page * 2 + 1] = DeltaLoc::default();
                continue;
            }
            if !loc.is_some() {
                continue;
            }
            if loc.offset.saturating_add(loc.len as u64) > delta_len {
                return Err(StoreError::Corrupt(
                    "spent.delta record past committed length",
                ));
            }
            let slots = read_delta_file(&delta, &self_path(&path), page_off, loc)?;
            let kept: PageSlots = slots
                .iter()
                .filter(|(rel, _)| {
                    page_off.saturating_add((u64::from(**rel) + 1).saturating_mul(SLOT as u64))
                        <= body_end
                })
                .map(|(rel, meta)| (*rel, *meta))
                .collect();
            if kept.len() != slots.len() {
                clipped = true;
                if new_generation == commit.generation {
                    new_generation = new_generation.wrapping_add(1).max(1);
                }
                if !kept.is_empty() {
                    let bytes = encode_delta(page_off, &kept);
                    delta.write_uncommitted(new_delta_len, &bytes)?;
                    directory[page * 2 + (new_generation as usize & 1)] = DeltaLoc {
                        generation: new_generation,
                        offset: new_delta_len,
                        len: bytes.len() as u32,
                    };
                    new_delta_len = new_delta_len.saturating_add(bytes.len() as u64);
                } else {
                    directory[page * 2] = DeltaLoc::default();
                    directory[page * 2 + 1] = DeltaLoc::default();
                }
            }
        }
        if clipped {
            for page in (new_dir_entries as usize)..(directory.len() / 2) {
                directory[page * 2] = DeltaLoc::default();
                directory[page * 2 + 1] = DeltaLoc::default();
            }
            new_dir_entries = (0..(directory.len() / 2))
                .rev()
                .find(|&page| {
                    select_loc(&directory[page * 2..page * 2 + 2], new_generation).is_some()
                })
                .map_or(0, |page| page as u64 + 1);
            directory.truncate(new_dir_entries as usize * 2);
            if new_delta_len > delta_len {
                if !delta.publish_end(new_delta_len, true) {
                    return Err(StoreError::io(
                        path.join(SPEND_DELTA_NAME),
                        std::io::Error::other("spent.delta clip header publish failed"),
                    ));
                }
                delta.sync_data_only()?;
            }
            for page in 0..new_dir_entries as usize {
                let slot = new_generation as usize & 1;
                let loc = directory[page * 2 + slot];
                if loc.is_some() {
                    dir_file.write_uncommitted(
                        DIR_MAGIC_OFF + page as u64 * DIR_STRIDE + slot as u64 * DIR_ENTRY,
                        &entry_bytes(loc),
                    )?;
                }
            }
            if !dir_file.publish_end(DIR_MAGIC_OFF + new_dir_entries * DIR_STRIDE, true) {
                return Err(StoreError::io(
                    path.join(SPEND_DELTA_IDX_NAME),
                    std::io::Error::other("spent.delta.idx clip header publish failed"),
                ));
            }
            dir_file.sync_data_only()?;
            let commit_bytes = encode_commit(
                commit.height,
                new_generation,
                new_delta_len,
                new_dir_entries,
                commit.sealed_tip,
                commit.fold_held,
            );
            write_file_synced_tmp_rename(&commit_path, &commit_bytes)?;
            return finish_load(
                path,
                directory,
                dir_file,
                delta,
                new_delta_len,
                new_generation,
                commit.height,
                commit.sealed_tip,
                commit.fold_held,
            );
        }
        Ok(Self {
            dir_path: path,
            inner: Mutex::new(Inner {
                open: HashMap::new(),
                sealed: None,
                directory,
                dir: Arc::new(dir_file),
                delta: Arc::new(delta),
                delta_len,
                generation: commit.generation,
                height: Some(commit.height),
                sealed_tip: Some(commit.sealed_tip),
                fold_held: commit.fold_held,
                open_edges: 0,
            }),
            pace: Arc::new(Pace::new()),
        })
    }

    pub fn bind_sync_wake(&self, wake: SpendSyncWake) {
        *self
            .pace
            .sync_wake
            .lock()
            .unwrap_or_else(|e| e.into_inner()) = Some(wake);
    }

    pub fn checkpoint_early(&self) -> bool {
        self.pace.early.load(Ordering::Acquire)
    }

    pub fn relax_pace(&self) {
        let (edges, pages) = {
            let g = self.lock();
            (
                g.open_edges,
                (g.open.len() + g.sealed.as_ref().map_or(0, |m| m.len())) as u64,
            )
        };
        let early = edges >= OPEN_EDGE_CAP || pages >= OPEN_PAGE_BUDGET;
        self.pace.early.store(early, Ordering::Release);
        if early {
            self.pace.arm();
        }
    }

    pub fn open_edge_count(&self) -> u64 {
        self.lock().open_edges
    }

    pub fn wait_open_edge_room(&self) {
        loop {
            let room = self.pace.room_mu.lock().unwrap_or_else(|e| e.into_inner());
            if self.lock().open_edges < OPEN_EDGE_CAP {
                return;
            }
            let bound = self
                .pace
                .sync_wake
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .is_some();
            if !bound {
                return;
            }
            self.pace.arm();
            drop(self.pace.room.wait(room).unwrap_or_else(|e| e.into_inner()));
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// True when the directory serves this exact slot.
    pub fn covers(&self, abs: u64) -> Result<bool, StoreError> {
        let page_off = page_of(abs);
        let rel = rel_of(abs);
        let loc = {
            let g = self.lock();
            if g.open.get(&page_off).is_some_and(|p| p.contains_key(&rel)) {
                return Ok(true);
            }
            if g.sealed
                .as_ref()
                .and_then(|m| m.get(&page_off))
                .is_some_and(|p| p.contains_key(&rel))
            {
                return Ok(true);
            }
            let page = (page_off / PAGE) as usize;
            if page * 2 + 1 >= g.directory.len() {
                DeltaLoc::default()
            } else {
                select_loc(&g.directory[page * 2..page * 2 + 2], g.generation)
            }
        };
        if !loc.is_some() {
            return Ok(false);
        }
        Ok(self.read_delta(page_off, loc)?.contains_key(&rel))
    }

    /// Page-level fast path for the write-side skip decision. A page that has
    /// any delta is conservatively logged; a later cumulative record dedups it.
    pub fn page_has_delta(&self, abs: u64) -> bool {
        let page = (page_of(abs) / PAGE) as usize;
        let g = self.lock();
        page * 2 + 1 < g.directory.len()
            && select_loc(&g.directory[page * 2..page * 2 + 2], g.generation).is_some()
    }

    pub fn slot(&self, abs: u64) -> Result<Option<Slot>, StoreError> {
        let page_off = page_of(abs);
        let rel = rel_of(abs);
        let loc = {
            let g = self.lock();
            if let Some(meta) = g.open.get(&page_off).and_then(|p| p.get(&rel)) {
                return Ok(Some(*meta));
            }
            if let Some(meta) = g
                .sealed
                .as_ref()
                .and_then(|m| m.get(&page_off))
                .and_then(|p| p.get(&rel))
            {
                return Ok(Some(*meta));
            }
            let page = (page_off / PAGE) as usize;
            if page * 2 + 1 >= g.directory.len() {
                DeltaLoc::default()
            } else {
                select_loc(&g.directory[page * 2..page * 2 + 2], g.generation)
            }
        };
        if !loc.is_some() {
            return Ok(None);
        }
        let slots = self.read_delta(page_off, loc)?;
        Ok(slots.get(&rel).copied())
    }

    pub fn slots_at(&self, abs: &[u64]) -> Result<Vec<Option<Slot>>, StoreError> {
        if abs.is_empty() {
            return Ok(Vec::new());
        }
        let mut out = vec![None; abs.len()];
        let mut order: Vec<(u64, u16, usize)> = abs
            .iter()
            .enumerate()
            .map(|(i, &off)| (page_of(off), rel_of(off), i))
            .collect();
        order.sort_unstable_by_key(|(page, _, i)| (*page, *i));
        struct Pending {
            page_off: u64,
            loc: DeltaLoc,
            slots: Vec<(u16, usize)>,
        }
        let mut pending: Vec<Pending> = Vec::new();
        {
            let g = self.lock();
            let mut start = 0;
            while start < order.len() {
                let page_off = order[start].0;
                let mut end = start + 1;
                while end < order.len() && order[end].0 == page_off {
                    end += 1;
                }
                let open = g.open.get(&page_off);
                let sealed = g.sealed.as_ref().and_then(|m| m.get(&page_off));
                let mut miss = Vec::new();
                for &(_, rel, i) in &order[start..end] {
                    if let Some(meta) = open.and_then(|p| p.get(&rel)) {
                        out[i] = Some(*meta);
                    } else if let Some(meta) = sealed.and_then(|p| p.get(&rel)) {
                        out[i] = Some(*meta);
                    } else {
                        miss.push((rel, i));
                    }
                }
                if !miss.is_empty() {
                    let page = (page_off / PAGE) as usize;
                    let loc = if page * 2 + 1 >= g.directory.len() {
                        DeltaLoc::default()
                    } else {
                        select_loc(&g.directory[page * 2..page * 2 + 2], g.generation)
                    };
                    if loc.is_some() {
                        pending.push(Pending {
                            page_off,
                            loc,
                            slots: miss,
                        });
                    }
                }
                start = end;
            }
        }
        for p in pending {
            let slots = self.read_delta(p.page_off, p.loc)?;
            for (rel, i) in p.slots {
                if let Some(meta) = slots.get(&rel) {
                    out[i] = Some(*meta);
                }
            }
        }
        Ok(out)
    }

    pub fn note_slots(&self, slots: &[(u64, Slot)]) -> Result<(), StoreError> {
        if slots.is_empty() {
            return Ok(());
        }
        let mut added = false;
        {
            let mut g = self.lock();
            for &(abs, meta) in slots {
                let page_off = page_of(abs);
                let rel = rel_of(abs);
                if (rel as usize) >= (PAGE as usize / SLOT) {
                    return Err(StoreError::Corrupt("spend delta slot past page"));
                }
                let page = g.open.entry(page_off).or_default();
                if page.insert(rel, meta).is_none() {
                    g.open_edges = g.open_edges.saturating_add(1);
                    added = true;
                }
            }
            let pages = (g.open.len() + g.sealed.as_ref().map_or(0, |m| m.len())) as u64;
            added = added && (g.open_edges >= OPEN_EDGE_CAP || pages >= OPEN_PAGE_BUDGET);
        }
        if added {
            self.pace.arm();
        }
        Ok(())
    }

    pub fn cover_confirmed_tip(&self, tip: u32) {
        let mut g = self.lock();
        g.sealed_tip = Some(g.sealed_tip.map_or(tip, |prev| prev.max(tip)));
    }

    pub fn freeze_open(&self) {
        let mut g = self.lock();
        let open = std::mem::take(&mut g.open);
        g.open_edges = 0;
        if open.is_empty() {
            return;
        }
        let mut merged = match &g.sealed {
            Some(sealed) => (**sealed).clone(),
            None => HashMap::new(),
        };
        for (page, slots) in open {
            merged.entry(page).or_default().extend(slots);
        }
        g.sealed = Some(Arc::new(merged));
    }

    pub fn hold_fold_above(&self, tip: u32) -> Result<(), StoreError> {
        let path = self.dir_path.join(SPEND_DELTA_COMMIT_NAME);
        let mut lower = false;
        {
            let mut g = self.lock();
            g.fold_held = true;
            if let Some(h) = g.height {
                if h > tip {
                    g.height = Some(tip);
                    lower = true;
                }
            }
        }
        if !path.exists() {
            return Ok(());
        }
        let mut buf = std::fs::read(&path).map_err(|e| StoreError::io(&path, e))?;
        if buf.len() < COMMIT_HEAD {
            return Ok(());
        }
        buf[6..8].copy_from_slice(&1u16.to_le_bytes());
        if lower {
            buf[8..12].copy_from_slice(&tip.to_le_bytes());
        }
        write_file_synced_tmp_rename(&path, &buf)
    }

    pub fn above_tip(&self, tip: u32) -> bool {
        let g = self.lock();
        g.sealed_tip.is_some_and(|h| h > tip) || g.height.is_some_and(|h| h > tip)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_commit_in<F, K, B, S>(
        &self,
        height: u32,
        _published: u64,
        _read_page: F,
        mut keep: K,
        before_freeze: B,
        seal_tip: S,
    ) -> Result<(), StoreError>
    where
        F: FnMut(u64, usize) -> Result<Vec<u8>, StoreError>,
        K: FnMut(&Slot) -> Result<bool, StoreError>,
        B: FnOnce(&Self, &dyn Fn() -> u64) -> Result<(), StoreError>,
        S: FnOnce(&Self) -> Result<u32, StoreError>,
    {
        before_freeze(self, &|| 0)?;
        let (sealed, delta_file, dir_file, fold_held) = {
            let g = self.lock();
            (
                g.sealed.clone(),
                Arc::clone(&g.delta),
                Arc::clone(&g.dir),
                g.fold_held,
            )
        };
        let Some(sealed) = sealed else {
            return Ok(());
        };
        let sealed_tip = seal_tip(self)?;
        let generation = {
            let mut g = self.lock();
            g.generation = g.generation.wrapping_add(1).max(1);
            g.generation
        };
        let mut delta_len = self.lock().delta_len;
        let mut dir_entries = (self.lock().directory.len() / 2) as u64;
        let mut writes: Vec<(u64, DeltaLoc)> = Vec::new();
        for (&page_off, new_slots) in sealed.iter() {
            let old = self.directory_loc(page_off);
            let mut merged = if old.is_some() {
                self.read_delta(page_off, old)?
            } else {
                BTreeMap::new()
            };
            for (rel, meta) in new_slots {
                merged.insert(*rel, *meta);
            }
            for meta in merged.values_mut() {
                if *meta != [0u8; SLOT] && !keep(meta)? {
                    *meta = [0u8; SLOT];
                }
            }
            let bytes = encode_delta(page_off, &merged);
            if bytes.len() > u32::MAX as usize {
                return Err(StoreError::Corrupt("spent.delta record too long"));
            }
            delta_file.write_uncommitted(delta_len, &bytes)?;
            let loc = DeltaLoc {
                generation,
                offset: delta_len,
                len: bytes.len() as u32,
            };
            delta_len = delta_len.saturating_add(bytes.len() as u64);
            let idx = (page_off / PAGE) as usize;
            if idx >= dir_entries as usize {
                dir_entries = idx as u64 + 1;
            }
            writes.push((page_off, loc));
        }
        if !delta_file.publish_end(delta_len, true) {
            return Err(StoreError::io(
                self.dir_path.join(SPEND_DELTA_NAME),
                std::io::Error::other("spent.delta header publish failed"),
            ));
        }
        delta_file.sync_data_only()?;
        for (page_off, loc) in &writes {
            let idx = (*page_off / PAGE) as usize;
            let slot = (generation & 1) as usize;
            dir_file.write_uncommitted(
                DIR_MAGIC_OFF + idx as u64 * DIR_STRIDE + slot as u64 * DIR_ENTRY,
                &entry_bytes(*loc),
            )?;
        }
        if dir_entries > 0 {
            if !dir_file.publish_end(DIR_MAGIC_OFF + dir_entries * DIR_STRIDE, true) {
                return Err(StoreError::io(
                    self.dir_path.join(SPEND_DELTA_IDX_NAME),
                    std::io::Error::other("spent.delta.idx header publish failed"),
                ));
            }
            dir_file.sync_data_only()?;
        }
        {
            let mut g = self.lock();
            while g.directory.len() < dir_entries as usize * 2 {
                g.directory.push(DeltaLoc::default());
            }
            for (page_off, loc) in &writes {
                let idx = (*page_off / PAGE) as usize;
                let slot = (generation & 1) as usize;
                g.directory[idx * 2 + slot] = *loc;
            }
            g.delta_len = delta_len;
            g.height = Some(height);
            g.sealed_tip = Some(sealed_tip);
        }
        let commit = encode_commit(
            height,
            generation,
            delta_len,
            dir_entries,
            sealed_tip,
            fold_held,
        );
        let path = self.dir_path.join(SPEND_DELTA_COMMIT_NAME);
        write_file_synced_tmp_rename(&path, &commit)?;
        {
            let mut g = self.lock();
            if g.sealed
                .as_ref()
                .is_some_and(|cur| Arc::ptr_eq(cur, &sealed))
            {
                g.sealed = None;
            }
        }
        self.pace.notify_room();
        Ok(())
    }

    fn directory_loc(&self, page_off: u64) -> DeltaLoc {
        let g = self.lock();
        let page = (page_off / PAGE) as usize;
        if page * 2 + 1 >= g.directory.len() {
            return DeltaLoc::default();
        }
        select_loc(&g.directory[page * 2..page * 2 + 2], g.generation)
    }

    fn read_delta(&self, page_off: u64, loc: DeltaLoc) -> Result<PageSlots, StoreError> {
        if !loc.is_some() {
            return Ok(BTreeMap::new());
        }
        let file = self
            .lock()
            .delta
            .read_file
            .try_clone()
            .map_err(|e| StoreError::io(self.dir_path.join(SPEND_DELTA_NAME), e))?;
        let mut buf = vec![0u8; loc.len as usize];
        handle_pread_all(
            &file,
            &self.dir_path.join(SPEND_DELTA_NAME),
            loc.offset,
            &mut buf,
        )?;
        decode_delta(&buf, page_off)
    }
}
