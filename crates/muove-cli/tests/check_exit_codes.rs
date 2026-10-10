use std::process::Command;

fn check(arguments: &[&str]) -> i32 {
    Command::new(env!("CARGO_BIN_EXE_muove"))
        .arg("check")
        .args(arguments)
        .output()
        .unwrap()
        .status
        .code()
        .unwrap()
}

#[test]
fn muove_check_exits_0_when_holding_1_when_failing_and_2_when_unknown() {
    assert_eq!(check(&["gate"]), 0);
    assert_eq!(check(&["gate-never"]), 1);
    assert_eq!(check(&["chain", "--budget", "1000"]), 2);
}
