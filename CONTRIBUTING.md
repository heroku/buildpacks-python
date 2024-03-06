# Contributing to Heroku Cloud Native Buildpacks

This page lists the operational governance model of this project, as well as
the recommendations and requirements for how to best contribute to Heroku
Cloud Native Buildpacks. We strive to obey these as best as possible. As
always, thanks for contributing.

## Governance Model: Salesforce Sponsored

The intent and goal of open sourcing this project is to increase the contributor
and user base. However, only Salesforce employees will be given `admin` rights
and will be the final arbitrars of what contributions are accepted or not.

## Getting started

Please feel free to join the
[Heroku Cloud Native Buildpacks discussions][discussions].
You may also wish to take a look at
[Heroku's product roadmap][roadmap] to see where are headed.

## Ideas and Feedback

Please use
[Heroku Cloud Native Buildpacks discussions][discussions]
to provide feedback, request enhancements, or discuss ideas.

## Issues, Feature Requests, and Bug Reports

Issues, feature requests, and bug reports are tracked via [GitHub issues on
this repository][issues]. If you find
an issue and/or bug, please search the issues, and if it isn't already tracked,
create a new issue.

## Fixes, Improvements, and Patches

Fixes, improvements, and patches all happen via [GitHub Pull Requests on this
repository][pulls]. If you'd like to
improve the tests, you want to make the documentation clearer, you have an
alternative implementation of something that may have advantages over the way
its currently done, or you have any other change, we would be happy to hear
about it. For trivial changes, send a pull request. For non-trivial changes,
consider [opening an issue](#issues-feature-requests-and-bug-reports) to
discuss it first instead.

## Development

### Dependencies

This buildpack relies on [heroku/libcnb.rs][libcnb] to compile buildpacks. All
[libcnb.rs dependencies][libcnb-deps] will need to be setup prior to building
or testing this buildpack.

### Building

1. Run `cargo check` to download dependencies and ensure there are no
   compilation issues.
1. Build the buildpack with `cargo libcnb package`.
1. Use the buildpack to build an app: `pack build sample-app --buildpack packaged/x86_64-unknown-linux-musl/debug/heroku_python --path /path/to/sample-app`

### Testing

- `cargo test` performs Rust unit tests.
- `cargo test -- --ignored` performs all integration tests.

## Code of Conduct
Please follow our [Code of Conduct](CODE_OF_CONDUCT.md).

## License
By contributing your code, you agree to license your contribution under the
terms of our project [LICENSE](LICENSE) and to sign the
[Salesforce CLA](https://cla.salesforce.com/sign-cla).


[discussions]: https://github.com/heroku/buildpacks/discussions
[issues]: https://github.com/heroku/buildpacks-python/issues
[libcnb]: https://github.com/heroku/libcnb.rs
[libcnb-deps]: https://github.com/heroku/libcnb.rs#development-environment-setup
[pulls]: https://github.com/heroku/buildpacks-python/pulls
[roadmap]: https://github.com/heroku/roadmap
