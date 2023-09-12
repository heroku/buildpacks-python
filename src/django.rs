use crate::utils::{self, CapturedCommandError, StreamedCommandError};
use indoc::indoc;
use libcnb::Env;
use libherokubuildpack::log::log_info;
use std::io;
use std::path::Path;
use std::process::Command;

const MANAGEMENT_SCRIPT_NAME: &str = "manage.py";

pub(crate) fn is_django_installed(dependencies_layer_dir: &Path) -> io::Result<bool> {
    dependencies_layer_dir.join("bin/django-admin").try_exists()
}

pub(crate) fn run_django_collectstatic(
    app_dir: &Path,
    command_env: &Env,
) -> Result<(), DjangoCollectstaticError> {
    if !has_management_script(app_dir)
        .map_err(DjangoCollectstaticError::CheckManagementScriptExists)?
    {
        log_info(indoc! {"
            Skipping automatic static file generation since no Django 'manage.py'
            script (or symlink to one) was found in the root directory of your
            application."
        });
        return Ok(());
    }

    if !has_collectstatic_command(app_dir, command_env)
        .map_err(DjangoCollectstaticError::CheckCollectstaticCommandExists)?
    {
        log_info(indoc! {"
            Skipping automatic static file generation since the 'django.contrib.staticfiles'
            feature is not enabled in your app's Django configuration."
        });
        return Ok(());
    }

    log_info("Running 'manage.py collectstatic'");
    utils::run_command_and_stream_output(
        Command::new("python")
            .args([
                MANAGEMENT_SCRIPT_NAME,
                "collectstatic",
                "--link",
                // Using `--noinput` instead of `--no-input` since the latter requires Django 1.9+.
                "--noinput",
            ])
            .current_dir(app_dir)
            .env_clear()
            .envs(command_env),
    )
    .map_err(DjangoCollectstaticError::CollectstaticCommand)
}

fn has_management_script(app_dir: &Path) -> io::Result<bool> {
    app_dir.join(MANAGEMENT_SCRIPT_NAME).try_exists()
}

fn has_collectstatic_command(
    app_dir: &Path,
    command_env: &Env,
) -> Result<bool, CapturedCommandError> {
    utils::run_command_and_capture_output(
        Command::new("python")
            .args([MANAGEMENT_SCRIPT_NAME, "help", "collectstatic"])
            .current_dir(app_dir)
            .env_clear()
            .envs(command_env),
    )
    .map_or_else(
        |err| match err {
            // We need to differentiate between the command not existing (due to the staticfiles app
            // not being installed) and the Django config or mange.py script being broken. Ideally
            // we'd inspect the output of `manage.py help --commands` but that command unhelpfully
            // exits zero even if the app's `DJANGO_SETTINGS_MODULE` wasn't a valid module.
            CapturedCommandError::NonZeroExitStatus(output)
                if String::from_utf8_lossy(&output.stderr).contains("Unknown command") =>
            {
                Ok(false)
            }
            _ => Err(err),
        },
        |_| Ok(true),
    )
}

/// Errors that can occur when running the Django collectstatic command.
#[derive(Debug)]
pub(crate) enum DjangoCollectstaticError {
    CheckCollectstaticCommandExists(CapturedCommandError),
    CheckManagementScriptExists(io::Error),
    CollectstaticCommand(StreamedCommandError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_management_script_django_project() {
        assert!(has_management_script(Path::new("tests/fixtures/django_collectstatic")).unwrap());
    }

    #[test]
    fn has_management_script_empty() {
        assert!(!has_management_script(Path::new("tests/fixtures/empty")).unwrap());
    }

    #[test]
    fn has_management_script_io_error() {
        assert!(has_management_script(Path::new("tests/fixtures/empty/.gitkeep")).is_err());
    }
}
