use std::str;

// We store these versions in requirements files so that Dependabot can update them.
// Each file must contain a single package specifier in the format `package==1.2.3`,
// from which we extract/validate the version substring at compile time.
pub(crate) const PIP_VERSION: &str =
    extract_requirement_version(include_str!("../requirements/pip.txt"))
        .expect("pip.txt must contain 'pip==VERSION'");
pub(crate) const POETRY_VERSION: &str =
    extract_requirement_version(include_str!("../requirements/poetry.txt"))
        .expect("poetry.txt must contain 'poetry==VERSION'");

// Extract the version substring from an exact-version package specifier (such as `foo==1.2.3`).
// This function should only be used to extract the version constants from the buildpack's own
// requirements files, which are controlled by us and don't require a full PEP 508 version parser.
// Note: Since this is a `const fn` we cannot use iterators and most methods on `str` / `Result`.
const fn extract_requirement_version(requirement: &'static str) -> Option<&'static str> {
    let mut bytes = requirement.as_bytes();
    while let [_, rest @ ..] = bytes {
        if let [b'=', b'=', version @ ..] = rest {
            if let Ok(version) = str::from_utf8(version.trim_ascii()) {
                return Some(version);
            }
            break;
        }
        bytes = rest;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_requirement_version_valid() {
        assert_eq!(extract_requirement_version("package==1.2.3"), Some("1.2.3"));
        assert_eq!(
            extract_requirement_version("\npackage == 0.12\n"),
            Some("0.12")
        );
    }

    #[test]
    fn extract_requirement_version_invalid() {
        assert_eq!(extract_requirement_version(""), None);
        assert_eq!(extract_requirement_version("package"), None);
        assert_eq!(extract_requirement_version("package=<1.2.3"), None);
    }
}
