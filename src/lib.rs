/// Zenith shared library — exposes all core modules so both the `zenith` CLI
/// and the `zenith-agent` binary can use the same runner, config, and sandbox code.

pub mod build;
pub mod cli;
pub mod cloud;
pub mod config;
pub mod plugin;
pub mod remote;
pub mod runner;
pub mod sandbox;
pub mod toolchain;
pub mod tools;
pub mod tui;
pub mod ui;

#[cfg(test)]
mod tests;
