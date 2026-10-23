# SABnzbd for lemonfiber

**Downloads from Usenet.**
SABnzbd is one of the parts the stack lemonfiber ships is built from.
Without it: No Usenet downloads.

## How lemonfiber carries it

The default bundle in
[lemonfiber-plugins](https://github.com/lemonfiber/lemonfiber-plugins) pins this
repository by a revision and the digests of its manifest and its tree, and
lemonfiber compiles that tree in.
lemonfiber reaches SABnzbd through the adapter this repository holds, which
speaks lemonfiber's `download.usenet` contract in front of it: a Rust crate built
on the core's adapter kit, into the image `ghcr.io/lemonfiber/sabnzbd-adapter`,
published from every commit on `main` and from each tag the release train cuts.
lemonfiber mints the key SABnzbd answers its API to, writes it into SABnzbd's
starting `sabnzbd.ini` and into the adapter's `upstream.json`, and hands it to
nothing else.

## What is here

| Path | What it is |
| --- | --- |
| `plugin.toml` | The part's manifest |
| `config/sabnzbd/` | The files SABnzbd starts with, written into its configuration directory |
| `recordings/sabnzbd/` | The responses its proofs are held to |
| `src/` | The adapter |
| `conformance/download.usenet@1/` | The adapter's answer to each conformance case, in front of the pinned SABnzbd |
| `Dockerfile` | The adapter's image |
| `targets.toml` | The lemonfiber its checks run against, and the core the adapter is built on |
| `proofs.json` | What those checks said, committed |

## Checks

The plugin verdict is one job of spec's
[`first-party-plugin`](https://github.com/lemonfiber/spec/blob/main/.github/workflows/first-party-plugin.yml)
workflow: the manifest as the lemonfiber `targets.toml` names reads it, every
proof against its recording, and every pinned digest. The adapter is held to its
format, its lints, its tests, full line coverage and its dependency policy, its
image runs as lemonfiber runs it, and `recordings` takes every conformance case
again against the pinned SABnzbd. `just ci` runs the gates that need nothing but
this tree.

## Licence

The code and data in this repository are under the licence in `LICENSE`.
SABnzbd itself is under GPL-2.0-only and is not distributed here: this
repository names an image, it does not contain one.
