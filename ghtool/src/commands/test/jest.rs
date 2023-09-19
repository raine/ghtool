use crate::commands::command::CheckError;
use eyre::Result;
use lazy_static::lazy_static;
use regex::Regex;

const TIMESTAMP_PATTERN: &str = r"(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z)";

lazy_static! {
    /// Regex to match a timestamp and single space after it
    static ref TIMESTAMP: Regex = Regex::new(&format!(r"{TIMESTAMP_PATTERN}\s?")).unwrap();
    static ref JEST_FAIL_LINE: Regex = Regex::new(&format!(
        r"{TIMESTAMP_PATTERN}\s+(?P<fail>FAIL)\s+(?P<path>[a-zA-Z0-9._-]*/[a-zA-Z0-9./_-]*)",
    ))
    .unwrap();
}

#[derive(Debug, Clone, PartialEq)]
pub struct JestPath {
    pub path: String,
    pub lines: Vec<String>,
}

#[derive(PartialEq, Debug)]
enum State {
    LookingForFail,
    ParsingFail,
}

#[derive(Debug)]
pub struct JestLogParser {
    state: State,
    current_fail: Option<CheckError>,
    all_fails: Vec<CheckError>,
    current_fail_start_col: usize,
    current_fail_lines: Vec<String>,
}

impl JestLogParser {
    pub fn new() -> Self {
        JestLogParser {
            state: State::LookingForFail,
            current_fail: None,
            all_fails: Vec::new(),
            current_fail_start_col: 0,
            current_fail_lines: Vec::new(),
        }
    }

    fn parse_line(&mut self, raw_line: &str) -> Result<(), eyre::Error> {
        let line_no_ansi = String::from_utf8(strip_ansi_escapes::strip(raw_line.as_bytes())?)?;
        let line = TIMESTAMP.replace(raw_line, "");

        match self.state {
            State::LookingForFail => {
                if let Some(caps) = JEST_FAIL_LINE.captures(&line_no_ansi) {
                    self.current_fail_start_col = caps.name("fail").unwrap().start();
                    let path = caps.name("path").unwrap().as_str().to_string();
                    self.current_fail = Some(CheckError {
                        lines: vec![line.to_string()],
                        path,
                    });
                    self.state = State::ParsingFail;
                }
            }
            State::ParsingFail => {
                if line_no_ansi.len() > self.current_fail_start_col
                    && line_no_ansi.chars().nth(self.current_fail_start_col) != Some(' ')
                {
                    let mut current_fail = std::mem::take(&mut self.current_fail).unwrap();

                    // Remove trailing empty lines
                    if let Some(last_non_empty_line) =
                        current_fail.lines.iter().rposition(|line| !line.is_empty())
                    {
                        current_fail.lines.truncate(last_non_empty_line + 1);
                    }

                    self.all_fails.push(current_fail);
                    self.current_fail_lines = Vec::new();
                    self.state = State::LookingForFail;
                } else {
                    self.current_fail
                        .as_mut()
                        .unwrap()
                        .lines
                        .push(line.to_string());
                }
            }
        }
        Ok(())
    }

    pub fn parse(log: &str) -> Result<Vec<CheckError>> {
        let mut parser = JestLogParser::new();

        for line in log.lines() {
            parser.parse_line(line)?;
        }

        Ok(parser.get_output())
    }

    pub fn get_output(self) -> Vec<CheckError> {
        self.all_fails
            .into_iter()
            .fold(Vec::new(), |mut acc, fail| {
                if !acc.contains(&fail) {
                    acc.push(fail);
                }
                acc
            })
    }
}

// Tests
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_extract_failing_tests() {
        let logs = r#"
2021-05-04T18:24:29.000Z FAIL src/components/MyComponent/MyComponent.test.tsx
2021-05-04T18:24:29.000Z   ● Test suite failed to run
2021-05-04T18:24:29.000Z     TypeError: Cannot read property 'foo' of undefined
2021-05-04T18:24:29.000Z
2021-05-04T18:24:29.000Z       1 | import React from 'react';
2021-05-04T18:24:29.000Z PASS src/components/MyComponent/MyComponent.test.tsx
2021-05-04T18:24:29.000Z FAIL src/components/MyComponent/MyComponent2.test.tsx
2021-05-04T18:24:29.000Z   ● Test suite failed to run
2021-05-04T18:24:29.000Z     TypeError: Cannot read property 'foo' of undefined
2021-05-04T18:24:29.000Z
2021-05-04T18:24:29.000Z       1 | import React from 'react';
2021-05-04T18:24:29.000Z PASS src/components/MyComponent/MyComponent2.test.tsx"#;

        let failing_tests = JestLogParser::parse(logs).unwrap();
        assert_eq!(
            failing_tests,
            vec![
                CheckError {
                    path: "src/components/MyComponent/MyComponent.test.tsx".to_string(),
                    lines: vec![
                        "FAIL src/components/MyComponent/MyComponent.test.tsx".to_string(),
                        "  ● Test suite failed to run".to_string(),
                        "    TypeError: Cannot read property 'foo' of undefined".to_string(),
                        "".to_string(),
                        "      1 | import React from 'react';".to_string(),
                    ]
                },
                CheckError {
                    path: "src/components/MyComponent/MyComponent2.test.tsx".to_string(),
                    lines: vec![
                        "FAIL src/components/MyComponent/MyComponent2.test.tsx".to_string(),
                        "  ● Test suite failed to run".to_string(),
                        "    TypeError: Cannot read property 'foo' of undefined".to_string(),
                        "".to_string(),
                        "      1 | import React from 'react';".to_string(),
                    ]
                },
            ]
        );
    }

    #[test]
    fn test_extract_failing_tests_2() {
        let logs = r#"
2023-06-28T21:11:38.9421220Z > ghtool-test-repo@1.0.0 test
2023-06-28T21:11:38.9428514Z > jest ./src --color --ci --shard=1/2
2023-06-28T21:11:38.9429089Z 
2023-06-28T21:11:43.1619050Z  FAIL  src/test2.test.ts
2023-06-28T21:11:43.1623893Z   test2
2023-06-28T21:11:43.1629746Z     ✓ succeeds (3 ms)
2023-06-28T21:11:43.1630396Z     ✕ fails (5 ms)
2023-06-28T21:11:43.1630949Z 
2023-06-28T21:11:43.1631448Z   ● test2 › fails
2023-06-28T21:11:43.1631750Z 
2023-06-28T21:11:43.1632455Z     expect(received).toBe(expected) // Object.is equality
2023-06-28T21:11:43.1633081Z 
2023-06-28T21:11:43.1633381Z     Expected: false
2023-06-28T21:11:43.1633800Z     Received: true
2023-06-28T21:11:43.1634250Z 
2023-06-28T21:11:43.1634753Z        5 |
2023-06-28T21:11:43.1635444Z        6 |   it("fails", () => {
2023-06-28T21:11:43.1636318Z     >  7 |     expect(true).toBe(false);
2023-06-28T21:11:43.1637060Z          |                  ^
2023-06-28T21:11:43.1642719Z        8 |   });
2023-06-28T21:11:43.1647216Z        9 | });
2023-06-28T21:11:43.1648590Z       10 |
2023-06-28T21:11:43.1649650Z 
2023-06-28T21:11:43.1651496Z       at Object.<anonymous> (src/test2.test.ts:7:18)
2023-06-28T21:11:43.1652032Z 
2023-06-28T21:11:43.1664383Z Test Suites: 1 failed, 1 total
2023-06-28T21:11:43.1665139Z Tests:       1 failed, 1 passed, 2 total
2023-06-28T21:11:43.1665683Z Snapshots:   0 total
2023-06-28T21:11:43.1666152Z Time:        3.464 s
2023-06-28T21:11:43.1666769Z Ran all test suites matching /.\/src/i."#;

        let failing_tests = JestLogParser::parse(logs).unwrap();
        assert_eq!(
            failing_tests,
            vec![CheckError {
                path: "src/test2.test.ts".to_string(),
                lines: vec![
                    " FAIL  src/test2.test.ts".to_string(),
                    "  test2".to_string(),
                    "    ✓ succeeds (3 ms)".to_string(),
                    "    ✕ fails (5 ms)".to_string(),
                    "".to_string(),
                    "  ● test2 › fails".to_string(),
                    "".to_string(),
                    "    expect(received).toBe(expected) // Object.is equality".to_string(),
                    "".to_string(),
                    "    Expected: false".to_string(),
                    "    Received: true".to_string(),
                    "".to_string(),
                    "       5 |".to_string(),
                    "       6 |   it(\"fails\", () => {".to_string(),
                    "    >  7 |     expect(true).toBe(false);".to_string(),
                    "         |                  ^".to_string(),
                    "       8 |   });".to_string(),
                    "       9 | });".to_string(),
                    "      10 |".to_string(),
                    "".to_string(),
                    "      at Object.<anonymous> (src/test2.test.ts:7:18)".to_string(),
                ],
            },]
        );
    }

    #[test]
    fn test_extract_failing_test_files() {
        let logs = r#"
2021-05-04T18:24:29.000Z FAIL src/components/MyComponent/MyComponent.test.tsx
2021-05-04T18:24:29.000Z   ● Test suite failed to run
2021-05-04T18:24:29.000Z     TypeError: Cannot read property 'foo' of undefined
2021-05-04T18:24:29.000Z
2021-05-04T18:24:29.000Z       1 | import React from 'react';
2021-05-04T18:24:29.000Z PASS src/components/MyComponent/MyComponent2.test.tsx
2021-05-04T18:24:29.000Z FAIL src/components/MyComponent/MyComponent3.test.tsx
2021-05-04T18:24:29.000Z   ● Test suite failed to run
2021-05-04T18:24:29.000Z     TypeError: Cannot read property 'foo' of undefined
2021-05-04T18:24:29.000Z
2021-05-04T18:24:29.000Z       1 | import React from 'react';
2021-05-04T18:24:29.000Z PASS src/components/MyComponent/MyComponent4.test.tsx"#;

        let failing_tests = JestLogParser::parse(logs).unwrap();
        let failing_test_files: Vec<String> = failing_tests
            .iter()
            .map(|jest_path| jest_path.path.clone())
            .collect();

        assert_eq!(
            failing_test_files,
            vec![
                "src/components/MyComponent/MyComponent.test.tsx".to_string(),
                "src/components/MyComponent/MyComponent3.test.tsx".to_string(),
            ]
        );
    }

    #[test]
    fn test_remove_duplicate_check_errors() {
        let logs = r#"
2023-09-14T12:22:30.2648458Z
2023-09-14T12:22:30.2648458Z FAIL src/components/MyComponent/MyComponent3.test.tsx
2023-09-14T12:22:30.2648458Z   ● Test suite failed to run
2023-09-14T12:22:30.2648458Z     TypeError: Cannot read property 'foo' of undefined
2023-09-14T12:22:30.2648458Z
2023-09-14T12:22:30.2648458Z       1 | import React from 'react';
2023-09-14T12:22:30.2648458Z 
2023-09-14T12:22:30.2649146Z Summary of all failing tests
2023-09-14T12:22:30.2648458Z FAIL src/components/MyComponent/MyComponent3.test.tsx
2023-09-14T12:22:30.2648458Z   ● Test suite failed to run
2023-09-14T12:22:30.2648458Z     TypeError: Cannot read property 'foo' of undefined
2023-09-14T12:22:30.2648458Z
2023-09-14T12:22:30.2648458Z       1 | import React from 'react';
2023-09-14T12:22:30.2673693Z 
2023-09-14T12:22:30.2673711Z 
2023-09-14T12:22:30.2678119Z Test Suites: 1 failed, 67 passed, 68 total
2023-09-14T12:22:30.2679079Z Tests:       1 failed, 469 passed, 470 total
2023-09-14T12:22:30.2680281Z Snapshots:   60 passed, 60 total
2023-09-14T12:22:30.2680933Z Time:        216.339 s
        "#;

        let failing_tests = JestLogParser::parse(logs).unwrap();
        assert_eq!(failing_tests.len(), 1);
    }
}
