use crate::project_descriptor::{self, ReadProjectDescriptorError, SalesforceProjectType};
use libcnb::data::launch::{Launch, LaunchBuilder, ProcessBuilder};
use libcnb::data::process_type;
use libcnb::Env;
use std::io;
use std::path::Path;
use std::process::{Command, Output};

pub const FUNCTION_RUNTIME_PROGRAM_NAME: &str = "sf-functions-python";

// TODO: Decide default number of workers.
const SERVE_SUBCOMMAND: &str = "serve --host 0.0.0.0 --port \"${PORT:-8080}\" --workers 4 .";

/// Detect whether the specified project directory is that of a Salesforce Function.
///
/// Returns `Ok(true)` if the specified project directory contains a `project.toml` file with a
/// `com.salesforce.type` of "function".
///
/// It is permitted for the `project.toml` file not to exist, or for there to be no `com.salesforce`
/// TOML table within the file, in which case `Ok(false)` will be returned.
///
/// However, an error will be returned if any other IO error occurred, if the `project.toml` file
/// is not valid TOML, or the TOML document does not adhere to the schema.
pub(crate) fn is_function_project(app_dir: &Path) -> Result<bool, ReadProjectDescriptorError> {
    project_descriptor::read_salesforce_project_type(app_dir)
        .map(|project_type| project_type == Some(SalesforceProjectType::Function))
}

/// Validate the function using the `sf-functions-python check` command.
// TODO: Add support for checking the function meets a minimum version, like the CLI does:
// - Explore pros/cons of version command vs looking up package version.
// - Version command failure cases: Not found / io error / exit code / invalid version (unparsable) / too old version
// TODO: Should we output the version of the salesforce-functions package in the CNB build, locally, at runtime etc?
// TODO: Should we inform that a new version is available, as a less strict complement to the minimum version?
pub(crate) fn check_function(env: &Env) -> Result<(), CheckFunctionError> {
    // Not using `utils::run_command` since we want to capture output and only
    // display it if the check command fails.
    Command::new(FUNCTION_RUNTIME_PROGRAM_NAME)
        .args(["check", "."])
        .envs(env)
        .output()
        .map_err(|io_error| match io_error.kind() {
            io::ErrorKind::NotFound => CheckFunctionError::ProgramNotFound,
            _ => CheckFunctionError::Io(io_error),
        })
        .and_then(|output| {
            if output.status.success() {
                Ok(())
            } else {
                Err(CheckFunctionError::NonZeroExitStatus(output))
            }
        })
}

/// Generate a `launch.toml` configuration for running Python Salesforce Functions.
///
/// Runs the `sf-functions-python serve` command with suitable options for production.
pub(crate) fn launch_config() -> Launch {
    LaunchBuilder::new()
        .process(
            // TODO: Stop running via bash once direct processes support env var interpolation:
            // https://github.com/buildpacks/rfcs/issues/258
            ProcessBuilder::new(process_type!("web"), "bash")
                .args([
                    "-c",
                    &format!("exec {FUNCTION_RUNTIME_PROGRAM_NAME} {SERVE_SUBCOMMAND}"),
                ])
                .default(true)
                .direct(true)
                .build(),
        )
        .build()
}

/// Errors that can occur when running the `sf-functions-python check` command.
#[derive(Debug)]
pub(crate) enum CheckFunctionError {
    Io(io::Error),
    NonZeroExitStatus(Output),
    ProgramNotFound,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_function_project_no_project_toml() {
        let app_dir = Path::new("test-fixtures/empty");

        assert!(!is_function_project(app_dir).unwrap());
    }

    #[test]
    fn is_function_project_non_salesforce_project_toml() {
        let app_dir = Path::new("test-fixtures/project_toml_non_salesforce");

        assert!(!is_function_project(app_dir).unwrap());
    }

    #[test]
    fn is_function_project_function_project_toml() {
        let app_dir = Path::new("test-fixtures/function_template");

        assert!(is_function_project(app_dir).unwrap());
    }

    #[test]
    fn is_function_project_invalid_project_toml() {
        let app_dir = Path::new("test-fixtures/project_toml_invalid");

        assert!(matches!(
            is_function_project(app_dir).unwrap_err(),
            ReadProjectDescriptorError::Parse(_)
        ));
    }
}