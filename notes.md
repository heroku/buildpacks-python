## Python package resolution
What are all of the ways packages end up on sys.path? And in what order?
-> Script/current dir, PYTHONPATH, user site-packages (incl `.pth` and `usercustomize`), system site-packages (incl `.pth` and `sitecustomize`)
Can system site-packages location be overridden?
-> Not really, since needs to be same as libs etc
Can user site-packages be overridden?
-> Yes, using `PYTHONUSERBASE`
What deps do Pip, Poetry and pipenv have? Can the tools be installed outside of the env they are managing?
-> Pip: None (it vendors). Managing deps outside of a venv is not supported (other than `--target` and perhaps `--prefix`). See: https://github.com/pypa/pip/issues/5472
-> Poetry: lots! However installing in a venv is both supported and recommended.
-> Pipenv: lots! However installing in a venv is supported and kinda recommended.
Is pip needed when installing Poetry/pipenv?
-> Poetry: Yes
-> Pipenv: Yes
What deps will the Python invoker have? (ie can cause conflicts) Or fully vendored / in Rust?
-> TBD
How do user installs work when there are conflicting dependencies? Can they be used inside a virtualenv?
-> Seems to work well. And no, can't be used in a venv. See https://pip.pypa.io/en/stable/user_guide/#user-installs
What approaches do other CNBs use?
-> GCP: Other buildpacks put their `requirements.txt` files into the build plan and then a single pip-install CNB installs it. Prior to that that they tried using `--prefix` and `--target` with `PYTHONPATH`. They cannot use `PYTHONUSERBASE` fully due to compatibility issues with their GAE image using system Python and having to use virtualenvs (which don't support user installs).
-> Paketo: `PYTHONUSERBASE` to set install location during pip install of pip/deps, but then `PYTHONPATH` afterwards. They used to use `PYTHONUSERBASE` for both but changed in https://github.com/paketo-buildpacks/pip-install/pull/58 to "allow other buildpacks to use `PYTHONUSERBASE`" -- seems like they perhaps haven't realised about the `PYTHONPATH` shadowing stdlib issues?
What are the issues with using `--target` and `--prefix` that meant GCP stopped using them?
-> https://github.com/GoogleCloudPlatform/buildpacks/commit/7768ebe4d5f300598b86328f607eeb70ab7b7131
-> https://github.com/GoogleCloudPlatform/buildpacks/commit/410b552aba55404bdb45acb638112feb271de01f
-> https://github.com/GoogleCloudPlatform/buildpacks/commit/b93391cd653eef7336bc154466fa6d3de4ed337b
-> https://github.com/pypa/pip/issues/8799
So what are the alternatives for where to install packages?
-> New venv (w/wo Pip / system site-packages)
-> Arbitrary directory and point at it with `PYTHONPATH`
-> Arbitrary directory used as user install location with `PYTHONUSERBASE`
-> System site-packages in same layer as Python runtime
-> Arbitrary directory and point at it with `.pth` file from user/system site-packages
Resources:
https://peps.python.org/pep-0370/
https://docs.python.org/3.10/library/site.html
https://docs.python.org/3.10/install/index.html#alternate-installation
https://docs.python.org/3.10/using/cmdline.html#envvar-PYTHONNOUSERSITE
https://docs.python.org/3.10/using/cmdline.html#envvar-PYTHONPATH
https://docs.python.org/3.10/library/sys.html#sys.path
https://docs.python.org/3.10/library/sysconfig.html#installation-paths
https://docs.python.org/3.11/library/sys_path_init.html#sys-path-init

## Installation locations
- Pip/setuptools/wheel: System site-packages in same layer as Python runtime
- Poetry/Pipenv (if applicable): Venv using `--symlinks --system-site-packages --without-pip` (using `--without-pip` saves ~8.5 MB and 1.6s on macOS). Must install using `python -m pip`.
- App dependencies: User site-packages
- Function invoker (if in Python): Arbitrary directory added to `PYTHONPATH` or make the user install

## Installing dependencies with pip
- Do we support having no package manager being used?
-> TBD
- Does a single layer handle all install types, or separate layer per package manager?
-> Separate
- When to cache/invalidate site-packages?
-> Invalidation needed to clean up removed packages (otherwise have to manually remove), and ensure unpinned deps are updated (if not using --upgrade)
- Should the pip cache also be cached? If so, when to invalidate that?
-> Helps when cached site-packages invalidated, or if a previously used package added back
- Should we use `--upgrade`?
-> Pros: Ensures unpinned deps stay up to date. Might mean we don't need to invalidate site-packages as often.
-> Cons: Causes pip to still query PyPI even for `==` deps.
-> Are people using `--upgrade` locally?
- What is the perf impact of caching site-packages vs pip cache? What about `--upgrade`?
- Options: `pip install --user --disable-pip-version-check --cache-dir <DIR> --no-input`
- What about `requirements.txt` files with an include?
- Do we need to use `--exists-action`?
- No way to purge pip cache of items older than X (https://github.com/pypa/pip/issues/8355)

curl -O https://raw.githubusercontent.com/mozilla/treeherder/master/requirements/common.txt
rm -rf venv /root/.cache/pip/ && python -m venv --symlink venv && time venv/bin/pip install --disable-pip-version-check -r common.txt -q --no-cache-dir

## When does site-packages need invalidating?
- Python version changed (any, or just major?)
-> Yes, perhaps any?
- Stack changed
-> Yes
- Pip/setuptools/wheel version changed?
-> Don't think so
- requirements.txt changes

## Should we use `--upgrade` or `--upgrade --upgrade-strategy eager`?
- Pros:
  - Means updated versions of unpinned packages (or unspecified transitive deps) are pulled in (without invalidating site-packages)
  - Means pip logs show what changed (vs invalidating site-packages)
- Cons:
  - Pip still queries PyPI for `==` pinned deps, slowing otherwise no-op runs.
  - If an updated package drops a dep, then that dep isn't uninstalled (vs invalidating site-packages).
  - Using `--upgrade --upgrade-strategy eager` results in errors for projects using hashes where a dependency has a transitive dep on setuptools (such as gunicorn)
- Other:
  - Updates are pulled in immediately rather than after a delay
  - Does `--upgrade` match what people are using locally?
  - Does pip handle transitive dep updates any differently from empty site-packages?

## Should we invalidate on root requirements.txt changes
- Yes! Have to otherwise package removals don't work.

## What isn't handled when invalidating on root requirements.txt changes when not using `--upgrade`?
- Updated versions of unpinned packages (or unspecified transitive deps) are not pulled in
- Removals from transitive requirements.txt files (unless we scan for those too)
- Explicit package updates that drop a dep, in transitive requirements.txt files (unless we scan for those too)

## What isn't handled when invalidating on root requirements.txt changes when using `--upgrade`?
- If an implicitly updated package drops a dep, then that dep isn't uninstalled (vs invalidating site-packages).
- Removals from transitive requirements.txt files (unless we scan for those too)
- Explicit package updates that drop a dep, in transitive requirements.txt files (unless we scan for those too)

## How could we handle transitive requirements.txt files?
- Scan root requirements.txt for `-r ...` usages and check for changes to those too
- Output a warning if `-r ...` usages found and encourage users to stop using them or switch to eg Poetry
- Offer alternative locations to just the repo root, hoping people would use those instead of includes? (But doesn't cover all use-cases eg common deps)

## Timings for treeherder's common.txt (Python 3.9, in venv, wheel installed, --disable-pip-version-check)
- Clean install, --no-cache-dir: 37.3s
- Clean install, cold cache: 37.8s
- Clean install, warm cache (all): 33.7s (however zstandard cached built wheel not used due to hashes)
- No-op repeat install, --no-cache, no upgrade: 0.61s
- No-op repeat install, warm cache, no upgrade: 0.61s
- No-op repeat install, --no-cache, --upgrade: 3.3s
- No-op repeat install, warm cache, --upgrade: 3.3s

## Timings for treeherder's common.txt with hashes removed (Python 3.9, in venv, wheel installed, --disable-pip-version-check)
- Clean install, --no-cache-dir: 37.8s
- Clean install, cold cache: 37.8s
- Clean install, warm cache (all): 9.0s (without wheel installed this increases to 12.9s)
- Clean install, warm cache (3 MB wheel dir only): 12.8s
- Clean install, warm cache (72 MB http dir only): 33.9s

## Timings for getting-started-guide's requirements.txt (Python 3.9, in venv, wheel installed, --disable-pip-version-check)
- Clean install, --no-cache-dir: 5.6s
- Clean install, cold cache: 5.7s
- Clean install, warm cache (all): 1.4s
- Clean install, warm cache (0.5 MB wheel dir only): 1.9s
- Clean install, warm cache (8.7 MB http dir only): 5.1s
- No-op repeat install, warm cache, no upgrade: 0.28s

## Pip cache conclusions
- Wheel generation is where most of the time is spent (on a fast connection at least)
- If caching pip cache must have wheel installed or wheels won't be cached properly
- Could just cache wheels directory of pip cache since fraction of the size for most of the benefit. But wouldn't help slow connections.
- Invalidating site-packages increases install time from: 0.25s -> 1.4s (small project), 0.6s -> 9s (large project), 0.6s -> 34s (large project using hashes)
- Invalidating pip cache too increases install time from: 1.4s -> 5.7s (small project), 9s -> 38s (large project), 34s -> 38s (large project using hashes)
- Pip hashes really impact caching - should we output a warning?

## Possible layer invalidation conditions
- Python version (either only when the major version changes, or also including minor version changes)
- Stack
- pip/setuptools/wheel version
- Poetry/pipenv version
- Input files from app (eg requirements.txt/Poetry.lock hash)
- Time since layer created
- Buildpack changes that aren't backwards compatible with old caches

## Layer scenarios
- Initial install: `build()` -> `create()`
- Keeping cached layer: `build()` -> `existing_layer_strategy()`
- Recreating cached layer: `build()` -> `existing_layer_strategy()` -> `create()`
- Updating cached layer: `build()` -> `existing_layer_strategy()` -> `update()`

## Logging
- What do users care about in the logs?
  - If something went wrong, what it was, whether it was their fault or not, and how to resolve
  - What is happening in general, so it doesn't seem like a black box
  - How behaviour can be customised
  - Why has behaviour changed since last build, particularly if something is now broken.
- When to use headings vs not?
- Should there always be a "doing thing" and "finished thing" message or just one or the other?
- How verbose should the logs be (particularly for output from subprocesses)?
- Should the verbosity be user controllable? Should we ask for a standard env var upstream?
- What should the logs show for using cache vs invalidating cache?

## Errors
- Remove unwraps throughout and replace with new error enum variants
- How fine grained should the io::Error instances be?
- should layer errors be flattened into the top level buildpack error enum, or have their own error enums?
- Should the error `From` implementations live with the error enums (eg in the layer), or in errors.rs?
- What if anything should be covered by retries? Presumably only things involving network I/O? How well do pip's retries work?

## Misc
- Utils for calling subprocesses
- Clear the env when calling subprocesses too (for most of them at least)
- What logic lives in the layer vs outside?
- Need to make Procfile mandatory given no default entrypoint. Although don't want to fail detect?
- Should set User Agent on outbound network requests
- Should we use https://docs.gunicorn.org/en/stable/settings.html#preload-app by default?

## Unit tests
- What things do/don't need a unit test?
- Should the unit test cover lower down functions or their parents?

## Integration tests
- Check Python static library works
- Check behaviour if buildpack run twice

## Poetry
- Should it use a different layer name for the `site-packages` layer?

## Improvements/decisions deferred to the future
- SHA256 checking of Python download.
- Decide whether to move pip/setuptools/wheel requirements to a requirements file so Dependabot can update them.
  - However then means it's harder for us to list versions.
  - Also, if integration tests include versions in log output and it's hardcoded, then Dependabot PRs will need manual updates anyway.
- Decide whether to use hashes for pip/setuptools/wheel requirements.

## Python version support
- Do we support "3.*" / "*"", or just "3.x.*"?
- Do we support major version syntax in runtime.txt?
- Which of these other formats do we support?
  - pyproject.toml's project.requires-python
  - a new pyproject.toml table/property
  - .python-version (with or w/o major version support?)
  - tool.poetry.dependencies.python in pyproject.toml
  - CNB project.toml file

### pyproject.toml
[project]
requires-python = ">=3.8"
requires-python = "~=3.8" (means >=3.8, <4.0)
requires-python = "~=3.8.2" (means >=3.8.2, <3.9)
requires-python = "==3.8" (means ==3.8.0)
requires-python = "==3.8.*"
https://www.python.org/dev/peps/pep-0621/#requires-python
https://www.python.org/dev/peps/pep-0440/#version-specifiers
~=: Compatible release clause
==: Version matching clause
!=: Version exclusion clause
<=, >=: Inclusive ordered comparison clause
<, >: Exclusive ordered comparison clause
===: Arbitrary equality clause.

### pyproject.toml
[tool.poetry.dependencies]
python = "^3.9"

### .python-version
X.Y.Z
didn't used to support X.Y unless using a plugin, but now does: https://github.com/pyenv/pyenv#prefix-auto-resolution

# pyc locations
- python stdlib
- pip/setuptools/wheel install in system site-packages
- app dependencies installed by pip in user site-packages
- poetry install in venv
- app dependencies installed by poetry in user site-packages
- app python files themselves in app dir

# pyc alternatives
- timestamp (default)
- checked hash by disabling automatic compileall then running manually
- checked hash by setting SOURCE_DATE_EPOCH (only works via py_compile not by just running)
- unchecked hash by disabling automatic compileall then running manually
- delete the pyc files and let them be generated at build and/or app boot

# pyc timings
- `python:3-slim`, native, `pip --version`, no pycs (creating timestamp): 0.628s
- `python:3-slim`, native, `pip --version`, no pycs (creating none): 0.571s
- `python:3-slim`, native, `pip --version`, existing timestamp: 0.151s
- `python:3-slim`, native, `pip --version`, existing checked: 0.161s
- `python:3-slim`, native, `pip --version`, existing unchecked: 0.152s
- `python:3-slim`, native, compileall pip dir, timestamp: 0.565s
- `python:3-slim`, native, compileall site-packages, checked: 0.637s
- `python:3-slim`, native, compileall site-packages, checked, workers=0: 0.199s
- `python:3-slim`, native, compileall python lib dir, timestamp: 1.277s
- `python:3-slim`, native, compileall python lib dir, checked: 1.275s
- `python:3-slim`, native, compileall python lib dir, checked, workers=0: 0.423s
- `python:3-slim`, qemu, `pip --version`, no pycs (creating timestamp): 5.475s
- `python:3-slim`, qemu, `pip --version`, no pycs (creating none): 5.357s
- `python:3-slim`, qemu, `pip --version`, existing timestamp: 1.360s
- `python:3-slim`, qemu, `pip --version`, existing checked: 1.386s
- `python:3-slim`, qemu, `pip --version`, existing unchecked: 1.356s
- `python:3-slim`, qemu, compileall pip dir, timestamp: 4.883s
- `python:3-slim`, qemu, compileall pip dir, checked: 4.869s
- `python:3-slim`, qemu, compileall python lib dir, timestamp: 11.682s
- `python:3-slim`, qemu, compileall python lib dir, checked: 11.708s
- `python:3-slim`, qemu, compileall python lib dir, checked, workers=0: 3.436s
- heroku gsg-ci, Perf-M, `pip --version`, existing timestamp: 0.202s
- heroku gsg-ci, Perf-M, `pip --version`, existing checked: 0.211s
- heroku gsg-ci, Perf-M, `pip --version`, existing unchecked: 0.202s
- heroku gsg-ci, Perf-M, `manage.py check`, existing timestamp: 0.283s
- heroku gsg-ci, Perf-M, `manage.py check`, existing checked: 0.299s
- heroku gsg-ci, Perf-M, `manage.py check`, existing unchecked: 0.282s

Tested using:

```
find /app/.heroku/python/lib/python3.10/ -depth -type f -name "*.pyc" -delete
time python -m compileall -qq --invalidation-mode timestamp /app/.heroku/python/lib/python3.10/
time python -m compileall -qq --invalidation-mode checked-hash /app/.heroku/python/lib/python3.10/
time python -m compileall -qq --invalidation-mode unchecked-hash /app/.heroku/python/lib/python3.10/
```

```
find /usr/local -depth -type f -name "*.pyc" -delete
time python -m compileall -qq --invalidation-mode timestamp /usr/local/lib/python3.10/
time python -m compileall -qq --invalidation-mode checked-hash /usr/local/lib/python3.10/
time python -m compileall -qq --invalidation-mode unchecked-hash /usr/local/lib/python3.10/
while true; do time pip --version; done
export SOURCE_DATE_EPOCH=1
```

# Summary of runtime perf impact of checked vs unchecked pycs
- Native Docker, pip --version: +9ms on 152ms = +5.9%
- QEMU Docker, pip --version: +30ms on 1,356ms = +2.2%
- Heroku, pip --version: +9ms on 202ms = +4.5%
- Heroku, gsg manage.py check: +17ms on 282ms = +6.0%

# pyc conclusion
- For Python runtime archive: delete all pycs, then regenerate using unchecked-hash
- For pip/setuptools/wheel: install using --no-compile, generate using unchecked-hash + concurrency
- For app dependencies installed using pip, either:
  - Install using --no-compile, generate using unchecked-hash + concurrency
  - Install using --no-compile, generate using checked-hash + concurrency
  - Install normally, but ensure checked-hash by setting SOURCE_DATE_EPOCH
- For app dependencies installed using poetry (which doesn't support --no-compile), either:
  - Install normally, but ensure checked-hash by setting SOURCE_DATE_EPOCH
  - Install normally, then regenerate using unchecked-hash + concurrency
  - Install normally, then regenerate using checked-hash + concurrency

# bundled pip timings
- Bundled pip qemu: 5.2s for `--version`
- Bundled pip native: 0.6s for `--version`
- Unpacked pip qemu, without pycs: 3.3s for `--version`
- Unpacked pip native, without pycs: 0.4s for `--version`
- Unpacked pip qemu, with pycs: 1.4s for `--version`
- Unpacked pip native, with pycs: 0.2s for `--version`

// before:
// time until pip install completed: 14.65s
// time until all completed (incl pycs): 16.65s
// after:
// time until pip install completed: 9.15s
// time until all completed (incl pycs): 11.15s
