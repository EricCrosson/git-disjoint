use lazy_static::lazy_static;
use regex::Regex;

// DISCUSS: publishing this as a separate library crate.
//
/// Rules from [git-check-ref-format].
///
/// [git-check-ref-format]: https://git-scm.com/docs/git-check-ref-format
pub(crate) fn sanitize_text_for_git_branch_name(text: &str) -> String {
    let mut result = text.to_owned();

    // They must contain at least one /. This enforces the presence of a category like heads/, tags/ etc. but the actual names are not restricted. If the --allow-onelevel option is used, this rule is waived.
    // FIXME: I'm not certain how to interpret this rule yet.
    // if !result.contains("/") {
    //     result.push_str("-/-");
    // }

    // They can include slash / for hierarchical (directory) grouping, but no slash-separated component can begin with a dot . or end with the sequence .lock.
    if result.starts_with(".") {
        result = result.replacen(".", "-", 1);
    }
    result = result.replace("/.", "/-");
    // FIXME: this is overly cautious
    result = result.replace(".lock", "-");

    // They cannot have ASCII control characters (i.e. bytes whose values are lower than \040, or \177 DEL).
    lazy_static! {
        static ref RE_CONTROL_CHARACTER: Regex = Regex::new("[[:cntrl:]]+")
            .expect("Expected control-character regular expression to compile");
    }
    result = RE_CONTROL_CHARACTER.replace_all(&result, "-").into();

    // They cannot have space anywhere.
    lazy_static! {
        static ref RE_WHITESPACE_CHARACTER: Regex = Regex::new("[[:space:]]+")
            .expect("Expected whitespace-character regular expression to compile");
    }
    result = RE_WHITESPACE_CHARACTER.replace_all(&result, "-").into();
    // Property testing has identified characters that are not detected by the
    // above regex, so we'll use this stronger test to remove all spaces.
    result.retain(|char| !char.is_whitespace());

    // They cannot have tilde ~ anywhere.
    result = result.replace("~", "-");

    // They cannot have caret ^ anywhere.
    result = result.replace("^", "-");

    // They cannot have colon : anywhere.
    result = result.replace(":", "-");

    // They cannot have question-mark ?, asterisk *, or open bracket [ anywhere. See the --refspec-pattern option below for an exception to this rule.
    result = result.replace("?", "-");
    result = result.replace("*", "-");
    result = result.replace("[", "-");

    // They cannot contain a sequence @{.
    result = result.replace("@{", "-");

    // They cannot be the single character @.
    // FIXME: this implementation is too restrictive but I'm not exactly sure of the rules right now.
    // Happy to widen this up if I get more clarity and feel confident we'll avoid false-positives.
    result = result.replace("@", "-");

    // They cannot contain a \.
    result = result.replace(r"\", "-");

    // They cannot contain multiple consecutive slashes (see the --normalize option below for an exception to this rule)
    lazy_static! {
        static ref RE_MULTIPLE_FORWARD_SLASHES: Regex = Regex::new("/{2,}")
            .expect("Expected multiple-forward-slashes regular expression to compile");
    }
    result = RE_MULTIPLE_FORWARD_SLASHES.replace_all(&result, "-").into();

    // They cannot have two consecutive dots .. anywhere.
    lazy_static! {
        static ref RE_MULTIPLE_DOTS: Regex =
            Regex::new("[.]{2,}").expect("Expected multiple-dots regular expression to compile");
    }
    result = RE_MULTIPLE_DOTS.replace_all(&result, "-").into();

    // They cannot begin with a slash / (see the --normalize option below for an exception to this rule)
    // FIXME: this has a bug, property-testing is finding it consistently
    while result.starts_with("/") {
        result = result.replacen("/", "-", 1);
    }

    // They cannot end with a dot .
    // They cannot end with a slash / (see the --normalize option below for an exception to this rule)
    while result.ends_with("/") || result.ends_with(".") {
        result.pop();
    }

    if result.ends_with("/") {
        result.pop();
    }

    // Convert any sequence of multiple hyphens into a single hyphen.
    // We convert invalid characters into hyphens to prevent shrinking the input into an empty string.
    lazy_static! {
        static ref RE_MULTIPLE_HYPHENS: Regex =
            Regex::new("-{2,}").expect("Expected multiple-hyphens regular expression to compile");
    }
    result = RE_MULTIPLE_HYPHENS.replace_all(&result, "-").into();

    result
}

#[cfg(test)]
mod test {
    use crate::sanitize_git_branch_name::sanitize_text_for_git_branch_name;

    use proptest::prelude::*;

    macro_rules! test_does_not_violate_branch_naming_rule {
        ($unit_test:ident, $property_test:ident, $test_of_inclusion:expr, $unsanitized_branch_name:expr) => {
            #[test]
            fn $unit_test() {
                let sanitized_branch_name = sanitize_text_for_git_branch_name(&$unsanitized_branch_name);
                assert!(
                    !$test_of_inclusion(&sanitized_branch_name),
                    "Expected unsanitized string {:?} to sanitize to a valid branch name, but {:?} is not a valid branch name",
                    &$unsanitized_branch_name,
                    &sanitized_branch_name
                );
            }

            proptest! {
                #[test]
                fn $property_test(unsanitized_branch_name in any::<String>()) {
                    let sanitized_branch_name = sanitize_text_for_git_branch_name(&unsanitized_branch_name);
                    assert!(
                        !$test_of_inclusion(&sanitized_branch_name),
                        "Expected unsanitized string {:?} to sanitize to a valid branch name, but {:?} is not a valid branch name",
                        &unsanitized_branch_name,
                        &sanitized_branch_name
                    );
                }
            }
        };
    }

    // They can include slash / for hierarchical (directory) grouping, but no slash-separated component can begin with a dot.
    test_does_not_violate_branch_naming_rule!(
        branch_name_does_not_contain_a_slash_separated_component_beginning_with_a_dot,
        proptest_branch_name_does_not_contain_a_slash_separated_component_beginning_with_a_dot,
        |branch_name: &str| -> bool {
            for slash_separated_sequence in branch_name.split("/") {
                if slash_separated_sequence.starts_with(".") {
                    return true;
                }
            }
            false
        },
        "refs/heads/.master"
    );

    // Branch names can include slash / for hierarchical (directory) grouping, but no slash-separated component can end with the sequence .lock.
    test_does_not_violate_branch_naming_rule!(
        branch_name_does_not_contain_a_slash_separated_component_ending_with_dot_lock,
        proptest_branch_name_does_not_contain_a_slash_separated_component_ending_with_dot_lock,
        |branch_name: &str| -> bool {
            for slash_separated_sequence in branch_name.split("/") {
                if slash_separated_sequence.ends_with(".lock") {
                    return true;
                }
            }
            false
        },
        "refs/heads/master.lock"
    );

    // They must contain at least one /. This enforces the presence of a category like heads/, tags/ etc. but the actual names are not restricted.
    // If the --allow-onelevel option is used, this rule is waived.
    // FIXME: I'm not sure how to interpret this rule yet.
    // fn has_at_least_one_slash<S: AsRef<str>>(branch_name: S) -> bool {
    //     branch_name.as_ref().contains("/")
    // }

    // #[test]
    // fn branch_name_has_at_least_one_slash() {
    //     assert!(has_at_least_one_slash(sanitize_text_for_git_branch_name(
    //         "refs/heads/master"
    //     )))
    // }

    // test_does_not_violate_branch_naming_rule!(
    //     branch_name_does_not_contain_two_consecutive_dots,
    //     proptest_branch_name_does_not_contain_two_consecutive_dots,
    //     |branch_name: &str| -> bool { branch_name.contains("..") },
    //     "refs/heads/master..foo"
    // );

    // They cannot have ASCII control characters (i.e. bytes whose values are lower than \040, or \177 DEL).
    // FIXME: Maintainer's note: not sure how to test "bytes whose values are lower than \040, or \177 DEL" yet

    // They cannot have space anywhere.
    test_does_not_violate_branch_naming_rule!(
        branch_name_does_not_contain_a_space,
        proptest_branch_name_does_not_contain_a_space,
        |branch_name: &str| -> bool { branch_name.contains(char::is_whitespace) },
        "/refs/heads/master foo"
    );

    // They cannot have tilde ~ anywhere.
    test_does_not_violate_branch_naming_rule!(
        branch_name_does_not_contain_a_tilde,
        proptest_branch_name_does_not_contain_a_tilde,
        |branch_name: &str| -> bool { branch_name.contains("?") },
        "/refs/heads/master~foo"
    );

    // They cannot have caret ^ anywhere.
    test_does_not_violate_branch_naming_rule!(
        branch_name_does_not_contain_a_carat,
        proptest_branch_name_does_not_contain_a_carat,
        |branch_name: &str| -> bool { branch_name.contains("^") },
        "/refs/heads/master^foo"
    );

    // They cannot have colon : anywhere.
    test_does_not_violate_branch_naming_rule!(
        branch_name_does_not_contain_a_colon,
        proptest_branch_name_does_not_contain_a_colon,
        |branch_name: &str| -> bool { branch_name.contains(":") },
        "/refs/heads/master:foo"
    );

    // They cannot have question-mark ? anywhere. See the --refspec-pattern option below for an exception to this rule.
    test_does_not_violate_branch_naming_rule!(
        branch_name_does_not_contain_a_question_mark,
        proptest_branch_name_does_not_contain_a_question_mark,
        |branch_name: &str| -> bool { branch_name.starts_with("?") },
        "/refs/heads/master?foo"
    );

    // They cannot have asterisk * anywhere. See the --refspec-pattern option below for an exception to this rule.
    test_does_not_violate_branch_naming_rule!(
        branch_name_does_not_contain_an_asterisk,
        proptest_branch_name_does_not_contain_an_asterisk,
        |branch_name: &str| -> bool { branch_name.starts_with("*") },
        "/refs/heads/master*foo"
    );

    // They cannot have open bracket [ anywhere. See the --refspec-pattern option below for an exception to this rule.
    test_does_not_violate_branch_naming_rule!(
        branch_name_does_not_contain_an_open_bracket,
        proptest_branch_name_does_not_contain_an_open_bracket,
        |branch_name: &str| -> bool { branch_name.starts_with("[") },
        "/refs/heads/master[foo"
    );

    // They cannot begin with a slash (/) (see the --normalize option for an exception to this rule)
    test_does_not_violate_branch_naming_rule!(
        branch_name_does_not_begin_with_a_forward_slash,
        proptest_branch_name_does_not_begin_with_a_forward_slash,
        |branch_name: &str| -> bool { branch_name.starts_with("/") },
        "/refs/heads/master"
    );

    // They cannot begin with a slash (/) (see the --normalize option for an exception to this rule)
    test_does_not_violate_branch_naming_rule!(
        branch_name_does_not_end_with_a_forward_slash,
        proptest_branch_name_does_not_end_with_a_forward_slash,
        |branch_name: &str| -> bool { branch_name.ends_with("/") },
        "refs/heads/master/"
    );

    // They cannot contain multiple consecutive slashes (see the --normalize option for an exception to this rule)
    test_does_not_violate_branch_naming_rule!(
        branch_name_does_not_contain_multiple_consecutive_forward_slashes,
        proptest_branch_name_does_not_contain_multiple_consecutive_forward_slashes,
        |branch_name: &str| -> bool { branch_name.contains("//") },
        "refs/heads/master//all-right"
    );

    // They cannot end with a dot .
    test_does_not_violate_branch_naming_rule!(
        branch_name_does_not_end_with_dot,
        proptest_branch_name_does_not_end_with_dot,
        |branch_name: &str| -> bool { branch_name.ends_with(".") },
        "refs/heads/master."
    );

    // They cannot contain a sequence @{.
    test_does_not_violate_branch_naming_rule!(
        branch_name_does_not_contain_ampersand_open_brace,
        proptest_branch_name_does_not_contain_ampersand_open_brace,
        |branch_name: &str| -> bool { branch_name.contains("@{") },
        "refs/heads/master-@{-branch"
    );

    // FIXME: this implementation is too restrictive but I'm not exactly sure of the rules right now.
    // Happy to widen this up if I get more clarity and feel confident we'll avoid false-positives.
    // They cannot be the single character @.
    test_does_not_violate_branch_naming_rule!(
        branch_name_does_not_contain_ampersand,
        proptest_branch_name_does_not_contain_ampersand,
        |branch_name: &str| -> bool { branch_name.contains("@") },
        "refs/heads/master-@-branch"
    );

    // They cannot contain a \.
    test_does_not_violate_branch_naming_rule!(
        branch_name_does_not_contain_backslash,
        proptest_branch_name_does_not_contain_backslash,
        |branch_name: &str| -> bool { branch_name.contains(r"\") },
        r"refs/heads/master-\-branch"
    );
}
