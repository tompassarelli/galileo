#[path = "../../../tests/golden/harness.rs"]
mod harness;

use harness::{area, cases, root, Golden};
use std::path::{Path, PathBuf};
use std::process::Command;

fn galileo(cwd: &Path, arguments: &[String]) -> String {
    let out = Command::new(env!("CARGO_BIN_EXE_galileo")).args(arguments).current_dir(cwd).output().unwrap();
    format!(
        "$ galileo {}\n{}{}exit {}\n",
        arguments.join(" "),
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr),
        out.status.code().unwrap()
    )
}

fn project(area: &str, case: &Path) -> (PathBuf, Vec<String>) {
    let name = case.file_name().unwrap();
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join("golden").join(area).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join(".git")).unwrap();
    let mut files = Vec::new();
    if case.is_dir() {
        for source in cases(case, "gal") {
            let file = source.file_name().unwrap().to_string_lossy().into_owned();
            std::fs::copy(&source, dir.join(&file)).unwrap();
            files.push(file);
        }
    } else {
        std::fs::copy(case, dir.join(name)).unwrap();
        files.push(name.to_string_lossy().into_owned());
    }
    (dir, files)
}

fn snapshot(case: &Path) -> PathBuf {
    case.with_extension("txt")
}

#[test]
fn golden_diagnostics() {
    let mut golden = Golden::new();
    for case in cases(&area("diagnostics"), "gal") {
        let (dir, files) = project("diagnostics", &case);
        let check: Vec<String> = std::iter::once("check".to_string()).chain(files.iter().cloned()).collect();
        let build: Vec<String> = ["build".to_string(), "--check".to_string()].into_iter().chain(files).collect();
        golden.compare(&snapshot(&case), &format!("{}{}", galileo(&dir, &check), galileo(&dir, &build)));
    }
    golden.finish();
}

#[test]
fn golden_lowering() {
    let mut golden = Golden::new();
    for case in cases(&area("lowering"), "gal") {
        let (dir, files) = project("lowering", &case);
        let arguments = ["build".to_string(), "--stdout".to_string(), files[0].clone()];
        golden.compare(&snapshot(&case), &galileo(&dir, &arguments));
    }
    golden.finish();
}

#[test]
fn golden_why() {
    let mut golden = Golden::new();
    for case in cases(&area("why"), "args") {
        let arguments: Vec<String> = std::fs::read_to_string(&case).unwrap().lines().map(String::from).collect();
        golden.compare(&snapshot(&case), &galileo(&root(), &arguments));
    }
    golden.finish();
}
