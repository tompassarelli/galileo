# nix-roundtrip

Development command: `nix develop -c cargo run -p nix-roundtrip -- [--ast | --check] FILE`.
`--ast` accepts multiple files and prints each muove-nix AST. Without it, one
file is parsed and printed through muove-nix. Invalid syntax and unsupported
nodes exit with an error; no unsupported expression is preserved as raw Nix.

`--check FILE [FILE ...]` attempts every file, reports each failure's first
differing line (line 1 when parsing emits no output), and prints the identical
count over the complete input count. Any failure exits 1. Select the 50 paths
from `firn-manifest "$FIRN_ROOT"` rows beginning `bnix` and containing ` -> `.

The converter covers the node kinds in firn's pinned `modules/kanata` files.
Parentheses are represented by expression structure. String contents use
rnix's escape decoding and indentation normalization.

List layout uses Beagle's six-item and 80-character limits, measured from the
list's indentation, and puts attribute-set items on separate lines. Nested
formal lambdas retain Beagle's parentheses.

External format dependency: [rnix-parser v0.14.0](https://github.com/nix-community/rnix-parser/tree/v0.14.0),
MIT, as declared in that release's Cargo.toml. Its AST API and string
normalization API inform this independent converter; no source is copied or
adapted. Cargo resolves the published crate, including its license; distributing
the dependency requires preserving its MIT license and copyright notice.
