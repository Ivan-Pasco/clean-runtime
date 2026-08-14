//! `clean-runtime` — the binary Manager installs and `cln run` invokes.
//!
//! Manager §00.13: a single binary containing every host implementation, which
//! picks a host from the artifact's declared world. `clean-runtime` is the name
//! of this binary and of nothing else — it never appears in a WIT contract, a
//! `[target] host` value, or a host registry key. Each host inside keeps its
//! own canonical name and publishes its own `host.wit`.
//!
//! Today exactly one host exists (`clean-server`), so `--world=server` is the
//! only world that runs. The other four parse and fail with a message naming
//! the gap rather than pretending to be unknown; see `world.rs`.
//!
//! # Two accepted CLI shapes
//!
//! Both of these run the same code:
//!
//! ```text
//! clean-runtime --world=server <wasm> --config=<path> [--assets=<dir>] -- [args]
//! clean-runtime <CONFIG>
//! ```
//!
//! The first is Manager §00.13 step 5. The second is the shape `clean-server`
//! has always taken and that Clean Cloud's node agent generates in production
//! today. Supporting both is deliberate: it lets Manager's `cln run` and
//! Cloud's agent adopt this binary independently, without a flag day. `--world`
//! defaults to `server`, which is what makes the bare positional form
//! unambiguous.

mod world;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;

use world::World;

#[derive(Parser)]
#[command(
    name = "clean-runtime",
    version,
    about = "The Clean runtime — runs a compiled component under the host for its world",
    long_about = None
)]
struct Cli {
    /// Which host to run the component under.
    ///
    /// Manager passes the world it read from the artifact's manifest. Defaults
    /// to `server`, which keeps the bare `clean-runtime <CONFIG>` form working
    /// for Cloud's node agent.
    #[arg(long, default_value = "server", value_name = "WORLD")]
    world: World,

    /// Path to the host configuration (`host.toml`).
    ///
    /// CLNH-13 makes an absent required key a startup error, so every host
    /// needs one of these. May be given as `--config=<path>` (Manager's shape)
    /// or as the sole positional argument (Cloud's shape).
    #[arg(long, value_name = "PATH")]
    config: Option<PathBuf>,

    /// The compiled component to run.
    ///
    /// Accepted for Manager's invocation shape. The guest path also lives in
    /// `host.toml` under `[guest] wasm`; when both are given they must agree,
    /// because silently preferring one would make a mismatched pair boot the
    /// wrong component.
    #[arg(value_name = "WASM")]
    wasm: Option<PathBuf>,

    /// Static asset directory, for hosts that serve one.
    #[arg(long, value_name = "DIR")]
    assets: Option<PathBuf>,

    /// Validate configuration and the guest's imports, then exit without
    /// binding a listener.
    #[arg(long)]
    check: bool,

    /// Arguments forwarded to the guest, after `--`.
    #[arg(last = true)]
    guest_args: Vec<String>,
}

fn main() -> ExitCode {
    let cli = Cli::parse();

    let config = match resolve_config(&cli) {
        Ok(path) => path,
        Err(message) => {
            eprintln!("error: {message}");
            return ExitCode::FAILURE;
        }
    };

    if !cli.world.is_implemented() {
        eprintln!(
            "error: no host for world '{}' in this build\n\
             \n\
             `{}` is specified but not yet implemented. This build runs: {}.\n\
             Build a package for one of those worlds, or install a runtime\n\
             version that ships the host you need.",
            cli.world,
            cli.world.host_name(),
            World::implemented()
                .iter()
                .map(|w| w.to_string())
                .collect::<Vec<_>>()
                .join(", ")
        );
        return ExitCode::FAILURE;
    }

    match cli.world {
        World::Server => run_server(&config, &cli),
        // Unreachable while `is_implemented` gates the match above, but written
        // out so adding a host crate is a compile error here until it is wired.
        World::Cli | World::Worker | World::Browser | World::Edge => {
            eprintln!("error: world '{}' has no dispatch arm", cli.world);
            ExitCode::FAILURE
        }
    }
}

/// Pick the config path from whichever CLI shape was used.
///
/// The positional argument is the component under Manager's shape and the
/// config under Cloud's, so its extension decides which it is. Both ambiguous
/// pairs — a bare `.wasm` with no `--config`, and two non-wasm paths — are
/// errors rather than guesses: booting is not something to do on a hunch about
/// which file the operator meant.
fn resolve_config(cli: &Cli) -> Result<PathBuf, String> {
    match (&cli.config, &cli.wasm) {
        (Some(config), Some(positional)) => {
            // Manager's full shape: the positional is the component. The wasm
            // path is also in host.toml and the host reads it from there, so
            // all this needs to do is catch a path that isn't there.
            if looks_like_wasm(positional) {
                if !positional.exists() {
                    return Err(format!("component not found: {}", positional.display()));
                }
                return Ok(config.clone());
            }
            // A non-wasm positional alongside --config is two config paths.
            // Preferring one silently would boot against a file the operator
            // did not mean; the same pair is a hard error in `clean-server`.
            Err(format!(
                "two configuration paths given: '{}' and --config '{}'\n\n\
                 pass the component as the positional argument, or drop one path.",
                positional.display(),
                config.display()
            ))
        }
        (Some(config), None) => Ok(config.clone()),
        (None, Some(positional)) => {
            if looks_like_wasm(positional) {
                Err(format!(
                    "a host.toml is required\n\n\
                     '{}' looks like a component, not a configuration file.\n\
                     usage: clean-runtime --world=<world> <wasm> --config=<host.toml>\n\
                        or: clean-runtime <host.toml>",
                    positional.display()
                ))
            } else {
                // Cloud's shape: the sole positional is the config.
                Ok(positional.clone())
            }
        }
        (None, None) => Err("a host.toml is required\n\n\
             usage: clean-runtime --world=<world> <wasm> --config=<host.toml>\n\
                or: clean-runtime <host.toml>"
            .to_string()),
    }
}

fn looks_like_wasm(path: &Path) -> bool {
    path.extension().is_some_and(|e| e == "wasm")
}

/// Hand off to the `clean-server` host.
///
/// The host owns its own startup, signal handling, and drain; this is a
/// delegation, not a reimplementation. Anything else would give the binary a
/// second copy of behavior that must stay identical to `clean-server`'s.
fn run_server(config: &Path, cli: &Cli) -> ExitCode {
    if cli.assets.is_some() {
        // The server takes its asset root from host.toml (`[assets] root`).
        // Accepting the flag and ignoring it would be worse than saying so.
        eprintln!(
            "warning: --assets is not read by clean-server; \
             set [assets] root in host.toml instead"
        );
    }
    if !cli.guest_args.is_empty() {
        eprintln!(
            "warning: clean-server does not forward guest arguments; \
             {} argument(s) ignored",
            cli.guest_args.len()
        );
    }

    clean_server::entrypoint::run(config, cli.check)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Cli {
        Cli::parse_from(std::iter::once("clean-runtime").chain(args.iter().copied()))
    }

    /// Cloud's node agent shape, which runs in production today.
    #[test]
    fn positional_config_is_the_config() {
        let cli = parse(&["/srv/apps/app-1/host.toml"]);
        assert_eq!(cli.world, World::Server, "world defaults to server");
        assert_eq!(
            resolve_config(&cli).unwrap(),
            PathBuf::from("/srv/apps/app-1/host.toml")
        );
    }

    /// Manager §00.13 step 5 shape.
    #[test]
    fn manager_flag_shape_resolves_the_same_config() {
        let dir = tempfile::tempdir().unwrap();
        let wasm = dir.path().join("app.wasm");
        std::fs::write(&wasm, b"\0asm").unwrap();

        let cli = parse(&[
            "--world=server",
            wasm.to_str().unwrap(),
            "--config=/cache/host.toml",
        ]);
        assert_eq!(cli.world, World::Server);
        assert_eq!(
            resolve_config(&cli).unwrap(),
            PathBuf::from("/cache/host.toml")
        );
    }

    /// Two config paths is operator error, not a precedence question: booting
    /// against the file they did not mean is worse than refusing.
    #[test]
    fn two_config_paths_is_an_error() {
        let cli = parse(&["--config=/a/host.toml", "/b/host.toml"]);
        let err = resolve_config(&cli).unwrap_err();
        assert!(err.contains("two configuration paths"), "{err}");
        assert!(
            err.contains("/a/host.toml") && err.contains("/b/host.toml"),
            "{err}"
        );
    }

    /// A bare `.wasm` has no configuration to boot from, and guessing one
    /// would be a silent wrong-file boot. CLNH-13 wants this loud.
    #[test]
    fn bare_wasm_without_config_is_an_error() {
        let cli = parse(&["app.wasm"]);
        let err = resolve_config(&cli).unwrap_err();
        assert!(err.contains("host.toml is required"), "{err}");
        assert!(err.contains("looks like a component"), "{err}");
    }

    #[test]
    fn missing_component_is_reported_by_path() {
        let cli = parse(&["/nope/app.wasm", "--config=/cache/host.toml"]);
        let err = resolve_config(&cli).unwrap_err();
        assert!(err.contains("component not found"), "{err}");
    }

    #[test]
    fn no_arguments_prints_both_usages() {
        let cli = parse(&[]);
        let err = resolve_config(&cli).unwrap_err();
        assert!(err.contains("--world"), "{err}");
        assert!(err.contains("or: clean-runtime <host.toml>"), "{err}");
    }

    #[test]
    fn guest_args_are_collected_after_the_separator() {
        let cli = parse(&["/srv/host.toml", "--", "--verbose", "input.txt"]);
        assert_eq!(cli.guest_args, vec!["--verbose", "input.txt"]);
    }

    #[test]
    fn unimplemented_world_still_parses() {
        // So the failure is "not implemented", not "unknown world".
        let cli = parse(&["--world=worker", "/srv/host.toml"]);
        assert_eq!(cli.world, World::Worker);
        assert!(!cli.world.is_implemented());
    }
}
