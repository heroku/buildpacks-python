use crate::tests::builder;
use indoc::indoc;
use libcnb_test::{assert_contains, assert_empty, BuildConfig, PackResult, TestRunner};

// This test uses symlinks for requirements.txt and manage.py to confirm that it's possible to use
// them when the Django app is nested inside a subdirectory (such as in backend+frontend monorepos).
#[test]
#[ignore = "integration test"]
fn django_staticfiles_latest_django() {
    TestRunner::default().build(
        BuildConfig::new(builder(), "tests/fixtures/django_staticfiles_latest_django")
            // Tests that env vars are passed to the 'manage.py' script invocations.
            .env("EXPECTED_ENV_VAR", "1"),
        |context| {
            assert_empty!(context.pack_stderr);
            assert_contains!(
                context.pack_stdout,
                indoc! {"
                    [Generating Django static files]
                    Running 'manage.py collectstatic'
                    
                    1 static file symlinked to '/workspace/backend/staticfiles'.
                "}
            );
        },
    );
}

// This tests the oldest Django version that works on Python 3.9 (which is the
// oldest Python that is available on all of our supported builders).
#[test]
#[ignore = "integration test"]
fn django_staticfiles_legacy_django() {
    TestRunner::default().build(
        BuildConfig::new(builder(), "tests/fixtures/django_staticfiles_legacy_django"),
        |context| {
            assert_empty!(context.pack_stderr);
            assert_contains!(
                context.pack_stdout,
                indoc! {"
                    Successfully installed Django-1.8.19
                    
                    [Generating Django static files]
                    Running 'manage.py collectstatic'
                    Linking '/workspace/testapp/static/robots.txt'
                    
                    1 static file symlinked to '/workspace/staticfiles'.
                "}
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn django_no_manage_py() {
    TestRunner::default().build(
        BuildConfig::new(builder(), "tests/fixtures/django_no_manage_py"),
        |context| {
            assert_empty!(context.pack_stderr);
            assert_contains!(
                context.pack_stdout,
                indoc! {"
                    [Generating Django static files]
                    Skipping automatic static file generation since no Django 'manage.py'
                    script (or symlink to one) was found in the root directory of your
                    application.
                "}
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn django_staticfiles_app_not_enabled() {
    TestRunner::default().build(
        BuildConfig::new(
            builder(),
            "tests/fixtures/django_staticfiles_app_not_enabled",
        ),
        |context| {
            assert_empty!(context.pack_stderr);
            assert_contains!(
                context.pack_stdout,
                indoc! {"
                    [Generating Django static files]
                    Skipping automatic static file generation since the 'django.contrib.staticfiles'
                    feature is not enabled in your app's Django configuration.
                "}
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn django_invalid_settings_module() {
    TestRunner::default().build(
        BuildConfig::new(builder(), "tests/fixtures/django_invalid_settings_module")
            .expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stdout,
                indoc! {"
                    [Generating Django static files]
                "}
            );
            assert_contains!(
                context.pack_stderr,
                indoc! {"
                    [Error: Unable to inspect Django configuration]
                    The 'python manage.py help collectstatic' Django management command
                    (used to check whether Django's static files feature is enabled)
                    failed (exit status: 1).
                    
                    Details:
                    
                    Traceback (most recent call last):
                "}
            );
            // Full traceback omitted since it will change across Django/Python versions causing test churn.
            assert_contains!(
                context.pack_stderr,
                indoc! {"
                    ModuleNotFoundError: No module named 'nonexistent-module'
                    
                    
                    This indicates there is a problem with your application code or Django
                    configuration. Try running the 'manage.py' script locally to see if the
                    same error occurs.
                "}
            );
        },
    );
}

#[test]
#[ignore = "integration test"]
fn django_staticfiles_misconfigured() {
    TestRunner::default().build(
        BuildConfig::new(builder(), "tests/fixtures/django_staticfiles_misconfigured")
            .expected_pack_result(PackResult::Failure),
        |context| {
            assert_contains!(
                context.pack_stdout,
                indoc! {"
                    [Generating Django static files]
                    Running 'manage.py collectstatic'
                "}
            );
            assert_contains!(
                context.pack_stderr,
                indoc! {"
                    [Error: Unable to generate Django static files]
                    The 'python manage.py collectstatic --link --noinput' Django management
                    command to generate static files failed (exit status: 1).
                    
                    This is most likely due an issue in your application code or Django
                    configuration. See the log output above for more information.
                    
                    If you are using the WhiteNoise package to optimize the serving of static
                    files with Django (recommended), check that your app is using the Django
                    config options shown here:
                    https://whitenoise.readthedocs.io/en/stable/django.html

                    Or, if you do not need to use static files in your app, disable the
                    Django static files feature by removing 'django.contrib.staticfiles'
                    from 'INSTALLED_APPS' in your app's Django configuration.
                "}
            );
        },
    );
}
