# Adding a host

What to do when `clean-cli`, `clean-worker`, `clean-browser`, or `clean-edge`
becomes real. The dispatch design is deliberately small: four edits, and the
compiler catches you if you miss one.

## The contract a host must offer

`clean-runtime` does not implement host behavior — it selects one. A host crate
must expose a single entry point that owns its own startup, signal handling,
and shutdown:

```rust
pub fn run(config_path: &Path, check: bool) -> ExitCode
```

`clean-server` does this as `clean_server::entrypoint::run`. That function
deliberately lives in the library, not in the host's `main.rs`, so the host's
own binary and `clean-runtime` call identical code and cannot drift in boot or
drain behavior. Follow the same pattern: the host's `main.rs` should be
argument parsing that delegates.

`check` means "validate configuration and the guest's imports, then exit
without binding anything" — the `--check` flag. Every host needs it; it is what
`cln` and CI use to validate a bundle without running it.

## The four edits

1. **Workspace `Cargo.toml`** — add the host as a path dependency next to
   `clean-server`, and add it to `crates/clean-runtime/Cargo.toml`.

2. **`world.rs` → `is_implemented`** — add the variant to the `matches!` arm.

3. **`main.rs` → the dispatch `match`** — add an arm calling your `run_*`
   helper. This `match` is exhaustive over `World`, so the compiler will not
   let you add a world without wiring it. That is on purpose; do not add a
   catch-all.

4. **`main.rs` → a `run_*` function** — delegate to the host's entry point.
   This is also where you handle flags the host does not support (see below).

Then update the world table in [../README.md](../README.md).

## Flags, and being honest about them

`clean-runtime` accepts Manager's full CLI shape for every world, but not every
host reads every flag. `run_server` warns that `--assets` and post-`--` guest
arguments are ignored, because `clean-server` takes its asset root from
`host.toml` and does not forward argv.

For `clean-cli` these become load-bearing: guest arguments after `--` are the
entire point of a CLI host, and it will need to forward them rather than warn.
When you wire it, remove the warning and pass `cli.guest_args` through — a
warning that fires on correct usage is worse than no warning.

## CI

`ci.yml` checks out each sibling host repo by path, using a per-repo read-only
deploy key. Adding a host means adding a checkout step and a
`<HOST>_DEPLOY_KEY` secret.

> **Push the host repo first.** CI checks out each sibling's **default
> branch**, not your working tree. A local `cargo build` compiles against
> whatever is on disk, so a change that spans this repo and a host repo builds
> fine locally and fails in CI with a missing item — the first push of this
> repo failed exactly that way (`cannot find entrypoint in clean_server`,
> because clean-server's commit had not landed yet). Land the host change,
> then the runtime change.

Extend the two behavioral CI steps as well:

- **"both CLI shapes boot the demo component"** — add an equivalent for the new
  world, against a real component from that host's repo. `--check` is enough;
  it exercises config resolution and import validation without binding ports.
- **"unimplemented worlds fail distinguishably"** — swap the example world for
  one that is still unimplemented, or drop the step once every world is
  implemented.

## Release

Two edits in `release.yml`:

- The **"record the bundled host versions"** step lists each host's version and
  commit in `BUNDLED-HOSTS.txt`. Add a line for the new host — after the split,
  this file is the only record of what is inside a given binary.
- The **package** step copies each host's `host.wit` as `<host>.host.wit` for
  Manager's `~/.cln/host-wit/` cache. Add a `cp` for the new one.

## Before the first release that bundles two hosts

Settle the versioning question in [versioning.md](versioning.md). While
`clean-server` is the only host, the runtime version tracking it is
unambiguous. With two hosts on independent cycles it stops being, and the
version is a hard contract with every already-built package — so it needs
deciding before a release, not after.
