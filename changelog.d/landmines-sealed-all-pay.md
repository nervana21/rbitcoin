Changed

- **`rbitcoin-landmines` sats mode is sealed all-pay.** Dollar-auction second-place spiral removed. Bids register sealed, self-reveal or force-open after auction end, then every opened bid pays and the unique highest wins the challenge pot.
- **Staked auctions require timed register / force-open.** Free abandon of a registered bid is refused (`BidStillSealed` until open). Default `StakeOutcome` is `Slash`; non-winner stakes are counted in settlement.
