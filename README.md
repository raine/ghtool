# ghtool

![rust](https://github.com/raine/ghtool/actions/workflows/rust.yml/badge.svg)

A command-line tool for interacting with Github API with some specialized
features oriented around Checks.

## features

- List failing tests across all test checks. Useful in big projects where tests
  are split across multiple jobs.

## installation

Rust toolchain is required. Install it from [rustup.rs](https://rustup.rs/).

```sh
cargo install ghtool
```

## setup

The tool currently uses [`gh`](https://github.com/cli/cli)'s oauth token to
authenticate against GitHub API.

Install `gh` and run `gh auth login` and `ghtool` should be able to read the
token from `~/.config/gh/hosts.yml`.

## usage

The tool is installed as executable `ght` for ease of use.

The tool is intended to be run in a repository, as it uses the current working
directory to determine the repository to operate on. The current branch is used
to determine which pull request to query.

## configuration

A TOML configuration at `.ghtool.toml` at repository root is required.

#### example

```toml
# A regular expression to match test job names
test_job_pattern = "(Unit|Integration|End-to-end) tests sharded"

# Test runner used in tests. Determines how logs are parsed.
# One of: jest
test_runner = "jest"
```

## commands

### `ght failing-tests`

Get the failing tests for the current branch's pull request's checks.

## example usage

### check failing tests

```
% ght failing-tests
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

### run tests for failing test files

```sh
% ght failing-tests --files | xargs yarn test
yarn run v1.22.19
$ NODE_ENV=test node ./node_modules/.bin/jest src/moduleA.test.ts src/moduleB.test.ts
...
```
