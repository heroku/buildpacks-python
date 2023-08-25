# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

[unreleased]: https://github.com/heroku/buildpacks-python/compare/v0.6.0...HEAD
[0.6.0]: https://github.com/heroku/buildpacks-python/compare/v0.5.0...v0.6.0
[0.5.0]: https://github.com/heroku/buildpacks-python/compare/v0.4.0...v0.5.0
[0.4.0]: https://github.com/heroku/buildpacks-python/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/heroku/buildpacks-python/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/heroku/buildpacks-python/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/heroku/buildpacks-python/releases/tag/v0.1.0
