# Scaled review effort

Cards: B1, B4, C1

B1, B4 and C1 include Galileo's missed B4 and Unknown C1, so the selection
does not favour Galileo.

Use the landed AF.36b program from `bench/scaled/galileo`: 21 clones of the
ten-module sample, 210 module directories and all 297 emitted Nix files.
The original sample identities become the `_001` copies. Map B1's `atuin`
and B4's `firefox` references and option path to `atuin_001` and
`firefox_001`; C1's generic rule refactor is unchanged. Each card edits only
its frozen `rule.tag-resolution` or `rule.tag-override` unit in the local
`lib/firn/tags.gal` copy. Retain the actual four guarantees with the local
Firn rules in scope.

The one sweep extracts each clone's already-declared identity into its own
`ownModule_<clone>(): Module` definition in the ignored project-local
`lib/firn/tags.gal`, where `Module` is declared. Each clone calls its helper
in a single-row known-stage
`for ?module where ?module is a Module and ?module == ownModule_<clone>()`
around its existing `config when` body. Every clone has exactly one such
body: 126 in `default.gal`, 84 in the existing implementation file. Every
helper is actually used; the sweep adds no facts, requirements or emitted
Nix fields. The Nix emitter expands the known row in the existing field
order, and native review lists the named definition addition for every clone.

This sweep was chosen because formatting and local variable renames are
normalized away by SPEC §8.1.

The failed clone-local helper prototype is retained in ignored preparation
scratch. Galileo finding for Results: an imported type (Module) cannot be
named in a function signature outside its declaring file, native build
exits 1 with `Module has no Core IR type yet` (lower.rs ty looks up the bare
name; import stores origin/Module).

Commit this placement correction before revising the sweep. The compiler
stays unchanged, and the revised placement still requires the actual
all-297-file diff 0 and at least 210 native diff hunks for each joined card.

Commit this registration before writing any corpus patch. Keep the original,
swept and three joined base/patched source trees in ignored preparation
scratch. Build the swept corpus and compare all 297 files to the unswept
corpus with `diff -r`, then build and compare each joined patch the same way.
Use native `diff -ru` for every I packet and require at least 210 hunks.
Measure each joined native default-budget review once against five seconds;
retain the complete stdout, stderr, exit and actual wall time, including any
Unknown or timing miss.

Prepare the six packets with `compare packets --scaled`. Give each packet
one fresh cold reviewer session in its own opaque `mktemp` directory outside
all repositories, record its rollout and actual session tokens and wall time,
then clean that exact directory. Append six size-210 rows while preserving
all eighteen sample rows and packets byte for byte. Print accuracy out of
three, condition medians and their sample growth, and loss2: it trips if R
has fewer right verdicts than I or its median tokens are not below I.

The registered sweep and each joined card built successfully, and all 297
emitted files compared byte for byte with `diff -r` exit 0. The actual
implementation diff hunks are B1 233, B4 232 and C1 234. Each native review
listed all 210 helper additions as edits.

The three native default-budget reviews ran once in one exclusive window,
without warmups or retries. B1 rejected (exit 1) in 40.129586016 seconds; B4
approved (exit 0) in 39.067216850 seconds. C1 timed out at 820.92 seconds,
with no native verdict. Its elapsed time is measured from the observed
native process start to the scope's terminal timeout event at 10 ms
precision; the wrapper exited 143 and released its lease. The C1 R packet
retains exactly the native stdout and stderr printed before timeout, with
no authored timeout explanation added to the packet.

under-5 s target: FAILED

B4's approval at 210 matches the sample measurement b (`galileo`, `B4`,
`b`, exit 0 in `bench/results/review.tsv`): a consistent Galileo miss,
rather than a scale effect. The native timing failure remains a failed
target, with no Target-debt claim.
