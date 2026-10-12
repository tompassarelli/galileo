use galileo_nix::*;
use std::process::Command;

fn sequential(name: &str, value: Expr, body: Expr) -> Expr {
    apply(Expr::Lambda { param: Param::Ident(name.into()), body: Box::new(body) }, value)
}

#[test]
fn spec_10_2_recursive_let_agrees_with_nix_or_refuses_capture() {
    for index in 0..32 {
        let name = format!("value{index}");
        let later = format!("later{index}");
        let body = sequential(&later, ident(&name), ident(&later));
        let source = sequential(&name, Expr::Int(index), body);
        let printed = print_idiomatic(&source).unwrap();
        let result = Command::new("nix-instantiate").args(["--eval", "--strict", "--expr", &printed]).output().unwrap();
        assert!(result.status.success(), "{}", String::from_utf8_lossy(&result.stderr));
        assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), index.to_string());
        for captured in [&name, &later] {
            let source = sequential(&name, ident(captured), sequential(&later, Expr::Int(index), ident(&name)));
            let error = print_idiomatic(&source).unwrap_err();
            assert!(error.contains(captured), "{error}");
        }
        let masked = Expr::Lambda { param: Param::Ident(name.clone()), body: Box::new(ident(&name)) };
        let source = sequential(&name, masked, apply(ident(&name), Expr::Int(index)));
        let printed = print_idiomatic(&source).unwrap();
        let result = Command::new("nix-instantiate").args(["--eval", "--strict", "--expr", &printed]).output().unwrap();
        assert!(result.status.success());
        assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), index.to_string());
    }
}
