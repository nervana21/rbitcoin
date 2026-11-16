//! Optional connect-path hook (stub). Off unless experimental gate exists.

/// Call site for future consensus integration.
///
/// Attach point: an explicit experimental regtest / signet feature flag in
/// the node connect path. Default rbitcoin validation must not invoke this
/// until that gate exists. Today only `name()` is defined so the seam stays
/// visible without touching connect.
pub trait ConsensusHook {
    /// Name for logs / gates.
    fn name(&self) -> &'static str;
}

/// No-op hook. Keeps the seam visible without touching connect.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullConsensusHook;

impl ConsensusHook for NullConsensusHook {
    fn name(&self) -> &'static str {
        "null"
    }
}
