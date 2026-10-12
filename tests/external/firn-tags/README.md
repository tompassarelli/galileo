# firn tags thesis demo

`thesis_demo.rs` (PLAN 6.12, DECISIONS D-64, D-78) reads `FIRN_ROOT`, defaulting
to `~/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9`, in place.
It imports the tag facts with 5.2's importer (`galileo_import::bnix_tags`) into a
scratch repository under Cargo's test scratch directory, beside
`lib/firn/tags.gal` and a requirement
`?module is enabled on ?host requires ?module is supported on ?host`.

At the pin, `galileo build` reproduces both hosts' `_generated-enables.nix`
byte for byte and `galileo check` exits 0 with the requirement holding for all
155 instances. The test commits that as the first image, then applies the patch
to `lib/firn/tags.gal`: rule `enabled-by-pin` with `ashashi pins git` and
`whiterabbit pins git`. Git is already enabled on both hosts, so the build is
still byte-identical, and the patch is the second image.

`galileo review --base HEAD~1` must exit 1 with git's failure introduced on
both hosts, the bypass proof naming `enabled-by-pin` at `lib/firn/tags:62`, and
6.11's line that the requirement reads that rule. Nothing is written into firn
(D-19), and the test asserts no wall time; the 5 s review budget is measured on
the `full` profile build under an exclusive lease.

Run it with `nix develop -c cargo test -p galileo-cli --test thesis_demo --locked --offline`
through the repository's depbox and capacity wrapper.
