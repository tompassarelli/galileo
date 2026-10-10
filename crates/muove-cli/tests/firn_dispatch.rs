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
        format!("{}/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9", std::env::var("HOME").unwrap())
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
    let workbench = std::env::var("FIRN_CLAUSE_WORKBENCH").unwrap_or_else(|_| {
        format!("{}/.local/lib/firn/cli/current/bin/clause-workbench", std::env::var("HOME").unwrap())
    });
    let scratch = std::env::temp_dir().join(format!("muove-firn-dispatch-{}", std::process::id()));
    std::fs::create_dir_all(scratch.join("native")).unwrap();
    std::fs::create_dir_all(scratch.join("bin")).unwrap();
    std::fs::write(scratch.join("bin/firn-io.mjs"), "export const executeRuntime = () => 0;\n").unwrap();
    let module = scratch.join("native/firn-dispatcher.js");
    let script = scratch.join("compare.mjs");
    std::fs::write(&script, COMPARE).unwrap();
    succeed(Command::new(&workbench).arg("compile-js").arg(firn_root().join("native/firn.clause")).arg(&module), "");
    let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../lib/firn/dispatch.muove");
    let plans = succeed(Command::new(env!("CARGO_BIN_EXE_muove")).arg("query").arg(&library).arg(QUESTION).arg("--json"), "");
    let verdict = succeed(Command::new("bun").arg(&script).arg(&module), &plans);
    std::fs::remove_dir_all(&scratch).unwrap();
    assert_eq!(verdict.trim(), r#"{"commands":47,"rows":47,"equal":47,"failures":[]}"#);
}
