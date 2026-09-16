# Vqro Collections extension

First-party Collections package for Vqro. `src/projection.rs` is the real projection implementation used by the bundled compatibility provider; it speaks the public `host.terminal-groups.v1` / extension-service v1 contracts in the host build. The installable manifest also exposes collection workflows without taking the reserved `vqro.collections` host-document namespace.

Vqro ships this extension installed and enabled by default. It can be removed from **Extensions → Installed**; removal retains user configuration and state.
