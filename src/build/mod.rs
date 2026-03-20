/// Phase 13: Build system & reproducibility engine.
///
/// - `derivation`    — content-addressed build identity (Nix-style)
/// - `store`         — local content-addressable output store
/// - `remote_cache`  — HTTP push/pull binary cache keyed by derivation ID
pub mod derivation;
pub mod store;
pub mod remote_cache;
