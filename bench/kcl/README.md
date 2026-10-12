Run from the repository root using the inherited bench shell:

```sh
mkdir -p bench/kcl/out
nix develop ./bench -c kcl run bench/kcl/main.k bench/schema/kcl/types.k bench/kcl/modules/main.k --format json -D host=whiterabbit -o bench/kcl/out/whiterabbit.json
nix develop ./bench -c kcl run bench/kcl/main.k bench/schema/kcl/types.k bench/kcl/modules/main.k --format json -D host=ashashi -o bench/kcl/out/ashashi.json
```

First run `bench/contract/make-reference.sh` without arguments to generate the ignored references and private settings.

KCL compiles the three files as one package. The schema catalog is included unchanged, and each module receives its own typed option records. The Zen package expression checks both symbolic reference ingredients against `Package` before constructing its nested call.

`main.k` reads private settings natively from `bench/contract/out/private.json` and the shared `bench/context.json`, resolves the complete host selections, applies the platform gate and tag override, and checks input URL uniqueness. `Realization` expresses the required-software rule; the actual responsibility facts and remaining five module bodies belong to AF.15.

The part-one export retains full sample host settings and emits only firefox, zen-browser, kanata, users and lane-gc bodies. Four roots retain their inline import lists, while lane-gc keeps its direct body. No reasons or reference exports are implementation data.
