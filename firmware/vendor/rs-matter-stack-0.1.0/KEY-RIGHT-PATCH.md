# Pinned Matter capacity profile

This is the published `rs-matter-stack` 0.1.0 crate, with two additional capacity
features and checks for the profile used by Key Right. Dependency versions and
the transport implementation are unchanged.

- Published crate SHA-256:
  `8c0923532d0b4c2dba8b525e94db8cf49478ed2112bd668acafd8c354cd050bf`
- Upstream repository: <https://github.com/sysgrok/rs-matter-stack>
- Published source commit: `35cf0afbf9d6b5e09ff3724066710195e4c4ac35`
- Cargo cache markers and the published package's own lockfile are omitted.
- The published MIT and Apache-2.0 licenses are retained.

The added `max-subscriptions-15` and `max-im-buffers-20` features fill gaps in the
upstream capacity choices. The application also selects the existing
`max-responders-2` feature. Fifteen slots cover the pinned Matter implementation's
five fabrics and advertised three subscriptions per fabric. Twenty IM buffers
cover those retained subscription requests, two concurrent request RX/TX pairs,
and the sequential publisher's TX buffer. Publishing status responses and Busy
responses use the transport exchange directly, without another IM buffer.

The earlier 16-subscription/32-buffer profile consumed about 35 KiB of main-stack
headroom and faulted during live startup with 22,080 bytes reserved. The captured
panic does not establish stack overflow as its cause. This profile restores
headroom without shrinking the 100 KiB radio heap or 32 KiB transport arena.
Actual boot, controller traffic, and repeated transport restart remain required
to establish runtime behavior; the linked stack check alone is insufficient.

Only `Cargo.toml`, `Cargo.toml.orig`, and `src/lib.rs` change from the published
crate; `src/capacity_tests.rs` and this record are added. A constant assertion
checks advertised minima and transient buffer headroom after Cargo feature
unification on every target build. The unit test holds all promised subscription
buffers while allocating both request pairs and a publisher, then checks reuse.
The normal firmware gate runs the same test through
`firmware/app/host-tests/src/matter_capacity_tests.rs` using the existing host
Matter dependency; no additional host SDK dependency graph is required.
