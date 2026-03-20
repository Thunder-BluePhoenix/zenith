/// Phase 10: Cloud Runtime
///
/// Run Zenith workflows on the official Zenith cloud service.
/// Same .zenith.yml syntax — no local VMs, no infrastructure setup.
///
/// NOTE: api.zenith.run is planned infrastructure. The client code is
/// complete; once the service launches, all commands will work without
/// any code changes.
///
/// Authentication: API key stored in ~/.zenith/config.toml [cloud] section.

pub mod client;
pub mod packager;
pub mod types;
