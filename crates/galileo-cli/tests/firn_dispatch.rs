#[path = "../../../tests/golden/harness.rs"]
mod harness;

use std::path::Path;
use std::process::Command;

const QUESTION: &str = "all ?order, ?node, ?edge, ?executable, ?arguments, ?required where ?command is number ?order and ?command walks ?node then ?edge and ?command plans ?executable with ?arguments and ?command needs leaf ?required, sorted by ?order";

#[test]
fn every_command_plans_as_firn_clause_does() {
    let library = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../lib/firn/dispatch.gal");
    let out = Command::new(env!("CARGO_BIN_EXE_galileo")).arg("query").arg(&library).arg(QUESTION).arg("--json").output().unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let plans = String::from_utf8(out.stdout).unwrap();
    assert_eq!(plans.matches("\"order\":").count(), 48);
    let mut golden = harness::Golden::new();
    golden.compare(&harness::area("firn-dispatch").join("plans.json"), &plans);
    golden.finish();
}
