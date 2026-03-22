//! The Furling Test — adversarial testing for axe-code.
//!
//! Seven chevrons. Seven stages. Each one locks only when every test passes.
//!
//! Chevron 1: Abydos      — the tutorial works
//! Chevron 3: The Kawoosh  — broken input doesn't crash us
//! Chevron 6: The Replicator — self-destruction is contained

#[path = "furling/chevron1_abydos.rs"]
mod chevron1_abydos;
#[path = "furling/chevron3_kawoosh.rs"]
mod chevron3_kawoosh;
#[path = "furling/chevron6_replicator.rs"]
mod chevron6_replicator;
