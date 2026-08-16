# Adding a host

What to do when `clean-worker`, `clean-browser`, or `clean-edge` becomes real.

The *dispatch* design is deliberately small: four edits in Rust, and the
compiler catches you if you miss one. But a host is not finished when it
compiles. It also has to be checked out by CI and bundled by the release, and
**nothing in the type system enforces those** — a host can be fully wired,
green in CI, and still break the release. Work through this document to the
end rather than stopping when `cargo build` is happy.

Read in order: the four Rust edits, then [CI](#ci), then
[Release](#release). The last is the one people skip.

`clean-cli` was the first host added this way after `clean-server`, and every
section below is a worked example of it.

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

`run_cli` shows the other half of that rule: it forwards `cli.guest_args` to
`clean_cli::entrypoint::run` rather than warning, because guest arguments are
the entire point of a CLI host — a warning that fires on correct usage is worse
than no warning. It still warns about `--assets`, which a command invocation
has no use for.

The lesson generalises: warn about a flag your host genuinely ignores, and wire
the ones it needs. Do not copy `run_server`'s warning set.

## CI

`ci.yml` checks out each sibling host repo by path, because the hosts are path
dependencies while everything is pre-1.0. Adding a host means adding a checkout
step — and, **only if that host's repo is private**, a `<HOST>_DEPLOY_KEY`
secret.

> **The `ssh-key:` lines are inert today.** Every host repo — `clean-server`,
> `clean-cli`, `clean-host-core` — is public, so `actions/checkout` reaches
> them with the workflow's default `GITHUB_TOKEN` and the empty
> `ssh-key: ${{ secrets.<HOST>_DEPLOY_KEY }}` is ignored. No such secret
> exists on this repo, and CI passes regardless. They are kept as the wiring
> a private host repo would need, not as a requirement you must satisfy now.
> If you go looking for a missing deploy key because a checkout failed, check
> first that the repo exists and is public — that is the far likelier cause.

If you *do* make a host repo private, the key is per-repo and read-only, so a
leak exposes read access to that one repo alone. Register the public half as a
deploy key on the host repo and the private half as `<HOST>_DEPLOY_KEY` here.

> **Push the host repo first.** CI checks out each sibling's **default
> branch**, not your working tree. A local `cargo build` compiles against
> whatever is on disk, so a change that spans this repo and a host repo builds
> fine locally and fails in CI with a missing item — the first push of this
> repo failed exactly that way (`cannot find entrypoint in clean_server`,
> because clean-server's commit had not landed yet). Land the host change,
> then the runtime change.

Extend the behavioral CI steps as well:

- **"both CLI shapes boot the demo component"** — add an equivalent for the new
  world, against a real component from that host's repo. `--check` exercises
  config resolution and import validation without binding ports. Go further
  where `--check` does not reach the host's actual contract: the `cli` steps run
  the guest and diff its stdout, because a CLI host's contract *is* its stdout
  and exit code.
- **"unimplemented worlds fail distinguishably"** — this uses `worker` as its
  example. Swap it for another unimplemented world when `clean-worker` lands,
  or drop the step once every world is implemented.
- If the host reads flags the others ignore, add a step proving they arrive —
  see "guest arguments reach the cli host rather than the runtime".

## Release

**A release bundles its hosts, so it checks out every host repo too.** This is
the coupling that is invisible from the code, and it is the one that bites: a
host wired in `world.rs` and green in CI will still fail the release if
`release.yml` has no checkout step for it. `release.yml` repeats the sibling
checkout block from `ci.yml` — the same repos, the same reasoning — and adds
two steps that read from those checkouts. Today it bundles `clean-server` and
`clean-cli` (plus `clean-host-core`, which both hosts depend on but which is
not itself a host and contributes no `host.wit`).

Three edits in `release.yml`:

- A **checkout step** for the new host repo, mirroring the one in `ci.yml`. The
  two blocks must stay in step; a host present in one and absent from the other
  is a release that fails after CI already passed.
- The **"record the bundled host versions"** step lists each host's version and
  commit in `BUNDLED-HOSTS.txt`. Add a line for the new host — after the split,
  this file is the only record of what is inside a given binary.
- The **package** step copies each host's `host.wit` as `<host>.host.wit` for
  Manager's `~/.cln/host-wit/` cache. Add a `cp` for the new one. Every bundled
  host contributes exactly one `host.wit`, collected into the release archive:
  that is how Manager learns a host's interface without building it.

### `BUNDLED-HOSTS.txt`

Generated during the release from the sibling checkouts, one line per host
giving its version and short commit SHA, and shipped inside every archive. Once
the binary is linked, the hosts inside it are no longer separately identifiable
— this file is the only record of what a given runtime build actually
contained. Reach for it when a bug reproduces on one runtime version and not
another and you need to know which host commits differed. See
[versioning.md](versioning.md) for how it stands in for a version matrix while
more than one host ships.

## Versioning, when your host lands

**Settled — you do not need to decide this.** `clean-cli` made two hosts and
forced the question; it was resolved at the `0.7.0` release in favour of the
runtime versioning **independently** of the hosts inside it. See
[versioning.md](versioning.md).

For you that means: adding a host does **not** change how the runtime is
numbered, and you should not try to make the runtime version reflect your
host's. Bump the runtime because the runtime shipped. Your host's version and
commit are recorded in `BUNDLED-HOSTS.txt`, which is where anyone should look
to find out what a given build contained.
