mod common;

use common::{parse_fixture, run_fixture};

#[test]
fn planning() {
    insta::glob!("fixtures/planning/*.kdl", |path| {
        let kdl = std::fs::read_to_string(path).unwrap();
        let fixture = parse_fixture(&kdl);
        let result = run_fixture(&fixture);
        insta::assert_snapshot!(result);
    });
}

#[test]
fn pre_validation() {
    insta::glob!("fixtures/pre_validation/*.kdl", |path| {
        let kdl = std::fs::read_to_string(path).unwrap();
        let fixture = parse_fixture(&kdl);
        let result = run_fixture(&fixture);
        insta::assert_snapshot!(result);
    });
}

#[test]
fn execution() {
    insta::glob!("fixtures/execution/*.kdl", |path| {
        let kdl = std::fs::read_to_string(path).unwrap();
        let fixture = parse_fixture(&kdl);
        let result = run_fixture(&fixture);
        insta::assert_snapshot!(result);
    });
}

#[test]
fn branch_names() {
    insta::glob!("fixtures/branch_names/*.kdl", |path| {
        let kdl = std::fs::read_to_string(path).unwrap();
        let fixture = parse_fixture(&kdl);
        let result = run_fixture(&fixture);
        insta::assert_snapshot!(result);
    });
}
