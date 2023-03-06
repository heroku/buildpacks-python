use crate::utils;
use serde::Deserialize;
use std::io;
use std::path::Path;

/// Reads the `com.salesforce.type` field from any `project.toml` in the specified directory.
///
/// It is permitted for the `project.toml` file not to exist, or for there to be no `com.salesforce`
/// table within the TOML document, in which case `Ok(None)` will be returned.
///
/// However, an error will be returned if any other IO error occurred, the file is not valid TOML,
/// or the TOML document does not adhere to the schema.
pub(crate) fn read_salesforce_project_type(
    app_dir: &Path,
) -> Result<Option<SalesforceProjectType>, ProjectDescriptorError> {
    read_project_descriptor(app_dir).map(|descriptor| {
        descriptor
            .unwrap_or_default()
            .com
            .unwrap_or_default()
            .salesforce
            .map(|salesforce| salesforce.project_type)
    })
}

/// Reads any `project.toml` file in the specified directory, parsing it into a [`ProjectDescriptor`].
///
/// It is permitted for the `project.toml` file not to exist, in which case `Ok(None)` will be returned.
///
/// However, an error will be returned if any other IO error occurred, the file is not valid TOML,
/// or the TOML document does not adhere to the schema.
fn read_project_descriptor(
    app_dir: &Path,
) -> Result<Option<ProjectDescriptor>, ProjectDescriptorError> {
    let project_descriptor_path = app_dir.join("project.toml");

    utils::read_optional_file(&project_descriptor_path)
        .map_err(ProjectDescriptorError::Io)?
        .map(|contents| parse(&contents).map_err(ProjectDescriptorError::Parse))
        .transpose()
}

/// Parse the contents of a project descriptor TOML file into a [`ProjectDescriptor`].
///
/// An error will be returned if the string is not valid TOML, or the TOML document does not
/// adhere to the schema.
fn parse(contents: &str) -> Result<ProjectDescriptor, toml::de::Error> {
    toml::from_str::<ProjectDescriptor>(contents)
}

/// Represents a Cloud Native Buildpack project descriptor file (`project.toml`).
///
/// Currently only fields used by the buildpack are enforced, so this represents only a
/// subset of the upstream CNB project descriptor schema.
///
/// See: <https://github.com/buildpacks/spec/blob/main/extensions/project-descriptor.md>
#[derive(Debug, Default, Deserialize, PartialEq)]
struct ProjectDescriptor {
    com: Option<ComTable>,
}

/// Represents the `com` table in the project descriptor.
#[derive(Debug, Default, Deserialize, PartialEq)]
struct ComTable {
    salesforce: Option<SalesforceTable>,
}

/// Represents the `com.salesforce` table in the project descriptor.
///
/// Currently only fields used by the buildpack are enforced, so this represents only a
/// subset of the Salesforce-specific project descriptor schema.
///
/// See: <https://salesforce.quip.com/tLL9AiScqg5q#WGeACA6OzZf>
#[derive(Debug, Deserialize, PartialEq)]
struct SalesforceTable {
    #[serde(rename = "type")]
    project_type: SalesforceProjectType,
}

/// The type of a Salesforce project.
///
/// For now `Function` is the only valid type, however others will be added in the future.
///
/// Unknown project types are intentionally rejected, since we're prioritising the UX for
/// functions projects where the type may have been mis-spelt, over forward-compatibility.
#[derive(Debug, Deserialize, PartialEq)]
pub(crate) enum SalesforceProjectType {
    #[serde(rename = "function")]
    Function,
}

/// Errors that can occur when reading and parsing a `project.toml` file.
#[derive(Debug)]
pub(crate) enum ProjectDescriptorError {
    Io(io::Error),
    Parse(toml::de::Error),
}

#[cfg(test)]
mod tests {
    use super::*;
    use libcnb_test::assert_contains;

    #[test]
    fn deserialize_empty_descriptor() {
        assert_eq!(parse("").unwrap(), ProjectDescriptor { com: None });
    }

    #[test]
    fn deserialize_non_salesforce_descriptor() {
        let toml_str = r#"
            [_]
            schema-version = "0.2"

            [io.buildpacks]
            builder = "my-builder"

            [com.example]
            key = "value"
        "#;

        assert_eq!(
            parse(toml_str),
            Ok(ProjectDescriptor {
                com: Some(ComTable { salesforce: None })
            })
        );
    }

    #[test]
    fn deserialize_function_descriptor() {
        let toml_str = r#"
            [_]
            schema-version = "0.2"

            [com.salesforce]
            schema-version = "0.1"
            id = "example"
            description = "Example function"
            type = "function"
            salesforce-api-version = "56.0"
        "#;

        assert_eq!(
            parse(toml_str),
            Ok(ProjectDescriptor {
                com: Some(ComTable {
                    salesforce: Some(SalesforceTable {
                        project_type: SalesforceProjectType::Function
                    })
                })
            })
        );
    }

    #[test]
    fn deserialize_minimal_function_descriptor() {
        let toml_str = r#"
            [com.salesforce]
            type = "function"
        "#;

        assert_eq!(
            parse(toml_str),
            Ok(ProjectDescriptor {
                com: Some(ComTable {
                    salesforce: Some(SalesforceTable {
                        project_type: SalesforceProjectType::Function
                    })
                })
            })
        );
    }

    #[test]
    fn reject_salesforce_table_with_no_project_type() {
        let toml_str = r#"
            [com.salesforce]
            schema-version = "0.1"
            id = "example"
        "#;

        let error = parse(toml_str).unwrap_err();
        assert_contains!(error.to_string(), "missing field `type`");
    }

    #[test]
    fn reject_unknown_salesforce_project_type() {
        let toml_str = r#"
            [com.salesforce]
            type = "some_unknown_type"
        "#;

        let error = parse(toml_str).unwrap_err();
        assert_contains!(
            error.to_string(),
            "unknown variant `some_unknown_type`, expected `function`"
        );
    }

    #[test]
    fn read_project_descriptor_no_project_toml_file() {
        let app_dir = Path::new("tests/fixtures/empty");

        assert_eq!(read_project_descriptor(app_dir).unwrap(), None);
    }

    #[test]
    fn read_project_descriptor_non_salesforce() {
        let app_dir = Path::new("tests/fixtures/project_toml_non_salesforce");

        assert_eq!(
            read_project_descriptor(app_dir).unwrap(),
            Some(ProjectDescriptor {
                com: Some(ComTable { salesforce: None })
            })
        );
    }

    #[test]
    fn read_project_descriptor_function() {
        let app_dir = Path::new("tests/fixtures/salesforce_function_template");

        assert_eq!(
            read_project_descriptor(app_dir).unwrap(),
            Some(ProjectDescriptor {
                com: Some(ComTable {
                    salesforce: Some(SalesforceTable {
                        project_type: SalesforceProjectType::Function
                    })
                })
            })
        );
    }

    #[test]
    fn read_project_descriptor_invalid_project_toml_file() {
        let app_dir = Path::new("tests/fixtures/project_toml_invalid");

        assert!(matches!(
            read_project_descriptor(app_dir).unwrap_err(),
            ProjectDescriptorError::Parse(_)
        ));
    }

    #[test]
    fn get_salesforce_project_type_missing() {
        let app_dir = Path::new("tests/fixtures/empty");

        assert_eq!(read_salesforce_project_type(app_dir).unwrap(), None);
    }

    #[test]
    fn get_salesforce_project_type_non_salesforce() {
        let app_dir = Path::new("tests/fixtures/project_toml_non_salesforce");

        assert_eq!(read_salesforce_project_type(app_dir).unwrap(), None);
    }

    #[test]
    fn get_salesforce_project_type_function() {
        let app_dir = Path::new("tests/fixtures/salesforce_function_template");

        assert_eq!(
            read_salesforce_project_type(app_dir).unwrap(),
            Some(SalesforceProjectType::Function)
        );
    }

    #[test]
    fn get_salesforce_project_type_invalid_project_toml_file() {
        let app_dir = Path::new("tests/fixtures/project_toml_invalid");

        assert!(matches!(
            read_salesforce_project_type(app_dir).unwrap_err(),
            ProjectDescriptorError::Parse(_)
        ));
    }
}
