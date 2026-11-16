Fixed

- **Power loss no longer leaves the chain naming lost transactions.** The
  connect barrier now device-syncs every archive file a block commit names
  (outputs, inputs and witness, spend slots, both locator families, txids,
  per-tx stats, and parent edges) before it writes the confirmed tip. A
  power cut could leave the tip, a strong bit, or a block's transaction
  range pointing at bytes that never reached the disk. The node then
  refused to start, rejected the next valid block, or wedged on one block
  until the store was wiped.
- **An unpublished archive tail is cut on open.** A body header that fell
  short of its locator refused startup ("body header short of locator").
  No committed row can name that tail any more, so open keeps the creates
  every body header still covers. A short `input.body` header cuts the
  input locator the same way.
- **An interrupted `tx.head` rebuild rebuilds again.** Segment files with
  no `tx.head/meta` refused startup as a legacy layout and needed a manual
  delete. Open now wipes that directory and rebuilds it from the archive.
- **Small state files are published whole.** Store `meta`, `store.secret`,
  the mempool `meta`, and the unbroadcast list are written to a synced
  temp file and renamed, with the directory synced. `store.secret` is
  owner-only before its first byte. A short mempool `meta` left by an older
  binary is moved aside like any other unreadable mempool image.
