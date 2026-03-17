mod common;

use common::{parse_fixture, run_fixture};

#[test]
fn fixtures() {
    insta::glob!("fixtures/*.kdl", |path| {
        let kdl = std::fs::read_to_string(path).unwrap();
        let fixture = parse_fixture(&kdl);
        let result = run_fixture(&fixture);
        insta::with_settings!({
            description => fixture.title.clone(),
            snapshot_path => path.parent().unwrap(),
            prepend_module_to_snapshot => false,
        }, {
            insta::assert_snapshot!(result);
        });
    });
}
