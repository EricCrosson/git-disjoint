use std::fmt::Display;

/// Characters that interfere with terminal tab-completion, that will be
/// replaced with a hyphen.
static CHARACTERS_TO_REPLACE_WITH_HYPHEN: &[char] = &['!', '`', '(', ')'];

/// Characters to be deleted.
static CHARACTERS_TO_REMOVE: &[char] = &['\'', '"'];

#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub(crate) struct BranchName(String);

fn elide_consecutive_hyphens(mut s: String) -> String {
    let mut current_run = 0;
    s.retain(|c| {
        match c == '-' {
            true => current_run += 1,
            false => current_run = 0,
        };
        current_run < 2
    });
    s
}

impl BranchName {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn new(value: String) -> Self {
        let s = value.replace(CHARACTERS_TO_REPLACE_WITH_HYPHEN, "-");
        let s = elide_consecutive_hyphens(s);
        let s = s.replace(CHARACTERS_TO_REMOVE, "");
        Self(s)
    }
}

impl Display for BranchName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<String> for BranchName {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}
