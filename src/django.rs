use crate::FileExistsError;
use crate::utils::{self, CapturedCommandError, StreamedCommandError};
use indoc::indoc;
use libcnb::Env;
use libherokubuildpack::log::log_info;
use std::path::Path;
use std::process::Command;

const MANAGEMENT_SCRIPT_NAME: &str = "manage.py";

pub(crate) fn is_django_installed(dependencies_layer_dir: &Path) -> Result<bool, FileExistsError> {
    // The Django package includes a `django-admin` entrypoint script, which we can use
    // as a simple/fast way to check if Django is installed.
    utils::file_exists(&dependencies_layer_dir.join("bin/django-admin"))
}

pub(crate) fn run_django_collectstatic(
    app_dir: &Path,
    env: &Env,
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

    if !has_collectstatic_command(app_dir, env)
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
            // Note: We can't use `--link` since it doesn't work with remote storage backends (eg S3).
            .args([
                MANAGEMENT_SCRIPT_NAME,
                "collectstatic",
                // Using `--noinput` instead of `--no-input` since the latter requires Django 1.9+.
                "--noinput",
            ])
            .current_dir(app_dir)
            .env_clear()
            .envs(env),
    )
    .map_err(DjangoCollectstaticError::CollectstaticCommand)
}

fn has_management_script(app_dir: &Path) -> Result<bool, FileExistsError> {
    utils::file_exists(&app_dir.join(MANAGEMENT_SCRIPT_NAME))
}

fn has_collectstatic_command(app_dir: &Path, env: &Env) -> Result<bool, CapturedCommandError> {
    utils::run_command_and_capture_output(
        Command::new("python")
            .args([MANAGEMENT_SCRIPT_NAME, "help", "collectstatic"])
            .current_dir(app_dir)
            .env_clear()
            .envs(env),
    )
    .map_or_else(
        |error| match error {
            // We need to differentiate between the command not existing (due to the staticfiles app
            // not being installed) and the Django config or mange.py script being broken. Ideally
            // we'd inspect the output of `manage.py help --commands` but that command unhelpfully
            // exits zero even if the app's `DJANGO_SETTINGS_MODULE` wasn't a valid module.
            // Note: Django incorrectly outputs "Unknown command" if the Django config is invalid
            // when using Django 1.10 and older, meaning any invalid config is silently ignored,
            // however, those Django versions are EOL and there isn't anything we can do about it.
            CapturedCommandError::NonZeroExitStatus(output)
                if String::from_utf8_lossy(&output.stderr).contains("Unknown command") =>
            {
                Ok(false)
            }
            _ => Err(error),
        },
        |_| Ok(true),
    )
}

/// Errors that can occur when running the Django collectstatic command.
#[derive(Debug)]
pub(crate) enum DjangoCollectstaticError {
    CheckCollectstaticCommandExists(CapturedCommandError),
    CheckManagementScriptExists(FileExistsError),
    CollectstaticCommand(StreamedCommandError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn has_management_script_django_project() {
        assert!(
            has_management_script(Path::new("tests/fixtures/django_staticfiles_latest_django"))
                .unwrap()
        );
    }

    #[test]
    fn has_management_script_empty() {
        assert!(!has_management_script(Path::new("tests/fixtures/empty")).unwrap());
    }

    #[test]
    fn has_management_script_io_error() {
        // We pass a path containing a NUL byte as an easy way to trigger an I/O error.
        let err = has_management_script(Path::new("\0/invalid")).unwrap_err();
        assert_eq!(err.path, Path::new("\0/invalid/manage.py"));
    }
}
