# Review intents

These four intents are registered before any guarantees are written.

1. **I1 — Enabled tag:** A module is enabled on a host only through a tag that the host enables, unless the host's own configuration enables it directly.
2. **I2 — Supported platform:** A module is enabled on a host only on a platform that supports it and, unless the host's own configuration enables it directly, only because that platform supports it.
3. **I3 — Tag override:** A tag override applies only to a module that its tag enables.
4. **I4 — Enabled realization:** A responsibility is realized only by an enabled module.

The cards in `cases.tsv` describe changes to these dependencies in words,
without choosing a language or an implementation. A breaking card removes
one of the dependencies even though the pin's current values stay the same.
A control card keeps all four intents.

The table columns are `id`, `kind`, `intent`, `unit`, `edit`,
`emitted_nix_change` and `reason`. `intent` names the broken intent for a
breaking card and is `none` for a control. `unit` names exactly one rule
unit present in `bench/sample.tsv`. `edit` describes the structural change;
`emitted_nix_change` describes the Nix change AF.41 applies to the pin, with
the exact value `none` when the emitted Nix is unchanged. `reason` states
which dependency is lost or why the refactor preserves the intents.

The pin is `10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9`. It selects
`terminal` on both whiterabbit and ashashi, enabling atuin; atuin supports
ashashi's darwin platform. Whiterabbit selects `browsers` and `notes`,
enabling firefox and obsidian, and its browsers selection applies the
firefox default override. Its knowledge-authoring responsibility requires
obsidian, which is realized by obsidian-module. The cards use these current
facts to keep outputs while changing their dependencies.

In the sample, whiterabbit's own configuration enables users, kanata and lane-gc directly; I1 and I2's second clause allow those enables, and I2's first clause still checks them. The sample does not state whiterabbit's responsibility assignment (hosts/whiterabbit/responsibilities.bnix lines 32 and 34), so on the sample I4 binds no host, and B5 and B6 change no derived fact in any column.

AF.41 checks each card's whiterabbit drvPath against the pin and replaces
any card that changes it. The unchanged emitted Nix described here is a
card design, not a measured drvPath result.
