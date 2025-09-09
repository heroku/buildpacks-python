import os
import sys
from pathlib import Path
from pprint import pprint

BASE_DIR = Path(__file__).resolve().parent.parent

INSTALLED_APPS = [
    "django.contrib.staticfiles",
    "testapp",
]

STATIC_ROOT = BASE_DIR / "staticfiles"
STATIC_URL = "static/"

# Older versions of Django require that `SECRET_KEY` is set when running collectstatic.
SECRET_KEY = "example"

env_vars = {
    k: v
    for k, v in os.environ.items()
    if not (k.startswith("CNB_") or k in {"HOME", "HOSTNAME"})
}
pprint(env_vars)
print()
pprint(sys.path)
