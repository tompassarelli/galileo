#![allow(dead_code)]

use std::path::{Path, PathBuf};

pub const BLESS: &str = "GALILEO_BLESS=1 nix develop -c cargo test --workspace --locked --offline golden";

pub fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap()
}

pub fn area(name: &str) -> PathBuf {
    root().join("tests/golden").join(name)
}

pub fn cases(area: &Path, extension: &str) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = std::fs::read_dir(area)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.is_dir() || path.extension().is_some_and(|e| e == extension))
        .collect();
    found.sort();
    found
}

pub struct Golden {
    failures: Vec<String>,
    bless: bool,
}

impl Golden {
    pub fn new() -> Self {
        Golden { failures: Vec::new(), bless: std::env::var_os("GALILEO_BLESS").is_some_and(|v| v == "1") }
    }

    pub fn compare(&mut self, snapshot: &Path, actual: &str) {
        let expected = std::fs::read_to_string(snapshot).unwrap_or_default();
        if expected == actual {
            return;
        }
        if self.bless {
            std::fs::write(snapshot, actual).unwrap();
            return;
        }
        let name = snapshot.strip_prefix(root()).unwrap_or(snapshot).display().to_string();
        self.failures.push(format!("--- {name}\n+++ actual\n{}", unified(&expected, actual)));
    }

    pub fn finish(self) {
        if !self.failures.is_empty() {
            panic!("{}\n{} snapshot(s) differ; review the diff, then bless with:\n  {BLESS}", self.failures.join("\n"), self.failures.len());
        }
    }
}

pub fn unified(expected: &str, actual: &str) -> String {
    let old: Vec<&str> = expected.lines().collect();
    let new: Vec<&str> = actual.lines().collect();
    let mut lcs = vec![vec![0usize; new.len() + 1]; old.len() + 1];
    for i in (0..old.len()).rev() {
        for j in (0..new.len()).rev() {
            lcs[i][j] = if old[i] == new[j] { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }
    let mut lines: Vec<(char, &str)> = Vec::new();
    let (mut i, mut j) = (0, 0);
    while i < old.len() || j < new.len() {
        if i < old.len() && j < new.len() && old[i] == new[j] {
            lines.push((' ', old[i]));
            i += 1;
            j += 1;
        } else if j < new.len() && (i == old.len() || lcs[i][j + 1] > lcs[i + 1][j]) {
            lines.push(('+', new[j]));
            j += 1;
        } else {
            lines.push(('-', old[i]));
            i += 1;
        }
    }
    let changed: Vec<usize> = lines.iter().enumerate().filter(|(_, (mark, _))| *mark != ' ').map(|(k, _)| k).collect();
    let mut out = String::new();
    let mut k = 0;
    while k < changed.len() {
        let start = changed[k].saturating_sub(3);
        let mut end = changed[k] + 4;
        while k + 1 < changed.len() && changed[k + 1] < end + 3 {
            k += 1;
            end = changed[k] + 4;
        }
        let end = end.min(lines.len());
        let old_start = lines[..start].iter().filter(|(m, _)| *m != '+').count();
        let new_start = lines[..start].iter().filter(|(m, _)| *m != '-').count();
        let old_len = lines[start..end].iter().filter(|(m, _)| *m != '+').count();
        let new_len = lines[start..end].iter().filter(|(m, _)| *m != '-').count();
        out.push_str(&format!("@@ -{},{old_len} +{},{new_len} @@\n", old_start + 1, new_start + 1));
        for (mark, line) in &lines[start..end] {
            out.push_str(&format!("{mark}{line}\n"));
        }
        k += 1;
    }
    out
}
