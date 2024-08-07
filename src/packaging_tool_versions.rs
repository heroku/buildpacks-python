use std::str;

// We store these versions in requirements files so that Dependabot can update them.
// Each file must contain a single package specifier in the format `package==1.2.3`,
// from which we extract/validate the version substring at compile time.
pub(crate) const PIP_VERSION: &str =
    extract_requirement_version(include_str!("../requirements/pip.txt"));

// Extract the version substring from an exact-version package specifier (such as `foo==1.2.3`).
// This function should only be used to extract the version constants from the buildpack's own
// requirements files, which are controlled by us and don't require a full PEP 508 version parser.
// Since this is a `const fn` we cannot use iterators, most methods on `str`, `Result::expect` etc.
const fn extract_requirement_version(requirement: &'static str) -> &'static str {
    let mut bytes = requirement.as_bytes();
    while let [_, rest @ ..] = bytes {
        if let [b'=', b'=', version @ ..] = rest {
            if let Ok(version) = str::from_utf8(version.trim_ascii()) {
                return version;
            }
            break;
        }
        bytes = rest;
    }
    // This is safe, since this function is only used at compile time.
    panic!("Requirement must be in the format: 'package==X.Y.Z'");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_requirement_version_valid() {
        assert_eq!(extract_requirement_version("package==1.2.3"), "1.2.3");
        assert_eq!(extract_requirement_version("\npackage == 0.12\n"), "0.12");
    }

    #[test]
    #[should_panic(expected = "Requirement must be in the format")]
    fn extract_requirement_version_invalid() {
        extract_requirement_version("package=<1.2.3");
    }
}
