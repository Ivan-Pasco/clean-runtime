# clean-runtime

The binary Clean Manager installs as `runtime` and `cln run` invokes
(Manager §00.13).

A single binary containing every host implementation, which picks one based on
the artifact's declared world. `cln run myapp.clapp` reads the app's world,
invokes this binary with `--world=<world>`, and the user never learns the host
taxonomy exists.

`clean-runtime` is the name of this **binary**, not of any host. Each host
keeps its own canonical name and publishes its own `host.wit` (HCV-02).
`clean-runtime` never appears in a WIT contract, a `[target] host` value, or a
host registry key — `crates/clean-runtime/src/world.rs` has a test asserting
this.

## Worlds

| World | Host | Repository | Status |
|---|---|---|---|
| `server` | `clean-server` | [clean-server](../clean-server/) | implemented |
| `cli` | `clean-cli` | — | specified, not implemented |
| `worker` | `clean-worker` | — | specified, not implemented |
| `browser` | `clean-browser` | — | specified, not implemented |
| `edge` | `clean-edge` | — | specified, not implemented |

Unimplemented worlds parse and then fail with a message naming the gap,
deliberately distinct from the unknown-world error for a typo — one is a
missing feature, the other a spelling mistake, and they need different fixes.

Adding a host: see [docs/adding-a-host.md](docs/adding-a-host.md).

## The CLI: two accepted shapes

```bash
# Manager §00.13 step 5 — what `cln run` generates.
clean-runtime --world=<world> <wasm> --config=<host.toml> [--assets=<dir>] -- [args]

# What clean-server has always taken, and what Clean Cloud's node agent
# generates in production today.
clean-runtime <host.toml>
```

`--world` defaults to `server`, which is what makes the bare positional form
unambiguous. Supporting both was deliberate: Manager's `cln run` was written
against the spec while Cloud runs the positional form in production, so each
side can adopt this binary without waiting for the other.

**How the positional argument is read.** Its extension decides: a `.wasm` is
the component (Manager's shape), anything else is the config (Cloud's shape).
Two cases are errors rather than guesses, because booting against the wrong
file is worse than refusing:

| Invocation | Result |
|---|---|
| `clean-runtime app.wasm` | error — a component with no configuration (CLNH-13) |
| `clean-runtime a.toml --config b.toml` | error — two configuration paths |

**Flags the server host ignores.** `--assets` and post-`--` guest arguments are
accepted for Manager's shape but warn: `clean-server` reads its asset root
from `[assets] root` in `host.toml` and does not forward argv to the guest.
Accepting them silently would be worse than saying so. Both will matter for
`clean-cli`.

## Building

The hosts are sibling path dependencies while everything is pre-1.0 — the same
convention `clean-server` uses for `clean-host-core`. Lay the repos out as
siblings:

```
Dev/Clean Language/
├── clean-runtime/     # this repo
├── clean-server/
└── clean-host-core/
```

```bash
cargo build --release --workspace
cargo test --workspace

# Format this package only. `cargo fmt --all` follows the path dependencies
# into the sibling repos, where drift is not this repo's to fix.
cargo fmt -p clean-runtime -- --check
```

Verify against the real demo component in `clean-server`:

```bash
demo=../clean-server/testing/demo-site
./target/release/clean-runtime --world=server "$demo/site.wasm" --config="$demo/host.toml"
curl -i http://127.0.0.1:3100/
```

## Versioning and releases

`clean-runtime --version` reports `clean-runtime <version>`. Clean Cloud's node
agent probes exactly that to satisfy each package's `build.runtime_version`
pin, so the number is a contract with every already-built `.clapp` — read
[docs/versioning.md](docs/versioning.md) before bumping it.

Releases fire on `v*` tags and publish per platform:

```
clean-runtime-<version>-<os>-<arch>.tar.gz
clean-runtime-<version>-<os>-<arch>.tar.gz.sha256
```

Platforms: `macos-arm64`, `macos-x86_64`, `linux-x86_64`, `linux-arm64`.
Windows lands at M1 with the reload-socket work.

Each archive carries the binary, every bundled host's `host.wit` (named
`<host>.host.wit`, for Manager's `~/.cln/host-wit/` cache), and
`BUNDLED-HOSTS.txt` recording the exact host versions and commits inside — the
build is the only place that information exists once the binary is linked.

The release workflow **fails** if the binary's reported version does not match
the tag: a mismatch would silently break every deploy pinned to that release.
