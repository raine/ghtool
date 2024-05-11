use crate::commands::command::CheckError;
use eyre::Result;
use lazy_static::lazy_static;
use regex::Regex;

const TIMESTAMP_PATTERN: &str = r"(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z)";

lazy_static! {
    /// Regex to match a timestamp and single space after it
    static ref TIMESTAMP: Regex = Regex::new(&format!(r"{TIMESTAMP_PATTERN}\s?")).unwrap();
    static ref JEST_FAIL_LINE: Regex =
        Regex::new(r"(?P<fail>FAIL)\s+(?P<path>[a-zA-Z0-9._-]*/[a-zA-Z0-9./_-]*)").unwrap();
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
        let line_no_ansi = String::from_utf8(strip_ansi_escapes::strip(raw_line.as_bytes()))?;
        let line_no_timestamp = TIMESTAMP.replace(raw_line, "");

        match self.state {
            State::LookingForFail => {
                if let Some(caps) = JEST_FAIL_LINE.captures(&line_no_ansi) {
                    // Attempt to find the column where the colored FAIL text starts.
                    // This column position will be used to determine where jest output starts.
                    // We can't just take everything after timestamp because there's possibility
                    // that jest is running inside docker-compose in which case there would be
                    // service name after timestamp.
                    // https://github.com/raine/ghtool/assets/11027/c349807a-cad1-45cb-b02f-4d5020bb3c23
                    let reset_escape_code = "\u{1b}[0m\u{1b}[7m";
                    let reset_pos = line_no_timestamp.find(reset_escape_code);
                    let fail_pos = line_no_timestamp.find("FAIL");
                    self.current_fail_start_col = reset_pos.unwrap_or(fail_pos.unwrap());
                    let path = caps.name("path").unwrap().as_str().to_string();
                    // Get line discarding things before the column where FAIL starts
                    let line = line_no_timestamp
                        .chars()
                        .skip(self.current_fail_start_col)
                        .collect::<String>();
                    self.current_fail = Some(CheckError {
                        lines: vec![line.to_string()],
                        path,
                    });
                    self.state = State::ParsingFail;
                }
            }
            State::ParsingFail => {
                if line_no_timestamp.len() > self.current_fail_start_col
                    && line_no_timestamp.chars().nth(self.current_fail_start_col) != Some(' ')
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
                    // Get line discarding things before the column where FAIL starts
                    let line = line_no_timestamp
                        .chars()
                        .skip(self.current_fail_start_col)
                        .collect::<String>();

                    self.current_fail.as_mut().unwrap().lines.push(line);
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

impl Default for JestLogParser {
    fn default() -> Self {
        Self::new()
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
                    "FAIL  src/test2.test.ts".to_string(),
                    " test2".to_string(),
                    "   ✓ succeeds (3 ms)".to_string(),
                    "   ✕ fails (5 ms)".to_string(),
                    "".to_string(),
                    " ● test2 › fails".to_string(),
                    "".to_string(),
                    "   expect(received).toBe(expected) // Object.is equality".to_string(),
                    "".to_string(),
                    "   Expected: false".to_string(),
                    "   Received: true".to_string(),
                    "".to_string(),
                    "      5 |".to_string(),
                    "      6 |   it(\"fails\", () => {".to_string(),
                    "   >  7 |     expect(true).toBe(false);".to_string(),
                    "        |                  ^".to_string(),
                    "      8 |   });".to_string(),
                    "      9 | });".to_string(),
                    "     10 |".to_string(),
                    "".to_string(),
                    "     at Object.<anonymous> (src/test2.test.ts:7:18)".to_string(),
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

    #[test]
    fn test_jest_in_docker() {
        let logs = r#"
2023-12-14T12:24:25.7014935Z [36mtest_1            |[0m $ jest -c jest.config.test.js
2023-12-14T12:24:43.7723478Z [36mtest_1            |[0m PASS src/a.test.ts (16.764 s)
2023-12-14T12:24:53.1189316Z [36mtest_1            |[0m FAIL src/b.test.ts
2023-12-14T12:24:53.1486488Z [36mtest_1            |[0m   ● test › return test things
2023-12-14T12:24:53.1488314Z [36mtest_1            |[0m
2023-12-14T12:24:53.1489247Z [36mtest_1            |[0m     expect(received).toMatchObject(expected)
2023-12-14T12:24:53.1490238Z [36mtest_1            |[0m
2023-12-14T12:24:53.1490994Z [36mtest_1            |[0m     - Expected  - 1
2023-12-14T12:24:53.1491871Z [36mtest_1            |[0m     + Received  + 0
2023-12-14T12:24:53.1492657Z [36mtest_1            |[0m
2023-12-14T12:24:53.1493405Z [36mtest_1            |[0m     @@ -17,9 +17,8 @@
2023-12-14T12:24:53.1662308Z [36mtest_1            |[0m     -       "testId": undefined,
2023-12-14T12:24:53.1684564Z [36mtest_1            |[0m           },
2023-12-14T12:24:53.1724498Z [36mtest_1            |[0m         },
2023-12-14T12:24:53.1764019Z [36mtest_1            |[0m       ]
2023-12-14T12:24:53.1788159Z [36mtest_1            |[0m
2023-12-14T12:24:53.1790147Z [36mtest_1            |[0m     > 62 |     expect(result).toMatchObject([
2023-12-14T12:24:53.1790859Z [36mtest_1            |[0m          |                    ^
2023-12-14T12:24:53.1794182Z [36mtest_1            |[0m
2023-12-14T12:24:53.1794946Z [36mtest_1            |[0m       at Object.<anonymous> (src/a.test.ts:62:20)
2023-12-14T12:24:53.1841737Z [36mtest_1            |[0m
2023-12-14T12:24:53.4683252Z [36mtest_1            |[0m PASS src/b.test.ts
        "#;

        let failing_tests = JestLogParser::parse(logs).unwrap();

        assert_eq!(
            failing_tests,
            vec![CheckError {
                path: "src/b.test.ts".to_string(),
                lines: vec![
                    "FAIL src/b.test.ts".to_string(),
                    "  ● test › return test things".to_string(),
                    "".to_string(),
                    "    expect(received).toMatchObject(expected)".to_string(),
                    "".to_string(),
                    "    - Expected  - 1".to_string(),
                    "    + Received  + 0".to_string(),
                    "".to_string(),
                    "    @@ -17,9 +17,8 @@".to_string(),
                    "    -       \"testId\": undefined,".to_string(),
                    "          },".to_string(),
                    "        },".to_string(),
                    "      ]".to_string(),
                    "".to_string(),
                    "    > 62 |     expect(result).toMatchObject([".to_string(),
                    "         |                    ^".to_string(),
                    "".to_string(),
                    "      at Object.<anonymous> (src/a.test.ts:62:20)".to_string(),
                ],
            }]
        );
    }

    #[test]
    fn test_colors() {
        let logs = r#"
2024-05-11T20:44:13.9945728Z [2K[1G[2m$ jest ./src --color --ci --shard=1/2[22m
2024-05-11T20:45:16.0032874Z [0m[7m[1m[31m FAIL [39m[22m[27m[0m [2msrc/[22m[1mtest2.test.ts[22m ([0m[1m[41m61.458 s[49m[22m[0m)
2024-05-11T20:45:16.0034300Z   test2
2024-05-11T20:45:16.0037347Z     [32m✓[39m [2msucceeds (1 ms)[22m
2024-05-11T20:45:16.0038258Z     [31m✕[39m [2mfails (2 ms)[22m
2024-05-11T20:45:16.0039034Z     [32m✓[39m [2mfoo (60001 ms)[22m
2024-05-11T20:45:16.0039463Z 
2024-05-11T20:45:16.0039981Z [1m[31m  [1m● [22m[1mtest2 › fails[39m[22m
2024-05-11T20:45:16.0040506Z 
2024-05-11T20:45:16.0041462Z     [2mexpect([22m[31mreceived[39m[2m).[22mtoBe[2m([22m[32mexpected[39m[2m) // Object.is equality[22m
2024-05-11T20:45:16.0045857Z 
2024-05-11T20:45:16.0046210Z     Expected: [32mfalse[39m
2024-05-11T20:45:16.0046774Z     Received: [31mtrue[39m
2024-05-11T20:45:16.0047256Z [2m[22m
2024-05-11T20:45:16.0047765Z [2m    [0m [90m  5 |[39m[0m[22m
2024-05-11T20:45:16.0048791Z [2m    [0m [90m  6 |[39m   it([32m"fails"[39m[33m,[39m () [33m=>[39m {[0m[22m
2024-05-11T20:45:16.0051048Z [2m    [0m[31m[1m>[22m[2m[39m[90m  7 |[39m     expect([36mtrue[39m)[33m.[39mtoBe([36mfalse[39m)[33m;[39m[0m[22m
2024-05-11T20:45:16.0052427Z [2m    [0m [90m    |[39m                  [31m[1m^[22m[2m[39m[0m[22m
2024-05-11T20:45:16.0053352Z [2m    [0m [90m  8 |[39m   })[33m;[39m[0m[22m
2024-05-11T20:45:16.0054060Z [2m    [0m [90m  9 |[39m[0m[22m
2024-05-11T20:45:16.0055164Z [2m    [0m [90m 10 |[39m   it([32m"foo"[39m[33m,[39m [36masync[39m () [33m=>[39m {[0m[22m
2024-05-11T20:45:16.0056008Z [2m[22m
2024-05-11T20:45:16.0057064Z [2m      [2mat Object.<anonymous> ([22m[2m[0m[36msrc/test2.test.ts[39m[0m[2m:7:18)[22m[2m[22m
2024-05-11T20:45:16.0057817Z 
2024-05-11T20:45:16.0064933Z [1mTest Suites: [22m[1m[31m1 failed[39m[22m, 1 total
2024-05-11T20:45:16.0065943Z [1mTests:       [22m[1m[31m1 failed[39m[22m, [1m[32m2 passed[39m[22m, 3 total
2024-05-11T20:45:16.0066489Z [1mSnapshots:   [22m0 total
2024-05-11T20:45:16.0066847Z [1mTime:[22m        61.502 s
2024-05-11T20:45:16.0067359Z [2mRan all test suites[22m[2m matching [22m/.\/src/i[2m.[22m
        "#;

        let failing_tests = JestLogParser::parse(logs).unwrap();
        assert_eq!(
            failing_tests,
            vec![CheckError {
                path: "src/test2.test.ts".to_string(),
                lines: vec![
                    "\u{1b}[0m\u{1b}[7m\u{1b}[1m\u{1b}[31m FAIL \u{1b}[39m\u{1b}[22m\u{1b}[27m\u{1b}[0m \u{1b}[2msrc/\u{1b}[22m\u{1b}[1mtest2.test.ts\u{1b}[22m (\u{1b}[0m\u{1b}[1m\u{1b}[41m61.458 s\u{1b}[49m\u{1b}[22m\u{1b}[0m)".to_string(),
                    "  test2".to_string(),
                    "    \u{1b}[32m✓\u{1b}[39m \u{1b}[2msucceeds (1 ms)\u{1b}[22m".to_string(),
                    "    \u{1b}[31m✕\u{1b}[39m \u{1b}[2mfails (2 ms)\u{1b}[22m".to_string(),
                    "    \u{1b}[32m✓\u{1b}[39m \u{1b}[2mfoo (60001 ms)\u{1b}[22m".to_string(),
                ]
            },]
        );
    }
}
