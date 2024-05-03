# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Updated setuptools from 68.0.0 to 69.5.1. ([#200](https://github.com/heroku/buildpacks-python/pull/200))
- Updated wheel from 0.42.0 to 0.43.0. ([#179](https://github.com/heroku/buildpacks-python/pull/179))
- The buildpack now implements Buildpack API 0.10 instead of 0.9, and so requires `lifecycle` 0.17.x or newer. ([#197](https://github.com/heroku/buildpacks-python/pull/197))
- The buildpack's base image compatibility metadata is now declared using `[[targets]]` instead of `[[stacks]]`. ([#197](https://github.com/heroku/buildpacks-python/pull/197))
- Changed compression format and S3 URL for Python runtime archives. ([#197](https://github.com/heroku/buildpacks-python/pull/197))

### Removed

- Removed support for Python 3.7. ([#197](https://github.com/heroku/buildpacks-python/pull/197))

## [0.8.4] - 2024-04-09

### Changed

- Updated the default Python version from 3.12.2 to 3.12.3. ([#189](https://github.com/heroku/buildpacks-python/pull/189))

## [0.8.3] - 2024-03-25

### Changed

- Updated pip from 23.3.2 to 24.0. ([#172](https://github.com/heroku/buildpacks-python/pull/172))

## [0.8.2] - 2024-02-07

### Changed

- Updated the default Python version from 3.12.1 to 3.12.2. ([#167](https://github.com/heroku/buildpacks-python/pull/167))

## [0.8.1] - 2024-01-11

### Changed

- Updated pip from 23.3.1 to 23.3.2. ([#156](https://github.com/heroku/buildpacks-python/pull/156))

## [0.8.0] - 2023-12-08

### Changed

- Updated the default Python version from 3.11.6 to 3.12.1. ([#152](https://github.com/heroku/buildpacks-python/pull/152) and [#154](https://github.com/heroku/buildpacks-python/pull/154))
- Updated wheel from 0.41.3 to 0.42.0. ([#150](https://github.com/heroku/buildpacks-python/pull/150))

## [0.7.3] - 2023-11-06

### Changed

- Updated wheel from 0.41.2 to 0.41.3. ([#137](https://github.com/heroku/buildpacks-python/pull/137))

## [0.7.2] - 2023-10-24

### Changed

- Updated pip from 23.2.1 to 23.3.1. ([#131](https://github.com/heroku/buildpacks-python/pull/131))
- Updated wheel from 0.41.0 to 0.41.2. ([#100](https://github.com/heroku/buildpacks-python/pull/100))
- Updated buildpack display name and description. ([#135](https://github.com/heroku/buildpack-python/pull/135))

## [0.7.1] - 2023-10-02

### Changed

- Updated the default Python version from 3.11.5 to 3.11.6. ([#121](https://github.com/heroku/buildpacks-python/pull/121))

## [0.7.0] - 2023-09-19

### Added

- Django's `collectstatic` command is now automatically run for Django apps that use static files. ([#108](https://github.com/heroku/buildpacks-python/pull/108))

## [0.6.0] - 2023-08-25

### Changed

- Updated the default Python version from 3.11.4 to 3.11.5. ([#101](https://github.com/heroku/buildpacks-python/pull/101))

### Removed

- Removed support for Salesforce Functions. ([#83](https://github.com/heroku/buildpacks-python/pull/83))

## [0.5.0] - 2023-07-24

### Changed

- User-provided environment variables are now propagated to subprocesses such as `pip install`. ([#65](https://github.com/heroku/buildpacks-python/pull/65))
- Updated pip from 23.1.2 to 23.2.1. ([#67](https://github.com/heroku/buildpacks-python/pull/67) and [#76](https://github.com/heroku/buildpacks-python/pull/76))
- Updated setuptools from 67.8.0 to 68.0.0. ([#51](https://github.com/heroku/buildpacks-python/pull/51))
- Updated wheel from 0.40.0 to 0.41.0. ([#78](https://github.com/heroku/buildpacks-python/pull/78))

## [0.4.0] - 2023-06-07

### Changed

- Updated the default Python version from 3.11.3 to 3.11.4. ([#45](https://github.com/heroku/buildpacks-python/pull/45))
- Updated setuptools from 67.7.2 to 67.8.0. ([#43](https://github.com/heroku/buildpacks-python/pull/43))
- Updated libcnb from 0.11.5 to 0.12.0. ([#35](https://github.com/heroku/buildpacks-python/pull/35))
- The buildpack now implements Buildpack API 0.9 instead of 0.8, and so requires `lifecycle` 0.15.x or newer. ([#35](https://github.com/heroku/buildpacks-python/pull/35))

## [0.3.0] - 2023-04-27

### Changed

- Updated pip from 23.0.1 to 23.1.2. ([#31](https://github.com/heroku/buildpacks-python/pull/31))
- Updated setuptools from 67.6.1 to 67.7.2. ([#30](https://github.com/heroku/buildpacks-python/pull/30))

## [0.2.0] - 2023-04-11

### Changed

- Updated the default Python version from 3.11.2 to 3.11.3. ([#22](https://github.com/heroku/buildpacks-python/pull/22))
- Updated setuptools from 67.5.0 to 67.6.1. ([#24](https://github.com/heroku/buildpacks-python/pull/24))
- Updated wheel from 0.38.4 to 0.40.0. ([#24](https://github.com/heroku/buildpacks-python/pull/24))

### Fixed

- The `PYTHONHOME` environment variable is now set, to work around uWSGI not handling relocated Python installs correctly. ([#25](https://github.com/heroku/buildpacks-python/pull/25))

## [0.1.0] - 2023-03-06

### Added

- Initial implementation. ([#3](https://github.com/heroku/buildpacks-python/pull/3))

[unreleased]: https://github.com/heroku/buildpacks-python/compare/v0.8.4...HEAD
[0.8.4]: https://github.com/heroku/buildpacks-python/compare/v0.8.3...v0.8.4
[0.8.3]: https://github.com/heroku/buildpacks-python/compare/v0.8.2...v0.8.3
[0.8.2]: https://github.com/heroku/buildpacks-python/compare/v0.8.1...v0.8.2
[0.8.1]: https://github.com/heroku/buildpacks-python/compare/v0.8.0...v0.8.1
[0.8.0]: https://github.com/heroku/buildpacks-python/compare/v0.7.3...v0.8.0
[0.7.3]: https://github.com/heroku/buildpacks-python/compare/v0.7.2...v0.7.3
[0.7.2]: https://github.com/heroku/buildpacks-python/compare/v0.7.1...v0.7.2
[0.7.1]: https://github.com/heroku/buildpacks-python/compare/v0.7.0...v0.7.1
[0.7.0]: https://github.com/heroku/buildpacks-python/compare/v0.6.0...v0.7.0
[0.6.0]: https://github.com/heroku/buildpacks-python/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/heroku/buildpacks-python/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/heroku/buildpacks-python/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/heroku/buildpacks-python/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/heroku/buildpacks-python/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/heroku/buildpacks-python/releases/tag/v0.1.0
