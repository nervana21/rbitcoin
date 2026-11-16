//! Output classification for quantum-vulnerable vs quantum-safe.

/// Quantum-vulnerable vs quantum-safe output.
///
/// Not Bitcoin Core "legacy" (non-segwit). Here `Vulnerable` means pre-QS
/// output types treated as stealable after q-day.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputKind {
    /// Pre-qday EC / older script types treated as vulnerable post-qday.
    Vulnerable,
    /// Quantum-safe output type (concrete scheme TBD).
    QuantumSafe,
}
