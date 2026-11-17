# rbitcoin-landmines

Experimental model of Cory Fields' [Quantum Landmines and Vulnerable Coin Auctions](https://groups.google.com/g/bitcoindev/c/ARMwWdWcz9A) (2026-11-04).

Crate: `crates/rbitcoin-landmines`.

## Non-goals

- Not default Core-parity consensus. `rbitcoin-node` must not call this on mainnet.
- Not a soft-fork / BIP. Wire covenants and activation are stubs.
- Not production post-quantum cryptography. `InsecureStubCrypto` / hash commitments are stand-ins.
- Not a wallet.

## Naming (reader map)

Types name the rule. Essay metaphors stay here as aliases.

| Type / term | Means | Fields essay alias |
|-------------|--------|--------------------|
| `ChallengeUtxo` | Bonded, timed, multi-path output under contest | "in-limbo" output |
| `ChallengeSchedule` | Landmine then auction then timeout bounds | (timeouts in the post) |
| `SpendPath` | Which path the challenge layer admits | spend conditions |
| `Phase::Timeout` | No proof required; spend as usual | after auction time-out |
| `OutputKind::Vulnerable` | Quantum-stealable pre-QS output | "legacy" in the post |
| `Landmine` | Dust QS output with hidden PoK | landmine |
| `qday` | Activation of post-quantum spend rules | q-day |

Do not use Core's "legacy" (non-segwit) for quantum-vulnerable coins.

## Phase ladder (shared spine)

Every collateralized post-qday vulnerable spend parks value in a `ChallengeUtxo`:

1. **Landmine** (`now < landmine_end`) — pre-qday PoK reclaim
2. **Auction** (`landmine_end <= now < auction_end`) — staked and/or sats bids
3. **Timeout** (`now >= auction_end`) — unrestricted spend of remaining lock

Wrong-phase paths always fail at the challenge layer. Phase time comes from `ActivationClock::now()`.

## Modes

| Mode | Pre-qday | Who wins | Vulnerable coins |
|------|----------|----------|------------------|
| Landmine | Hidden PoK in a trivial QS output | Key owner with valid reveal | Paid to recovery key with collateral |
| Staked auction | Hash-committed stake+bid; auction **register** seals reveal | Lowest valid opened bid at timeout | Burned; winner takes bid from collateral |
| Sats auction | None | Unique highest opened bid gets full challenge pot | All opened bids pay (sealed all-pay) |

Fields + Narula floated a dollar auction for sats (second-place spiral). This crate picks **sealed all-pay** instead: every opened bid pays; no second-place escalation. Fields left the best auction form open.

Open mailing-list knobs live in `SettlementPolicy` (`ValueDestination`, `StakeOutcome`). Default `losing_stake` is `Slash` (Fields listed return-or-slash; this crate leans slash so abandon/lose is costly). Defaults are explicit, not consensus law.

## Plug-in map (replace stubs later)

No other workspace crate owns a PQ/QS verify, PoK, or covenant prototype. Seams below stay in this crate until real soft-fork / PQ land. Soft-fork attachment later uses existing consensus upgrade hooks (unknown tapscript leaf, `OP_SUCCESS`, unknown pubkey length), not a drop-in landmines API elsewhere.

| Seam | Today | Wired? | Later |
|------|-------|--------|-------|
| `ActivationClock` | `StubClock` | Yes — challenge/mode time + pre-qday fund gates | Tripwire / height activation |
| `QuantumSafeCrypto` | `InsecureStubCrypto` | Reserved — verifies future QS *spends* of payout keys; not PoK reclaim (commit/reveal) | Real QS verify |
| `PreQdayPok` | `StubPok` (SHA256 preimage) | Yes — landmine fund/reclaim | Taproot-like leaf / soft-fork proof |
| `StakeCommit` | `StubStakeCommit` (SHA256) | Yes — pre-qday staked leaf commit | Same leaf family as PoK |
| `TimedCommit` | `StubTimedCommit` (hash + plaintext seal) | Yes — sats/staked seal, self-reveal, force-open after `auction_end` | Real timed / force-open scheme |
| `ConsensusHook` | `NullConsensusHook` | Reserved — name-only until experimental regtest/signet gate | Opt-in connect call site |
| `OutputKind` | `Vulnerable` / `QuantumSafe` | Yes — labels funded QS commitment outputs | Concrete QS script templates |
| `WireCovenant` | (not present) | No | Script / implicit consensus encode |
| Amounts / outpoints | `bitcoin::Amount` / `bitcoin::OutPoint` | Yes | Unchanged binding |

## Security invariants enforced in-crate

- Full collateral required to create a challenge (default 1:1).
- Phase membership before any path effect.
- Checked sat arithmetic (`AmountOverflow`).
- Landmine: commitment verify + `vulnerable_id` binding; fund only pre-qday.
- Staked: leaf commit verify, target bind, strictly lower bid, depth limit; commit only pre-qday; auction **register** seals reveal; settle refuses `BidStillSealed`; force-open only at/after `auction_end`; default slash on non-winner stakes.
- Sats: sealed register in auction; self-reveal or force-open; settle refuses sealed bids; all-pay on opened amounts; unique highest wins pot; reject `bid > pot` at register; reject tied highest at settle.

## How to test

```bash
cargo test -p rbitcoin-landmines --test suite
```

## Upstream policy

This tree is a fork of [reardencode/rbitcoin](https://github.com/reardencode/rbitcoin) for consensus-shaped experimentation. Keep landmines changes in `crates/rbitcoin-landmines` (+ this doc). Do not change default validation paths until an explicit experimental gate exists.
