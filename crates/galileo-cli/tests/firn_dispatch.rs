use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

const QUESTION: &str = "all ?order, ?node, ?edge, ?executable, ?arguments, ?required where ?command is number ?order and ?command walks ?node then ?edge and ?command plans ?executable with ?arguments and ?command needs leaf ?required, sorted by ?order";

const COMPARE: &str = r#"
import { isDeepStrictEqual } from 'node:util';
const m = await import(process.argv[2]);
const rows = JSON.parse(await Bun.stdin.text());
const commands = m.commands();
let equal = 0;
const failures = [];
commands.forEach((c, i) => {
  const row = rows[i] ?? {};
  const required = 'required' in c.leaf;
  const leaf = required ? ['fixture'] : [];
  const expected = m['parse-dispatch']([c.node, c.edge, ...leaf]);
  const planned = { steps: [{ executable: row.executable, arguments: [...(row.arguments ?? []), ...leaf] }] };
  const bare = m['parse-dispatch']([c.node, c.edge]);
  if (row.node === c.node && row.edge === c.edge && row.required === required
      && isDeepStrictEqual(planned, expected) && ('message' in bare) === required) equal++;
  else failures.push({ command: c, row, expected, planned, bare });
});
console.log(JSON.stringify({ commands: commands.length, rows: rows.length, equal, failures }));
"#;

fn firn_root() -> PathBuf {
    PathBuf::from(std::env::var("FIRN_ROOT").unwrap_or_else(|_| {
        format!("{}/code/nixos-config/pins/4bcfad4cdb806dd7f66401dad5581037cb996ff0", std::env::var("HOME").unwrap())
    }))
}

fn succeed(command: &mut Command, input: &str) -> String {
    let mut child = command.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn().unwrap();
    child.stdin.take().unwrap().write_all(input.as_bytes()).unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{command:?}: {}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn every_command_plans_as_firn_clause_does() {
    let firn = firn_root();
    let mut workbench = match std::env::var("FIRN_CLAUSE_WORKBENCH") {
        Ok(path) => Command::new(path),
        Err(_) => {
            let revision = std::fs::read_to_string(firn.join("config/clause-revision")).unwrap();
            let revision = revision.trim();
            let source = PathBuf::from(succeed(
                Command::new("nix").args(["eval", "--offline", "--raw", "--expr"])
                    .arg(format!("(builtins.getFlake \"github:tompassarelli/clause/{revision}\").outPath")),
                "",
            ));
            let target = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/firn-clause").join(revision);
            let mut command = Command::new("cargo");
            command.args(["run", "--locked", "--offline", "--package", "clause-workbench"])
                .env("CARGO_BUILD_JOBS", "4")
                .arg("--manifest-path").arg(source.join("Cargo.toml"))
                .arg("--target-dir").arg(target).arg("--");
            command
        }
    };
    let scratch = std::env::temp_dir().join(format!("galileo-firn-dispatch-{}", std::process::id()));
    std::fs::create_dir_all(scratch.join("native")).unwrap();
    std::fs::create_dir_all(scratch.join("bin")).unwrap();
    std::fs::write(scratch.join("bin/firn-io.mjs"), "export const executeRuntime = () => 0;\n").unwrap();
    let module = scratch.join("native/firn-dispatcher.js");
    let script = scratch.join("compare.mjs");
    std::fs::write(&script, COMPARE).unwrap();
    succeed(workbench.arg("compile-js").arg(firn.join("native/firn.clause")).arg(&module), "");
    let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../lib/firn/dispatch.gal");
    let plans = succeed(Command::new(env!("CARGO_BIN_EXE_galileo")).arg("query").arg(&library).arg(QUESTION).arg("--json"), "");
    let verdict = succeed(Command::new("bun").arg(&script).arg(&module), &plans);
    std::fs::remove_dir_all(&scratch).unwrap();
    assert_eq!(verdict.trim(), r#"{"commands":48,"rows":48,"equal":48,"failures":[]}"#);
}
