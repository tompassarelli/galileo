# Muove

Muove is a rule language built for a world where AI writes most of the code
and people approve what it does. The same rules that run a system also
explain it and constrain changes to it:

- **Determine:** a Muove program is what runs. It emits the Nix that NixOS
  evaluates, and it is the state machine north's host drives.
- **Explain:** `why` and `why not` answer from the same evaluation, in
  sentences that point at source.
- **Constrain:** requirements live in the program, and every change is
  reviewed against them: what it changes, what follows, and whether it
  does what its author said.

It replaces [Clause](https://github.com/tompassarelli/clause) and [Beagle](https://github.com/tompassarelli/beagle),
first in firn (nixos-config) and then in north, keeping the best ideas from
both.

- `SPEC.md`: the language and its acceptance scenarios.
- `PLAN.md`: the build plan and its progress.
- `DECISIONS.md`: what was decided and why.
