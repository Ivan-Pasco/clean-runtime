# Versioning

## What the number means to other components

`clean-runtime --version` prints `clean-runtime <version>`. Two things depend
on that string:

- **Clean Cloud's node agent** probes it (`probe_runtime_version` takes the
  last whitespace-separated token) and refuses to deploy a package whose
  pinned runtime no node reports.
- **Every `.clapp` and `.serve`** records `build.runtime_version` in its
  manifest. Manager §00.13 makes that an exact pin: `cln run` requires that
  version to be installed.

So the version is a contract with artifacts that already exist. Changing it
does not just label a release — it decides which already-built packages can
still be deployed.

## Why it started at 0.7.0

Tracking `clean-server`'s version, rather than starting a fresh 1.0.0 line as
the spec's example paths suggest (`~/.cln/versions/runtime/1.0.0/`).

Every `.clapp` built so far pins `runtime_version = "0.7.0"`, because
`clean-server 0.7.0` was the binary that ran them. Starting at 1.0.0 would
have made every one of those packages undeployable the day Cloud began
enforcing pins. The pre-1.0 number is also honest: four of the five specified
hosts are unimplemented.

## The open question: how to version a fat binary of independent hosts

While `clean-server` was the only host and lived in the same workspace, the two
versions could not diverge. After the repo split they can, and when
`clean-cli` and `clean-worker` land there will be three hosts on independent
release cycles inside one binary.

That makes "what does `clean-runtime 0.8.0` mean?" a real question, and it is
**not yet decided**. The options, with the tradeoff that matters:

1. **Runtime versions independently; `BUNDLED-HOSTS.txt` records the
   contents.** Simple to release — bump when the runtime ships, regardless of
   which host changed. The cost: `clean-runtime 0.8.0` does not tell you which
   `clean-server` is inside without reading the artifact, and a host bugfix
   needs a runtime release to reach users.

2. **Runtime version tracks the highest-versioned host.** Keeps a legible
   relationship to `clean-server` while it is the dominant host. Breaks down
   as soon as two hosts move at different speeds — which is the point of
   splitting the repos.

3. **A published version matrix** (Manager §00.7 already describes one for
   compiler/framework/libraries). Most honest and most machinery: the matrix
   answers "runtime 0.8.0 contains server 0.7.2, cli 0.1.0" for tooling rather
   than for humans reading a filename.

**Recommendation when this is decided:** (1) plus the matrix from (3) once
more than one host ships, treating `BUNDLED-HOSTS.txt` as the interim matrix.
(2) is a trap — it reads as informative and stops being true exactly when the
system gets complicated.

Until it is settled, the operating rule is narrow and safe:

> The runtime version tracks `clean-server`'s while `clean-server` is the only
> implemented host. The first release that bundles a second host must resolve
> this question first.

## Rules that hold regardless

- **The tag and the binary must agree.** The release workflow fails the build
  otherwise. A silent mismatch breaks every deploy pinned to that release.
- **Never reuse or retract a published version.** Packages pin exactly; a
  version that changes meaning is a package that boots a different runtime than
  it was built against.
- **`BUNDLED-HOSTS.txt` ships in every archive.** Once the binary is linked,
  which host commits went into it is not recoverable from the binary itself.
