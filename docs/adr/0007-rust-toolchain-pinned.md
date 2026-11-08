# 0007. The Rust toolchain is pinned in rust-toolchain.toml

Date: 2026-11-08
Status: Accepted

## Context

In freemapper, CI installed a newer stable Rust than the local one, and a new clippy lint failed every push
(its ADR 0018).

## Decision

`rust-toolchain.toml` pins the same channel as freemapper (1.99.1) with `rustfmt` and `clippy`. Local builds,
CI and the release workflow use it. The pin moves by hand, together with fixing what the newer clippy reports,
and preferably together with freemapper.

## Consequences

- Local and CI lints agree, and the two projects build with the same compiler.
- Cost if wrong: the project stays on an older compiler until someone moves the pin.

Source: freemapper ADR 0018.
