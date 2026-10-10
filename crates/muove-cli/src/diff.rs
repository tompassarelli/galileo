const CONTEXT: usize = 3;

#[derive(Clone, Copy, PartialEq)]
enum Op {
    Same,
    Delete,
    Insert,
}

fn script(old: &[&str], new: &[&str]) -> Vec<Op> {
    let (n, m) = (old.len(), new.len());
    let mut lcs = vec![vec![0usize; m + 1]; n + 1];
    for i in (0..n).rev() {
        for j in (0..m).rev() {
            lcs[i][j] = if old[i] == new[j] { lcs[i + 1][j + 1] + 1 } else { lcs[i + 1][j].max(lcs[i][j + 1]) };
        }
    }
    let (mut i, mut j, mut ops) = (0, 0, Vec::new());
    while i < n || j < m {
        if i < n && j < m && old[i] == new[j] {
            ops.push(Op::Same);
            i += 1;
            j += 1;
        } else if j == m || (i < n && lcs[i + 1][j] >= lcs[i][j + 1]) {
            ops.push(Op::Delete);
            i += 1;
        } else {
            ops.push(Op::Insert);
            j += 1;
        }
    }
    ops
}

fn lines(text: &str) -> Vec<&str> {
    text.split_inclusive('\n').collect()
}

pub fn unified(path: &str, old: &str, new: &str) -> String {
    let (a, b) = (lines(old), lines(new));
    let ops = script(&a, &b);
    let mut positions = Vec::with_capacity(ops.len() + 1);
    let (mut i, mut j) = (0, 0);
    for op in &ops {
        positions.push((i, j));
        match op {
            Op::Same => (i, j) = (i + 1, j + 1),
            Op::Delete => i += 1,
            Op::Insert => j += 1,
        }
    }
    positions.push((i, j));
    let changed: Vec<usize> = (0..ops.len()).filter(|&k| ops[k] != Op::Same).collect();
    let mut out = format!("--- {path} (committed)\n+++ {path} (emitted)\n");
    let mut k = 0;
    while k < changed.len() {
        let first = positions[changed[k]].0;
        let start = changed[k].saturating_sub(CONTEXT);
        let mut last = changed[k];
        while k + 1 < changed.len() && changed[k + 1] <= last + 2 * CONTEXT + 1 {
            k += 1;
            last = changed[k];
        }
        let end = (last + CONTEXT + 1).min(ops.len());
        let (a0, b0) = positions[start];
        let (a1, b1) = positions[end];
        out.push_str(&format!("@@ -{},{} +{},{} @@ {path}:{}\n", a0 + (a1 > a0) as usize, a1 - a0, b0 + (b1 > b0) as usize, b1 - b0, first + 1));
        for (op, &(i, j)) in ops[start..end].iter().zip(&positions[start..end]) {
            let (sign, text) = match op {
                Op::Same => (' ', a[i]),
                Op::Delete => ('-', a[i]),
                Op::Insert => ('+', b[j]),
            };
            out.push(sign);
            out.push_str(text);
            if !text.ends_with('\n') {
                out.push_str("\n\\ No newline at end of file\n");
            }
        }
        k += 1;
    }
    out
}
