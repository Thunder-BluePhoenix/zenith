/// Phase 13: Build system & reproducibility engine.
///
/// - `derivation` — content-addressed build identity (Nix-style)
/// - `store`      — local content-addressable output store
pub mod derivation;
pub mod store;
