Added

- **Experimental `rbitcoin-landmines` crate.** Models Cory Fields' quantum landmines and vulnerable coin auctions (challenge phases, landmine PoK reclaim, staked and sats auctions) behind stub seams. Not part of default Core-parity validation; `rbitcoin-node` does not call it. Public spine names the rule (`ChallengeUtxo`, `SpendPath`, `Vulnerable`); Fields' "in-limbo" / "legacy" wording stays in `docs/landmines.md` as glossary aliases.
