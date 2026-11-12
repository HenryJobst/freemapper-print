# Update fixtures

Manifests for the tests of `freemapper-print update` (`tests/update_cli.rs`).

- Source: ported on 2026-11-12 from freemapper's fixtures (`crates/freemapper-cli/tests/fixtures/update/` in
  https://github.com/HenryJobst/freemapper), with the app name, URLs and asset names changed to freemapper-print.
- Date: 2026-11-12.
- Licence: GPL-3.0-or-later, like the repository.

| File | Content |
|---|---|
| `manifest-99.json` | Release 99.0.0 for the five platforms, `min_self_update_from` 0.1.0. The hashes match nothing. |
| `manifest-99-manual.json` | The same with `min_self_update_from` 98.0.0, so a program below 98 cannot update itself. |
| `manifest-99-noasset.json` | The same with `"assets": {}`. |
| `*.minisig` | Signatures with core's test key in `tests/keys/` (key id 09F80E494B3AE92B, secret half public on purpose). |

Regenerate a signature after changing a manifest, from `crates/freemapper-print-cli`:

```sh
cd tests/fixtures/update
for f in manifest-99.json manifest-99-manual.json manifest-99-noasset.json; do
  minisign -S -s ../../keys/test.key -m "$f" -x "$f.minisig" -t "freemapper-print test fixture" </dev/null
done
```

A signature by a key the program does not know is made at test time with a throwaway key and never committed.
