use galileo_nix::{AttrKey, AttrPath, BinOp, Binding, Expr, Param, StrPart};
use std::collections::{BTreeMap, BTreeSet};
use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};

type Census = BTreeMap<String, (usize, BTreeSet<String>)>;

fn count(census: &mut Census, construct: impl Into<String>, module: &str) {
    let (count, files) = census.entry(construct.into()).or_default();
    *count += 1;
    files.insert(module.to_owned());
}

fn keys(path: &AttrPath, selection: bool, module: &str, census: &mut Census) {
    for key in path {
        match key {
            AttrKey::Ident(_) => count(census, "name key", module),
            AttrKey::Str(parts) => {
                count(
                    census,
                    if selection {
                        "quoted selection"
                    } else {
                        "quoted key"
                    },
                    module,
                );
                for part in parts {
                    if let StrPart::Interp(value) = part {
                        count(census, "interpolated key", module);
                        visit(value, module, census);
                    }
                }
            }
            AttrKey::Dynamic(value) => {
                count(census, "computed key", module);
                visit(value, module, census);
            }
        }
    }
}

fn bindings(values: &[Binding], module: &str, census: &mut Census) {
    for binding in values {
        match binding {
            Binding::Attr { path, value } => {
                keys(path, false, module, census);
                visit(value, module, census);
            }
            Binding::Inherit { from, .. } => {
                count(census, "inherit", module);
                if let Some(value) = from {
                    visit(value, module, census);
                }
            }
        }
    }
}

fn call_name(value: &Expr) -> Option<String> {
    match value {
        Expr::Ident(name) => Some(name.clone()),
        Expr::Select {
            expr,
            path,
            default: None,
        } => {
            let mut name = call_name(expr)?;
            for key in path {
                let key = match key {
                    AttrKey::Ident(key) => key,
                    AttrKey::Str(parts) => match parts.as_slice() {
                        [StrPart::Text(key)] => key,
                        _ => return None,
                    },
                    _ => return None,
                };
                name.push('.');
                name.push_str(key);
            }
            Some(name)
        }
        _ => None,
    }
}

fn visit(value: &Expr, module: &str, census: &mut Census) {
    match value {
        Expr::Int(_) => count(census, "integer", module),
        Expr::Float(_) => count(census, "float", module),
        Expr::Uri(_) => count(census, "URI", module),
        Expr::Ident(_) => count(census, "identifier", module),
        Expr::Path(_) => count(census, "path", module),
        Expr::Str(parts) | Expr::IndentedStr(parts) => {
            count(
                census,
                if matches!(value, Expr::Str(_)) {
                    "string"
                } else {
                    "indented string"
                },
                module,
            );
            for part in parts {
                if let StrPart::Interp(value) = part {
                    count(census, "string interpolation", module);
                    visit(value, module, census);
                }
            }
        }
        Expr::List(values) => {
            count(census, "list", module);
            for value in values {
                visit(value, module, census);
            }
        }
        Expr::AttrSet {
            rec,
            bindings: values,
        } => {
            count(
                census,
                if *rec { "recursive record" } else { "record" },
                module,
            );
            bindings(values, module, census);
        }
        Expr::Select {
            expr,
            path,
            default,
        } => {
            count(census, "selection", module);
            keys(path, true, module, census);
            visit(expr, module, census);
            if let Some(value) = default {
                count(census, "selection default", module);
                visit(value, module, census);
            }
        }
        Expr::HasAttr { expr, path } => {
            count(census, "?", module);
            visit(expr, module, census);
            keys(path, true, module, census);
        }
        Expr::Apply { func, arg } => {
            count(census, "application", module);
            if let Some(name) = call_name(func) {
                if name.starts_with("lib.")
                    || name.starts_with("builtins.")
                    || matches!(name.as_str(), "import" | "toString")
                {
                    count(census, format!("call {name}"), module);
                }
            }
            visit(func, module, census);
            visit(arg, module, census);
        }
        Expr::Lambda { param, body } => {
            match param {
                Param::Ident(name) => {
                    count(census, "identifier lambda", module);
                    if name.starts_with("__clause_argument_") {
                        count(census, "generated argument lambda", module);
                    }
                }
                Param::Formals {
                    formals,
                    ellipsis,
                    at,
                } => {
                    count(census, "formals lambda", module);
                    if *ellipsis {
                        count(census, "open formals", module);
                    }
                    if at.is_some() {
                        count(census, "@", module);
                    }
                    for formal in formals {
                        if let Some(value) = &formal.default {
                            count(census, "formal default", module);
                            visit(value, module, census);
                        }
                    }
                }
            }
            visit(body, module, census);
        }
        Expr::Let {
            bindings: values,
            body,
        } => {
            count(census, "let in", module);
            bindings(values, module, census);
            visit(body, module, census);
        }
        Expr::If { cond, then, else_ } => {
            count(census, "if", module);
            for value in [cond, then, else_] {
                visit(value, module, census);
            }
        }
        Expr::With { scope, body } | Expr::Assert { cond: scope, body } => {
            count(
                census,
                if matches!(value, Expr::With { .. }) {
                    "with"
                } else {
                    "assert"
                },
                module,
            );
            visit(scope, module, census);
            visit(body, module, census);
        }
        Expr::BinOp { op, lhs, rhs } => {
            count(census, format!("operator {}", op.symbol()), module);
            if *op == BinOp::Add && matches!(lhs.as_ref(), Expr::BinOp { op: BinOp::Add, .. }) {
                count(census, "left-nested + chain", module);
            }
            visit(lhs, module, census);
            visit(rhs, module, census);
        }
        Expr::Not(inner) | Expr::Neg(inner) => {
            count(
                census,
                if matches!(value, Expr::Neg(_)) {
                    "negation"
                } else {
                    "not"
                },
                module,
            );
            visit(inner, module, census);
        }
    }
}

fn parse_nix(source: &[u8], directory: &Path) -> Result<Vec<u8>, String> {
    let mut child = Command::new("nix-instantiate")
        .args(["--parse", "-"])
        .current_dir(directory)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(source).unwrap();
    let output = child.wait_with_output().unwrap();
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(String::from_utf8_lossy(&output.stderr).replace('\n', " "))
    }
}

fn whiterabbit_enables(firn: &str) -> BTreeMap<String, bool> {
    let output = Command::new("nix")
        .args(["eval", "--json"])
        .arg(format!(
            "path:{firn}#nixosConfigurations.whiterabbit.config.myConfig.modules"
        ))
        .args([
            "--apply",
            "modules: builtins.mapAttrs (name: module: module.enable or null) modules",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "whiterabbit eval failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let text = String::from_utf8(output.stdout).unwrap();
    let body = text.trim().trim_start_matches('{').trim_end_matches('}');
    body.split(',')
        .filter_map(|entry| {
            let (name, value) = entry.split_once(':')?;
            let enabled = match value {
                "true" => true,
                "false" => false,
                _ => return None,
            };
            Some((name.trim_matches('"').to_owned(), enabled))
        })
        .collect()
}

const DEFAULT_MODULES: [&str; 3] = ["auto-upgrade", "bluetooth", "boot"];

#[test]
fn d39_committed_clause_outputs_match_nix_instantiate_parse() {
    let firn = std::env::var("FIRN_ROOT").unwrap_or_else(|_| {
        format!(
            "{}/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9",
            std::env::var("HOME").unwrap()
        )
    });
    let mut modules = std::fs::read_dir(Path::new(&firn).join("modules"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.join("tags.clause").is_file() && path.join("default.nix").is_file())
        .collect::<Vec<_>>();
    modules.sort();
    let full = std::env::var_os("GALILEO_FULL").is_some();
    if !full {
        modules.retain(|path| DEFAULT_MODULES.contains(&path.file_name().unwrap().to_str().unwrap()));
        assert_eq!(modules.len(), DEFAULT_MODULES.len());
    }
    let enables = whiterabbit_enables(&firn);
    let scratch = Path::new(env!("CARGO_TARGET_TMPDIR")).join("clause-nix");
    std::fs::create_dir_all(&scratch).unwrap();
    let mut census = Census::new();
    let mut identical = 0;
    let mut failures = Vec::new();
    for directory in &modules {
        let name = directory.file_name().unwrap().to_str().unwrap();
        let module = format!("modules/{name}/default.nix");
        let source = std::fs::read(directory.join("default.nix")).unwrap();
        let tree = galileo_import::nix::parse(std::str::from_utf8(&source).unwrap()).unwrap();
        visit(&tree, &module, &mut census);
        let imported = Command::new(env!("CARGO_BIN_EXE_galileo"))
            .args(["import", "nix"])
            .arg(directory.join("default.nix"))
            .output()
            .unwrap();
        let result = if !imported.status.success() {
            Err(format!(
                "import: {}",
                String::from_utf8_lossy(&imported.stderr).trim()
            ))
        } else {
            let file = scratch.join(format!("{name}.gal"));
            std::fs::write(&file, imported.stdout).unwrap();
            let built = Command::new(env!("CARGO_BIN_EXE_galileo"))
                .args(["build", "--stdout"])
                .arg(file)
                .output()
                .unwrap();
            if !built.status.success() {
                Err(format!(
                    "build: {}",
                    String::from_utf8_lossy(&built.stderr)
                        .lines()
                        .next()
                        .unwrap_or("unknown failure")
                ))
            } else {
                parse_nix(&source, &scratch).and_then(|expected| {
                    parse_nix(&built.stdout, &scratch).and_then(|actual| {
                        if expected == actual { Ok(()) }
                        else {
                            let expected = String::from_utf8(expected).unwrap();
                            let actual = String::from_utf8(actual).unwrap();
                            let first = expected.chars().zip(actual.chars()).take_while(|(a, b)| a == b).count();
                            let original = expected.chars().skip(first.saturating_sub(40)).take(160).collect::<String>();
                            let rebuilt = actual.chars().skip(first.saturating_sub(40)).take(160).collect::<String>();
                            Err(format!("nix-instantiate --parse construct differs at character {first}: original={original:?}; rebuilt={rebuilt:?}"))
                        }
                    })
                })
            }
        };
        let whiterabbit = match enables.get(name) {
            Some(true) => "enabled",
            Some(false) => "disabled",
            None => "always",
        };
        match result {
            Ok(()) => {
                identical += 1;
                println!("IDENTICAL {module} whiterabbit={whiterabbit}");
            }
            Err(error) => failures.push((format!("{module} whiterabbit={whiterabbit}"), error)),
        }
    }
    for (construct, (count, files)) in census {
        println!(
            "CENSUS {construct}: occurrences={count} modules={} files={}",
            files.len(),
            files.into_iter().collect::<Vec<_>>().join(",")
        );
    }
    for (module, error) in &failures {
        println!("FIRST FAILURE {module}: {error}");
    }
    let enabled = modules
        .iter()
        .filter(|path| enables.get(path.file_name().unwrap().to_str().unwrap()) != Some(&false))
        .count();
    println!(
        "CLAUSE NIX outputs={} parsed={} parse-identical={identical} failures={} whiterabbit-enabled={enabled}",
        modules.len(),
        modules.len(),
        failures.len()
    );
    assert_eq!(
        identical,
        modules.len(),
        "D-39 requires every Clause output parse-identical"
    );
}
