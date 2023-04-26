# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Changed

- Updated pip from 23.0.1 to 23.1.2. ([#31](https://github.com/heroku/buildpacks-python/pull/31))
- Updated setuptools from 67.6.1 to 67.7.2. ([#30](https://github.com/heroku/buildpacks-python/pull/30))

## [0.2.0] - 2023-04-11

### Changed

- The default Python version is now 3.11.3 (previously 3.11.2). ([#22](https://github.com/heroku/buildpacks-python/pull/22))
- Updated setuptools from 67.5.0 to 67.6.1. ([#24](https://github.com/heroku/buildpacks-python/pull/24))
- Updated wheel from 0.38.4 to 0.40.0. ([#24](https://github.com/heroku/buildpacks-python/pull/24))

### Fixed

- The `PYTHONHOME` environment variable is now set to work around uWSGI not handling relocated Python installs correctly. ([#25](https://github.com/heroku/buildpacks-python/pull/25))

## [0.1.0] - 2023-03-06

### Added

- Initial implementation. ([#3](https://github.com/heroku/buildpacks-python/pull/3))
