use muove_nix::*;
use std::process::Command;

fn option(name: &str, fields: Vec<Binding>) -> Binding {
    attr(
        &format!("options.myConfig.modules.kanata.{name}"),
        apply(select(ident("lib"), "mkOption"), attrs(fields)),
    )
}

fn types(t: &str) -> Expr {
    select(ident("lib"), &format!("types.{t}"))
}

fn kanata_default() -> Expr {
    let body = attrs(vec![
        attr(
            "options.myConfig.modules.kanata.enable",
            apply(select(ident("lib"), "mkEnableOption"), text("Kanata keyboard remapping")),
        ),
        option(
            "devices",
            vec![
                attr("type", apply(types("listOf"), types("str"))),
                attr("default", Expr::List(vec![])),
                attr("description", text("Input device paths for kanata to capture. Find yours with: ls /dev/input/by-id/")),
            ],
        ),
        option(
            "configFile",
            vec![attr("type", types("path")), attr("description", text("Path to kanata .kbd config file"))],
        ),
        option(
            "port",
            vec![
                attr("type", apply(types("nullOr"), types("port"))),
                attr("default", ident("null")),
                attr("description", text("TCP port for kanata server (e.g. for glide integration)")),
            ],
        ),
        option(
            "extraArgs",
            vec![
                attr("type", apply(types("listOf"), types("str"))),
                attr("default", Expr::List(vec![])),
                attr(
                    "description",
                    text("Extra command-line args for kanata (e.g. --debug, --log-layer-changes for diagnosing layer/spacebar seizures)"),
                ),
            ],
        ),
        attr("imports", Expr::List(vec![path("./kanata.nix")])),
    ]);
    let formals = ["config", "lib", "pkgs"]
        .iter()
        .map(|n| Formal { name: n.to_string(), default: None })
        .collect();
    Expr::Lambda { param: Param::Formals { formals, ellipsis: true, at: None }, body: Box::new(body) }
}

fn parse(source: &str) -> String {
    let out = Command::new("nix-instantiate").args(["--parse", "-E", source]).output().expect("nix-instantiate runs");
    assert!(out.status.success(), "nix-instantiate --parse failed:\n{}\n{source}", String::from_utf8_lossy(&out.stderr));
    String::from_utf8(out.stdout).unwrap()
}

#[test]
fn kanata_default_prints_parseable_nix_equal_to_firn() {
    let root = std::env::var("FIRN_ROOT").unwrap_or_else(|_| {
        format!("{}/code/nixos-config/pins/10740a8ae2fd9aa327a7d30f7e7109ea797b9fa9", std::env::var("HOME").unwrap())
    });
    let fixture = std::fs::read_to_string(format!("{root}/modules/kanata/default.nix")).expect("fixture readable");
    let printed = print(&kanata_default());
    assert_eq!(parse(&printed), parse(&fixture));
}
