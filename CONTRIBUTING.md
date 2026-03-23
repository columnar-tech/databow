<!--
Copyright 2026 Columnar Technologies Inc.
SPDX-License-Identifier: Apache-2.0
-->

# Contributing

All contributors are expected to follow the [Code of Conduct](https://github.com/columnar-tech/databow?tab=coc-ov-file#readme).

## Issues and Feature Requests

Please file issues and feature requests on the GitHub issue tracker: https://github.com/columnar-tech/databow/issues

Potential security vulnerabilities should be reported to [security@columnar.tech](mailto:security@columnar.tech) instead.

## Development

Format, lint, and test:

```sh
cargo fmt
cargo clippy
cargo test
```

Build and run the binary:

```sh
cargo build
./target/debug/databow
```

## Pull Requests

Before opening a pull request:

- Review your changes and ensure no stray files or folders are included.
- Use the [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) format for all commit messages.
- Check if there is an existing issue. If not, please file one, unless the change is trivial.

When writing a pull request description:

- Use the [Conventional Commits](https://www.conventionalcommits.org/en/v1.0.0/) format for the pull request title.
- Ensure the description ends with `Closes #NNN`, `Fixes #NNN`, or similar, so that the issue will be linked to your pull request.
