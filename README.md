# ghtool

[![Crates.io][crates-badge]][crates-url]
![rust][build-badge]

A command-line tool for interacting with Github API with some specialized
features oriented around Checks.

## features

- List failing tests across all test checks. Useful in big projects where tests
  are split across multiple jobs.
- List linting issues across all checks.

## installation

Rust toolchain is required. Install it from [rustup.rs](https://rustup.rs/).

```sh
cargo install ghtool
```

## setup

The tool currently uses [`gh`](https://github.com/cli/cli)'s oauth token to
authenticate against GitHub API.

[Install](https://github.com/cli/cli#installation) `gh` and run `gh auth login`
and `ghtool` should be able to read the token from `~/.config/gh/hosts.yml`.

## usage

The tool is installed as executable `ght` for ease of use.

The tool is intended to be run in a repository, as it uses the current working
directory to determine the repository to operate on. The current branch is used
to determine which pull request to query.

### commands

#### `ght tests`

Get the failing tests for the current branch's pull request's checks.

#### `ght lint`

Get linting issues for the current branch's pull request's checks.

## configuration

A TOML configuration at `.ghtool.toml` at repository root is required.

#### example

```toml
[test]

# A regular expression to match test job names
job_pattern = "(Unit|Integration|End-to-end) tests sharded"

# Test runner used in tests. Determines how logs are parsed.
# One of: jest
runner = "jest"

[lint]

# A regular expression to match test job names
job_pattern = "Lint"

# Lint tool used in the checks. Determines how logs are parsed.
# One of: eslint
tool = "eslint"
```

## example usage

### check failing tests

```
% ght tests
┌─────────────────────────────────────────────────────────────────────────────┐
│ Job: Unit tests sharded (2)                                                 │
│ Url: https://github.com/org/repo/actions/runs/5252627921/jobs/9488888294    │
└─────────────────────────────────────────────────────────────────────────────┘
FAIL src/components/MyComponent/MyComponent.test.tsx
  ● Test suite failed to run
    Error: Cannot read property 'foo' of undefined

      1 | import React from 'react';
      2 | import { render } from '@testing-library/react';
    > 3 | import MyComponent from './MyComponent';
        | ^
      4 |
      5 | test('renders learn react link', () => {
      6 |   const { getByText } = render(<MyComponent />);

┌─────────────────────────────────────────────────────────────────────────────┐
│ Job: Unit tests sharded (3)                                                 │
│ Url: https://github.com/org/repo/actions/runs/5252627921/jobs/9488888295    │
└─────────────────────────────────────────────────────────────────────────────┘
FAIL src/components/AnotherComponent/AnotherComponent.test.tsx
    ● Test suite failed to run
...
```

### check lint issues

```
% ght lint
┌─────────────────────────────────────────────────────────────────────────────┐
│ Job: Lint                                                                   │
│ Url: https://github.com/org/repo/actions/runs/5252627921/jobs/9488888294    │
└─────────────────────────────────────────────────────────────────────────────┘
@org/module:lint: /path/to/work/directory/src/components/component-directory/subcomponent-file/index.tsx
@org/module:lint:    99:54  warning  Unexpected any. Specify a different type  @typescript-eslint/no-explicit-any
@org/module:lint:   109:46  warning  Unexpected any. Specify a different type  @typescript-eslint/no-explicit-any
@org/module:lint:   143:59  warning  Unexpected any. Specify a different type  @typescript-eslint/no-explicit-any

@org/module:lint: /path/to/work/directory/src/components/another-component/ComponentTest.spec.tsx
@org/module:lint:   30:33  warning  Forbidden non-null assertion  @typescript-eslint/no-non-null-assertion

@org/another-module:lint: /path/to/work/directory/src/components/DifferentComponent/ComponentTest.spec.tsx
@org/another-module:lint:   2:18  error  'waitFor' is defined but never used  @typescript-eslint/no-unused-vars
```

### run tests for failing test files

```sh
% ght tests --files | xargs yarn test
yarn run v1.22.19
$ NODE_ENV=test node ./node_modules/.bin/jest src/moduleA.test.ts src/moduleB.test.ts
...
```

[crates-badge]: https://img.shields.io/crates/v/ghtool.svg
[crates-url]: https://crates.io/crates/ghtool
[build-badge]: https://github.com/raine/ghtool/actions/workflows/rust.yml/badge.svg
