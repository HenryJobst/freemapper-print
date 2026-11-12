These keys are for tests only: the test key pair of freemapper-core's `freemapper-update` crate
(`crates/freemapper-update/tests/keys/test.*`, key id 09F80E494B3AE92B), copied here because the integration tests
sign manifests with it. It has no password, and its secret key is public on purpose. A release build never accepts
it: `src/update/mod.rs` adds it to the accepted keys only in unit tests and in a debug build with the `test-stub`
feature, and refuses to compile with `FREEMAPPER_DIST=github-release` if `assets/update/minisign.pub` (the release
key, freemapper's, key id 9353842ABFFAC9FB) ever holds it.
