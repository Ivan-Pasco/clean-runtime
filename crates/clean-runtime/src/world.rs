//! The world → host dispatch table (Manager §00.13).
//!
//! `clean-runtime` is the name of a *binary*, not of a host. Each world names
//! a host that keeps its own canonical name and its own `host.wit`; this
//! module is the only place that maps between the two.

use std::fmt;
use std::str::FromStr;

/// The worlds Manager may ask for via `--world`.
///
/// Every variant is listed here, including the ones no host implements yet, so
/// `--world=worker` fails with "not implemented in this build" rather than
/// "unknown world". The two are different problems for whoever is reading the
/// error: one is a missing feature, the other a typo.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum World {
    Server,
    Cli,
    Worker,
    Browser,
    Edge,
}

impl World {
    /// The canonical host name for this world, as used in `[target] host`,
    /// host registry keys, and `host.wit` publication.
    pub fn host_name(self) -> &'static str {
        match self {
            World::Server => "clean-server",
            World::Cli => "clean-cli",
            World::Worker => "clean-worker",
            World::Browser => "clean-browser",
            World::Edge => "clean-edge",
        }
    }

    /// Whether this build contains a host for the world.
    ///
    /// Only `clean-server` exists today. This is a plain match rather than a
    /// registry because the compiler should force an update here when a host
    /// crate is added.
    ///
    /// Adding a host means: a dependency in the workspace `Cargo.toml`, an arm
    /// here, an arm in `main.rs`'s dispatch, and a `run_*` delegation. The
    /// `match` in `main.rs` is exhaustive, so the compiler will not let a new
    /// world be added without wiring it. See `docs/adding-a-host.md`.
    pub fn is_implemented(self) -> bool {
        matches!(self, World::Server)
    }

    /// Every world, for help text and error messages.
    pub fn all() -> &'static [World] {
        &[
            World::Server,
            World::Cli,
            World::Worker,
            World::Browser,
            World::Edge,
        ]
    }

    /// The worlds this binary can actually run.
    pub fn implemented() -> Vec<World> {
        World::all()
            .iter()
            .copied()
            .filter(|w| w.is_implemented())
            .collect()
    }
}

impl fmt::Display for World {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            World::Server => "server",
            World::Cli => "cli",
            World::Worker => "worker",
            World::Browser => "browser",
            World::Edge => "edge",
        })
    }
}

#[derive(Debug)]
pub struct ParseWorldError(String);

impl fmt::Display for ParseWorldError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let known = World::all()
            .iter()
            .map(|w| w.to_string())
            .collect::<Vec<_>>()
            .join(", ");
        write!(f, "unknown world '{}' (expected one of: {known})", self.0)
    }
}

impl std::error::Error for ParseWorldError {}

impl FromStr for World {
    type Err = ParseWorldError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "server" => Ok(World::Server),
            "cli" => Ok(World::Cli),
            "worker" => Ok(World::Worker),
            "browser" => Ok(World::Browser),
            "edge" => Ok(World::Edge),
            other => Err(ParseWorldError(other.to_string())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_world_parses_and_round_trips() {
        for w in World::all() {
            assert_eq!(World::from_str(&w.to_string()).unwrap(), *w);
        }
    }

    #[test]
    fn unknown_world_lists_the_known_ones() {
        let err = World::from_str("srever").unwrap_err().to_string();
        assert!(err.contains("srever"), "names the bad input: {err}");
        assert!(err.contains("server"), "lists valid worlds: {err}");
    }

    /// Guards the §00.13 rule that `clean-runtime` is a binary name, never a
    /// host name. If a world ever reports it, a `[target] host` value could be
    /// written as `clean-runtime` and the host taxonomy would blur.
    #[test]
    fn no_world_reports_the_binary_as_its_host() {
        for w in World::all() {
            assert_ne!(w.host_name(), "clean-runtime");
        }
    }

    #[test]
    fn only_server_is_implemented_today() {
        assert_eq!(World::implemented(), vec![World::Server]);
    }
}
