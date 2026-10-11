use std::io::{BufRead, BufReader, Read, Write};
use std::path::Path;
use std::process::{ChildStdin, ChildStdout, Command, Stdio};

fn send(stdin: &mut ChildStdin, body: &str) {
    write!(stdin, "Content-Length: {}\r\n\r\n{body}", body.len()).unwrap();
    stdin.flush().unwrap();
}

fn receive(stdout: &mut BufReader<ChildStdout>) -> String {
    let mut length = 0;
    loop {
        let mut header = String::new();
        stdout.read_line(&mut header).unwrap();
        let header = header.trim();
        if header.is_empty() {
            break;
        }
        if let Some(n) = header.strip_prefix("Content-Length: ") {
            length = n.parse().unwrap();
        }
    }
    let mut body = vec![0; length];
    stdout.read_exact(&mut body).unwrap();
    String::from_utf8(body).unwrap()
}

fn until(stdout: &mut BufReader<ChildStdout>, needle: &str) -> String {
    loop {
        let message = receive(stdout);
        if message.contains(needle) {
            return message;
        }
    }
}

#[test]
fn lsp_over_stdio_reports_devics_and_hovers_kanata() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/external/kanata");
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("kanata-lsp");
    let _ = std::fs::remove_dir_all(&scratch);
    std::fs::create_dir_all(&scratch).unwrap();
    std::fs::copy(source.join("default.gal"), scratch.join("default.gal")).unwrap();
    let kanata = std::fs::read_to_string(source.join("kanata.gal")).unwrap();
    let typo = kanata.replace("devices: cfg.devices", "devices: cfg.devics");
    assert_ne!(typo, kanata);
    std::fs::write(scratch.join("kanata.gal"), &typo).unwrap();
    let root = format!("file://{}", scratch.canonicalize().unwrap().display());
    let uri = format!("{root}/kanata.gal");

    let mut child = Command::new(env!("CARGO_BIN_EXE_galileo"))
        .arg("lsp")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdin = child.stdin.take().unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());

    send(&mut stdin, &format!(r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"capabilities":{{}},"rootUri":"{root}"}}}}"#));
    let init = until(&mut stdout, r#""id":1"#);
    assert!(init.contains(r#""hoverProvider":true"#), "{init}");
    send(&mut stdin, r#"{"jsonrpc":"2.0","method":"initialized","params":{}}"#);
    let text = typo.replace('\\', "\\\\").replace('"', "\\\"").replace('\n', "\\n");
    send(&mut stdin, &format!(r#"{{"jsonrpc":"2.0","method":"textDocument/didOpen","params":{{"textDocument":{{"uri":"{uri}","languageId":"galileo","version":1,"text":"{text}"}}}}}}"#));
    let diagnostics = until(&mut stdout, &format!(r#""uri":"{uri}""#));
    eprintln!("{diagnostics}");
    let (typo_line, typo_column) = typo.lines().enumerate().find_map(|(i, l)| l.find("devics").map(|c| (i, c))).unwrap();
    assert!(diagnostics.contains(&format!(r#""start":{{"character":{typo_column},"line":{typo_line}}}"#)), "{diagnostics}");
    assert!(["`devics`", "`myConfig.modules.kanata`", "`devices`", "default.gal:4"].iter().all(|fact| diagnostics.contains(fact)), "{diagnostics}");

    let (line, column) = typo.lines().enumerate().find_map(|(i, l)| l.find("port: cfg.port").map(|c| (i, c + "port: cfg.".len()))).unwrap();
    let at = format!(r#""textDocument":{{"uri":"{uri}"}},"position":{{"line":{line},"character":{column}}}"#);
    send(&mut stdin, &format!(r#"{{"jsonrpc":"2.0","id":2,"method":"textDocument/hover","params":{{{at}}}}}"#));
    let hover = until(&mut stdout, r#""id":2"#);
    eprintln!("{hover}");
    for expected in ["`cfg.port`", "`myConfig.modules.kanata.port`", "TCP port for kanata server", "`maybe Int`", "staged"] {
        assert!(hover.contains(expected), "{expected} in {hover}");
    }

    send(&mut stdin, &format!(r#"{{"jsonrpc":"2.0","id":3,"method":"textDocument/definition","params":{{{at}}}}}"#));
    let definition = until(&mut stdout, r#""id":3"#);
    assert!(definition.contains(&format!(r#""uri":"{root}/default.gal""#)) && definition.contains(r#""line":8"#), "{definition}");

    send(&mut stdin, &format!(r#"{{"jsonrpc":"2.0","id":4,"method":"textDocument/references","params":{{{at},"context":{{"includeDeclaration":true}}}}}}"#));
    let references = until(&mut stdout, r#""id":4"#);
    assert_eq!(references.matches(r#""uri""#).count(), 3, "{references}");

    send(&mut stdin, r#"{"jsonrpc":"2.0","id":5,"method":"shutdown"}"#);
    until(&mut stdout, r#""id":5"#);
    send(&mut stdin, r#"{"jsonrpc":"2.0","method":"exit"}"#);
    assert!(child.wait().unwrap().success());
}

#[test]
fn refs_lists_port_uses_in_both_kanata_sources() {
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tests/external/kanata");
    let out = Command::new(env!("CARGO_BIN_EXE_galileo"))
        .args(["refs", "myConfig.modules.kanata.port"])
        .current_dir(&source)
        .output()
        .unwrap();
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let stdout = String::from_utf8(out.stdout).unwrap();
    eprintln!("{stdout}");
    assert_eq!(
        stdout,
        "./default.gal:9: option myConfig.modules.kanata.port\n./kanata.gal:22:17: cfg.port
./kanata.gal:22:31: cfg.port\n"
    );
}
