import os
from pathlib import Path

BASE_DIR = Path(__file__).resolve().parent.parent

INSTALLED_APPS = [
    "django.contrib.staticfiles",
    "testapp",
]

STATIC_ROOT = BASE_DIR / "staticfiles"
STATIC_URL = "static/"

# Tests that app env vars are passed to the 'manage.py' script invocations.
assert "EXPECTED_ENV_VAR" in os.environ
