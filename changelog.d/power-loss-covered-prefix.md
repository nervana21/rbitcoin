Fixed

- **Covered Class A prefix.** A body header shorter than the locator count
  used to refuse startup. Open now truncates to the last create those
  headers still name.
- **Short input body.** A short `input.body` header cuts `input.loc` to the
  creates that header still names.
- **Short-input probe.** The probe runs only after that fit, and only when
  the next row is still inside the fitted count.
- **Witness row past the body.** A witness row past the body is not read;
  the input count is the prefix that landed.
- **Tip shrink before clear.** Open shrinks the tip to match, then clears
  `header_txs` ranges past that prefix.
- **Spend recheck before cut.** A cut that drops creates syncs
  `(covered, old]` in `spend.recheck` before the Class A truncate.
- **Header range end.** `old` is also the highest `header_txs` range end
  still past the kept count, noted before those ranges are cleared.
- **Reused spender.** A spender number inside one of those spans counts
  only when that create's input at the stored vin still names the parent
  and vout.
- **Same spend check.** Block validation and the Electrum spend join use
  that same check.
- **Unstrong orphan.** An unstrong orphan at that cut does not keep later
  confirmed blocks, including when those ranges were already cleared.
- **Torn Class C array.** A Class C array torn mid-record opens at the last
  whole row.
- **Torn `tx.head/` directory.** A `tx.head/` directory with segment files
  and no `meta` rebuilds. That directory used to refuse as a legacy shard;
  open wipes it and rebuilds from Class A.
- **Legacy head layouts.** A monolithic `tx.head` and a flat `tx.head.meta`
  still refuse.
- **Synced rename.** Store `meta`, `store.secret`, `seqsigwit.prune`,
  `seqsigwit.gap`, mempool `meta`, and `unbroadcast` no longer truncate the
  live name.
- **Short prune file.** A short prune file is not a height.
- **Missing witness locator.** Open keeps creates the witness locator no
  longer names, and those creates have no scriptSig or witness.
- **Missing locator without prune.** Without the prune file, that missing
  locator still drops the block.
- **Short mempool meta.** A short mempool `meta` is quarantined and the
  pool starts empty.
- **Secret mode.** `store.secret` is mode 0600 on the temporary inode,
  before the name is published.
- **Empty witness rows.** Open appends an empty witness row for each create
  a short, non-zero locator did not name.
- **Short witness body.** A witness body that does not cover the last named
  row is cut back to the last row it still covers before that fill,
  including when the locator count already matches the creates.
- **No covered witness row.** Open refuses when the body covers no witness
  row.
- **Locator at zero.** A locator already cut to zero stays empty, so the
  keep-window rewrite is not handed those rows.
- **Torn marker append.** A 0-byte prune file with a positive locator is
  only that torn marker, so the following append still writes the witness
  stem.
- **Finished cut sentinel.** A 0-byte file whose locator is already 0 is
  the finished cut: open writes a 4-byte sentinel height and does not fill
  empty rows.
- **Empty keep window.** When that cut has no keep-window files, the prune
  height becomes the tip.
- **Missing keep prefix.** A missing prefix of an otherwise present window
  sets the prune height to the last missing height, and the files still on
  disk stay.
- **Next scriptSig.** The next block's scriptSig is written to the keep
  window.
- **Missing tip file.** A missing tip file does not delete the older files,
  and open refuses instead of starting a node that cannot seal the next
  block.
- **Prune height.** A 4-byte height is prune mode. A positive short locator
  on that height is the prefix that landed, so open drops the creates past
  it.
- **Finished cut chain.** Locator 0 is the finished cut and does not drop
  the chain.
- **In-process stem cut.** A stem cut in this process is that same finished
  cut: a body header that still names every create still seals the tip.
- **Restart witness.** The empty rows are still not a witness after a
  restart, and the tip stays.
- **Gap sync.** The gap file is synced before the empty witness rows and
  the staged locator are written.
- **Gap delete sync.** Deleting a gap span syncs the store directory.
- **Later gap span.** A later torn locator is its own span, so a witness
  stored between the two fills stays available.
- **Locator ahead of count.** A witness locator ahead of the covered create
  count is cut back to that count, and a gap span past it is dropped.
- **Height drops gap.** A 4-byte height drops that span too, including when
  open does not fill empty rows.
- **Matched locator.** When that locator already matches, the witness body
  counter is set back to that count and a tail past it is dropped before
  the next append.
- **Pruned confirm keep file.** A pruned confirm still writes the keep file
  from the witness it just appended, even when the locator does not name
  that create.
- **Seal reads keep file.** A later seal with an empty append cache reads
  that keep file.
- **Seal failure.** A height at or below the prune height, and a missing
  keep file, fail the seal.
- **Empty pad.** Turning prune on skips an empty pad instead of decoding it
  as an input.
- **Shortened `tx.head`.** A `tx.head` that still has `meta` is shortened
  to that count.
- **Dropped segment.** A segment that starts past that count is unlinked
  (open-address file, `.mphf`, and `.fuse8`).
- **Closed maps.** Its maps are closed first, so the unlink still works
  while this process had the segment open.
- **Kept segment.** A segment that stays keeps its slots, and open does not
  rebuild it from Class A.
- **Reused cut slot.** A later create that reuses one of those fks replaces
  that slot when the new txid's probe reaches it.
- **Earlier probe stop.** A probe that stops earlier leaves the old slot,
  and the new txid still resolves.
- **Lost tail unlink.** A watermark just above that count unlinks the tail
  when the outputs are still there.
- **Outputs gone.** When those outputs are already gone, the seal falls
  first and the watermark file is replaced with the covered count.
- **Nearby seal copy.** A missing include file copies a nearby SEAL into
  that watermark before the check, so a seal within a million creates of
  the stem unlinks before write-behind recovery.
- **Far seal.** A seal farther ahead stays out of the inclusion file until
  tip finalize.
- **Packed head.** The packed head stays.
- **Crash between writes.** A crash between those two writes still has the
  high watermark, so the next start tries again.
- **Through marker before seal.** The write-behind advance and the Class A
  tail backfill each record `scripthash.through` before a wave can seal the
  ingest head, and again before the body sync.
- **Collect marker.** The unsorted collect records that marker before the
  first shard pack.
- **DONE above count.** A `DONE` last above the create count collects the
  covered prefix again, leaves `include_hwm` at that count, and records the
  dropped numbers in `scripthash.recheck`.
- **Open drops markers.** Open records that span and drops the `DONE`
  markers while the count is still the cut.
- **Packed extract.** A packed shard keeps its extract.
- **Unpacked pass-1 maps.** A first-pass map on a shard that is not packed
  is removed before those markers go, and again before the unsorted
  directory is removed, so the next open does not mark that map packed or
  publish its create count.
- **Directory syncs.** Dropping the markers syncs that directory, and
  removing the directory syncs the store directory.
- **Equal DONE.** A `DONE` last that still equals the create count is
  collected again when an unsealed shard has no map, and again when the
  body has no rows.
- **Fault wipe byte.** A fault rebuild writes the wipe byte before it drops
  pack marks.
- **Pack-complete wipe.** A pack-complete head with no include mark, no
  SEAL, and no resume mark does the same, then removes that directory and
  those unpacked maps, so the old map is not published onto the wiped body.
- **Wipe retry.** A crash during that wipe retries even when the next open
  restored the pack marks and the inclusion watermark.
- **Resume mark.** A resume mark keeps that pack-complete head, and startup
  stays Direct until the tail backfill publishes the watermark, so the next
  block does not publish it over creates the pack never scanned.
- **Pack floor.** The pack syncs the shard bodies and stores `include_hwm`
  at the keys-pass count, capped at a cut, before it removes the unsorted
  directory.
- **Floor is not posts end.** That floor is not the posts-pass scan end:
  the posts pass does not store a create for a script the keys pass saw
  once.
- **Later finalize.** A later finalize of that head, when the watermark is
  still missing, stores the capped `scripthash.through` and SEAL before
  that removal, and does not publish a through value above `DONE.keys` or
  raise a watermark that is already set.
- **Backfill start.** The tail backfill then starts after that floor.
- **Posts-pass through.** Before that removal, `scripthash.through` names
  the create the posts pass synced, including when the inclusion watermark
  stays at the keys-pass count or the cut.
- **Through lock.** `note_sh_through` holds one lock across the load and
  the rename, so a tail backfill and write-behind cannot replace a higher
  mark or lose the tmp file.
- **Watermark at cut.** The inclusion watermark stays at the cut until the
  tail backfill appends the new creates.
- **DONE drop.** Open drops a `DONE` marker that is still above the create
  count.
- **Keys inside, posts past.** When `DONE.keys` is inside that count and
  only `DONE.post` is past it, open keeps `DONE.keys` and the pass-1 maps.
  The next floor stays at that keys-pass count, so a one-seen script paid
  again on an already packed shard is still appended.
- **Through recheck span.** Open records `(covered, through]` in
  `scripthash.recheck` when that value is past the create count, including
  when `include_hwm` is not.
- **Repair before watermark.** That repair records the cut span before the
  watermark moves.
- **Disjoint cut.** A later cut that does not meet that span stays its own
  record.
- **Full rebuild.** A full rebuild removes the file.
- **Stale row append.** A later append stores a create at or below the live
  count when the row still ends past that count, and also when the live
  count has already passed the stale end. Creates past the live count drop
  off the row.
- **Head before free.** The new head is synced before the old slab is
  freed.
- **Reused cut history.** A reused cut number that no longer pays the
  script stays out of windowed history and does not shorten an Esplora
  page.
- **Reused cut touch.** That number also stays out of a height touch, a
  subscription status, and the join create count.
- **No recheck span.** With no recheck span, that count stays the packed
  counter. A recheck span still walks the postings for it.
- **Shard bump past file.** A shard offset past its file rebuilds once.
- **Wipe byte.** Open writes the wipe byte when that bump is past the file,
  before startup discard.
- **Resume overwritten by a torn bump.** That bump overwrites a resume byte.
- **Resume keeps shards.** A resume byte with no torn bump still keeps the
  sealed shards.
- **Overflow bump.** The overflow body is checked the same way.
- **Past-end read.** A page, slab, or extent read past the logical end
  writes the same byte on either read backend.
- **Cold overflow reset.** A cold rebuild drops sealed overflow heads,
  including L0 and L1, starts ingest empty, and syncs `scripthash.ovf/` so
  those files cannot come back after the resume byte.
- **Repair batch.** Several keys repaired in one batch flush each body and
  each home once.
- **Stale slab read.** A history read of a stale slab does not schedule that
  rebuild. Repair of that mismatch still does.
- **Matching floor.** A floor that already matches the count does not
  rebuild.
- **Fault mark.** A fault writes `scripthash.rebuild` as a wipe request.
- **Wipe then resume.** That open wipes once, then rewrites the file as a
  resume mark.
- **Resume next open.** The next open keeps sealed shards while the resume
  mark is still there, including a pack-complete head whose include mark
  and SEAL were not published yet.
- **Lost tail under resume.** A nearby lost tail still comes down under
  that mark, so the high watermark does not stick and the next finalize can
  clear the mark.
- **Fault during rebuild.** A fault mark, including one written again
  during the rebuild, wipes once more.
- **Force rebuild.** An explicit force rebuild still wipes, then stores the
  resume byte before the collect.
- **Cancel.** A cancel keeps `DONE` and any shard already packed. The next
  start without the force flag resumes from that progress.
- **Resume head write-behind.** A resume head does not queue write-behind
  copies of creates the tail backfill also packs.
