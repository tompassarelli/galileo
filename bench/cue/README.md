Run `bench/cue/check.sh` from the repository root to generate both native host exports, validate them and compare the complete outputs with the pinned references.

The nearest native why command is CUE evaluation of the rule inputs and computed fields. After the check, inspect Firefox's tag contributions, active result and override with:

```sh
nix develop ./bench -c cue eval bench/schema/cue/types.cue bench/cue/main.cue bench/cue/hosts.cue bench/cue/modules.cue bench/cue/rules.cue --with-context -l 'strings.TrimSuffix(path.Base(filename), ".json")' bench/context.json bench/contract/out/private.json -t host=whiterabbit -e _contributions.firefox -e _active.firefox -e _overrides.firefox
```

Use `_realization` for the responsibility constraint or `_inputs` for active flake inputs. [`cue eval -e`](https://cuelang.org/docs/reference/command/cue-help-eval/) evaluates expressions within the configuration; it prints evaluated values rather than an explanation trace.
