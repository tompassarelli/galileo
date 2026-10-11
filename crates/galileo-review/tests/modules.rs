use galileo_review::{parse, truth, Baseline, TruthChange};

const TAGS: &str = include_str!("../../../lib/firn/tags.gal");

const FIREFOX: &str = "\
use firn/tags
firefox is a Module
browsers is a Tag
firefox is tagged browsers
browsers sets myConfig.modules.firefox.default to true for firefox
";

const WHITERABBIT: &str = "\
use firn/tags
whiterabbit is a Host
whiterabbit runs linux
whiterabbit enables browsers
linux is a Platform
linux allows every module
";

fn baseline() -> Baseline {
    Baseline::freeze_modules(&[("firn/tags", TAGS), ("modules/firefox/default", FIREFOX), ("hosts/whiterabbit/tags", WHITERABBIT)]).unwrap()
}

fn loses_firefox(baseline: &Baseline, patch: &galileo_review::Patch) -> bool {
    truth(baseline.program(), &patch.program, galileo_core::Budget::default())
        .unwrap()
        .contains(&TruthChange::Lost("firefox is enabled on whiterabbit".into()))
}

#[test]
fn a_written_patch_edits_the_module_that_states_or_declares_each_name() {
    let baseline = baseline();
    let modules = |patch: &galileo_review::Patch| patch.modules.clone();
    let base: Vec<(String, String)> = baseline.modules().to_vec();

    let untag = parse(&baseline, "patch untag\n  remove\n    firefox is tagged browsers\n").unwrap();
    assert!(untag.diagnostics.is_empty(), "{:?}", untag.diagnostics);
    let mut expected = base.clone();
    expected[1].1 = FIREFOX.replace("firefox is tagged browsers\n", "");
    assert_eq!(modules(&untag), expected);
    assert!(loses_firefox(&baseline, &untag));

    let off = parse(&baseline, "patch off\n  add\n    whiterabbit turns off firefox\n").unwrap();
    assert!(off.diagnostics.is_empty(), "{:?}", off.diagnostics);
    assert_eq!(off.edit_sentences, ["Added: whiterabbit turns off firefox    patch:3"]);
    let (added, text) = off.modules.last().unwrap();
    assert!(!base.iter().any(|(name, _)| name == added));
    assert!(text.ends_with("whiterabbit turns off firefox\n"));
    assert_eq!(&off.modules[..3], &base[..]);
    assert!(loses_firefox(&baseline, &off));

    let rule = TAGS.split("rule tag-enabled: ").nth(1).unwrap().split("\n\n").next().unwrap();
    let body: String = rule.lines().map(|line| format!("    {}\n", line.trim())).collect();
    let replaced = parse(&baseline, &format!("patch same\n  replace rule tag-enabled\n{body}")).unwrap();
    assert!(replaced.diagnostics.is_empty(), "{:?}", replaced.diagnostics);
    let changed: Vec<&str> = replaced.modules.iter().zip(&base).filter(|(a, b)| a != b).map(|(a, _)| a.0.as_str()).collect();
    assert_eq!(changed, ["firn/tags"]);
    assert_eq!(replaced.modules.len(), 3);

    let error = parse(&baseline, "patch never\n  remove\n    whiterabbit turns on firefox\n").unwrap_err();
    assert_eq!(error.line, 3, "{error:?}");
}

#[test]
fn a_patch_over_modules_carries_its_contract() {
    let baseline = baseline();
    let verdicts = |claims: &str| {
        let source = format!("patch untag\n  remove\n    firefox is tagged browsers\n  intends\n{claims}");
        let patch = parse(&baseline, &source).unwrap();
        assert!(patch.diagnostics.is_empty(), "{:?}", patch.diagnostics);
        galileo_review::intended(baseline.program(), &patch, galileo_core::Budget::default()).unwrap()
    };
    let holds = |claim: &str| matches!(verdicts(&format!("    {claim}\n"))[..], [galileo_review::IntendedVerdict { verdict: galileo_core::Verdict::Holds(_), .. }]);

    assert!(holds("firefox is enabled on whiterabbit becomes false"));
    assert!(holds("only firefox is enabled on ?h changes"));
    assert!(holds("requirements unchanged"));
    let stays = verdicts("    firefox is enabled on whiterabbit stays\n");
    assert!(matches!(stays[0].verdict, galileo_core::Verdict::Fails(_)), "{stays:?}");
    assert!(stays[0].failure.as_deref().is_some_and(|f| f.contains("firefox is enabled on whiterabbit")), "{stays:?}");
}
