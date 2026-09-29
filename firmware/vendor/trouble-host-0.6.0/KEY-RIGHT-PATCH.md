# Pinned Trouble GAP restart fix

This directory contains the published `trouble-host` 0.6.0 crate, with one source
patch in `src/gap.rs`. The crate version and dependency requirements are unchanged.
The application and its host tests select this directory through Cargo's
`[patch.crates-io]` table.

- Published crate SHA-256:
  `1df7817cead4b83dfbeeaa59736ecbc97c30b21dc3bfc0cc2ce8a6687f70b37a`
- Upstream repository: <https://github.com/embassy-rs/trouble>
- Published source commit: `29b0c515831e4be2bba5e55616bfa6297d75db31`
- Source directory at that commit: `host`
- Cargo cache markers and the published package's own lockfile are omitted.
- Upstream MIT and Apache-2.0 license files are retained in this directory.

Both GAP configuration builders previously initialized a process-global
`StaticCell` for the device name on every construction. Recreating the Matter
transport therefore panicked when its GATT server was constructed a second time.
The first board reproduced this on firmware 0.1.1 after `test network`.

The patched builders borrow the immutable name into the existing read-only
attribute. `AttributeTable<'a>` already constrains the name's lifetime, so this
needs no global storage, allocation, unsafe code, or radio-lifecycle change.
The original 22-byte limit and read-only characteristic remain intact. Both
peripheral and central construction can repeat, and separate tables retain
separate names.

`firmware/app/host-tests/src/commissioning_ble_tests.rs` exercises repeated
construction, simultaneous distinct names, read-only access, and length checking.
The repeated-build tests fail with the original published crate. Run the regular
`scripts/check-firmware.sh` gate to exercise the patched dependency on both host
and ESP32-C3 targets.
