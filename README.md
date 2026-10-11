# Galileo

Galileo is a general-purpose language in the making, built for a world where
AI writes most of the code and people approve what it does. A program is
rules, state and requirements, and the same rules that run a system also
explain it and constrain changes to it:

- **Determine:** a Galileo program is what runs. It emits the Nix that NixOS
  evaluates, and it is the state machine north's host drives.
- **Explain:** `why` and `why not` answer from the same evaluation, in
  sentences that point at source.
- **Constrain:** requirements live in the program, and every change is
  reviewed against them: what it changes, what follows, and whether it
  does what its author said.

Its first job is to replace [Clause](https://github.com/tompassarelli/clause) and [Beagle](https://github.com/tompassarelli/beagle),
first in firn (nixos-config) and then in north, keeping the best ideas from
both. Those two prove it on real work; command-line tools and game logic
come after.

- `SPEC.md`: the language and its acceptance scenarios.
- `PLAN.md`: the build plan and its progress.
- `DECISIONS.md`: what was decided and why.

Quickstart: build, write a module, check it, print its Nix, see the rest.

```sh
nix develop -c cargo build -p galileo-cli
cat > hello.gal <<'EOF'
nix module
  arguments: config, lib, ...
  option services.hello.enable: enable "Enable the hello service"
  option services.hello.greeting: Text
  config when config.services.hello.enable
    services.hello.greeting: "hello, world"
EOF
./target/debug/galileo check hello.gal
./target/debug/galileo build --stdout hello.gal
./target/debug/galileo help
```
