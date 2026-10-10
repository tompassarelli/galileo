# nix-roundtrip

Development command: `nix develop -c cargo run -p nix-roundtrip -- [--ast] FILE`.
`--ast` accepts multiple files and prints each muove-nix AST. Without it, one
file is parsed and printed through muove-nix. Invalid syntax and unsupported
nodes exit with an error; no unsupported expression is preserved as raw Nix.

The converter covers the node kinds in firn's pinned `modules/kanata` files.
Parentheses are represented by expression structure. String contents use
rnix's escape decoding and indentation normalization.

External format dependency: [rnix-parser v0.14.0](https://github.com/nix-community/rnix-parser/tree/v0.14.0),
MIT, as declared in that release's Cargo.toml. Its AST API and string
normalization API inform this independent converter; no source is copied or
adapted. Cargo resolves the published crate, including its license; distributing
the dependency requires preserving its MIT license and copyright notice.
