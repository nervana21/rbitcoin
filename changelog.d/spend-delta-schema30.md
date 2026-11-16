Changed

- **Schema 30 replaces the spend page-image log with a cumulative page delta
  overlay over `spent.body`.** Confirm notes slots in RAM and a checkpoint
  writes only changed slots into `spent.delta`; `spent.delta.idx` is a dense
  page directory and `spent.delta.commit` is the snapshot. Readers check the
  open/sealed maps, one directory entry per page, then `spent.body`. The
  schema-29 `spend.pages` log is folded once into `spent.body` on open and
  removed. `spent.body` and `create.loc` offsets are unchanged, so there is no
  Class A reindex. A 29 binary refuses 30 `meta`.
