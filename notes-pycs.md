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
export PYTHONDONTWRITEBYTECODE=1
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

# Questions:
- Does --compile only affect wheels, or sdists too?
