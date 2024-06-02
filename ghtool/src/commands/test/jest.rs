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
    static ref ESCAPE_SEQUENCE: Regex = Regex::new(r"\x1B\[\d+(;\d+)*m").unwrap();
    static ref FAIL_START: Regex = Regex::new(r"(\x1B\[\d+(;\d+)*m)+\s?FAIL").unwrap();
}

fn find_fail_start(log: &str) -> Option<usize> {
    // First handle test_jest_in_docker case: ... |^[[0m FAIL src/b.test.ts
    // In this case, we should get the position where FAIL starts
    // Otherwise try to find left most escape sequence position before FAIL
    log.find("\u{1b}[0m FAIL")
        .and_then(|_| log.find("FAIL"))
        .or_else(|| FAIL_START.find(log).map(|m| m.start()))
        .or_else(|| log.find("FAIL"))
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
                    self.current_fail_start_col = find_fail_start(&line_no_timestamp).unwrap();
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
                let next_char_from_fail =
                    find_next_non_ansi_char(&line_no_timestamp, self.current_fail_start_col);

                // https://github.com/raine/ghtool/assets/11027/08dd631e-391c-4277-8eab-75fe55d9e659
                if line_no_timestamp.len() > self.current_fail_start_col
                    && next_char_from_fail.is_some()
                    && next_char_from_fail != Some(' ')
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

fn find_next_non_ansi_char(str: &str, start_col: usize) -> Option<char> {
    let bytes = str.as_bytes();
    let mut index = start_col;

    while index < bytes.len() {
        if bytes[index] == 0x1B {
            // found an ESC character, start skipping the ANSI sequence
            index += 1; // skip the ESC character
            if index < bytes.len() && bytes[index] == b'[' {
                index += 1; // skip the '[' character
                            // skip until we find a letter indicating the end of the ANSI sequence
                while index < bytes.len() && !bytes[index].is_ascii_alphabetic() {
                    index += 1;
                }
            }
        } else {
            // found a non-ANSI escape character
            return str[index..].chars().next();
        }
        index += 1;
    }

    None
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
    fn test_find_fail_position() {
        let test_cases = vec![
            (
                "2023-12-14T12:24:53.1189316Z [36mtest_1            |[0m FAIL src/b.test.ts",
                Some(58),
            ),
            (
                "2024-05-11T20:45:16.0032874Z [0m[7m[1m[31m FAIL [39m[22m[27m[0m [2msrc/[22m[1mtest2.test.ts[22m ([0m[1m[41m61.458 s[49m[22m[0m)",
                Some(29),
            ),
            (
                "2024-05-29T08:34:09.8655201Z   [1m[31m[7mFAIL[27m[39m[22m src/a.spec.tsx ([31m[7m14728 ms[27m[39m)",
                Some(31),
            ),
            (
                "2024-05-11T20:45:16.0032874Z [0m[7m[1m[31m FAIL [39m[22m[27m[0m [2msrc/[22m[1mtest2.test.ts[22m ([0m[1m[41m61.458 s[49m[22m[0m)",
                Some(29),
            ),
        ];

        for (input, expected) in test_cases {
            assert_eq!(find_fail_start(input), expected);
        }
    }

    #[test]
    fn test_escape_sequence() {
        assert!(ESCAPE_SEQUENCE.is_match("[0m"));
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
                    "".to_string(),
                    "\u{1b}[1m\u{1b}[31m  \u{1b}[1m● \u{1b}[22m\u{1b}[1mtest2 › fails\u{1b}[39m\u{1b}[22m".to_string(),
                    "".to_string(),
                    "    \u{1b}[2mexpect(\u{1b}[22m\u{1b}[31mreceived\u{1b}[39m\u{1b}[2m).\u{1b}[22mtoBe\u{1b}[2m(\u{1b}[22m\u{1b}[32mexpected\u{1b}[39m\u{1b}[2m) // Object.is equality\u{1b}[22m".to_string(),
                    "".to_string(),
                    "    Expected: \u{1b}[32mfalse\u{1b}[39m".to_string(),
                    "    Received: \u{1b}[31mtrue\u{1b}[39m".to_string(),
                    "\u{1b}[2m\u{1b}[22m".to_string(),
                    "\u{1b}[2m    \u{1b}[0m \u{1b}[90m  5 |\u{1b}[39m\u{1b}[0m\u{1b}[22m".to_string(),
                    "\u{1b}[2m    \u{1b}[0m \u{1b}[90m  6 |\u{1b}[39m   it(\u{1b}[32m\"fails\"\u{1b}[39m\u{1b}[33m,\u{1b}[39m () \u{1b}[33m=>\u{1b}[39m {\u{1b}[0m\u{1b}[22m".to_string(),
                    "\u{1b}[2m    \u{1b}[0m\u{1b}[31m\u{1b}[1m>\u{1b}[22m\u{1b}[2m\u{1b}[39m\u{1b}[90m  7 |\u{1b}[39m     expect(\u{1b}[36mtrue\u{1b}[39m)\u{1b}[33m.\u{1b}[39mtoBe(\u{1b}[36mfalse\u{1b}[39m)\u{1b}[33m;\u{1b}[39m\u{1b}[0m\u{1b}[22m".to_string(),
                    "\u{1b}[2m    \u{1b}[0m \u{1b}[90m    |\u{1b}[39m                  \u{1b}[31m\u{1b}[1m^\u{1b}[22m\u{1b}[2m\u{1b}[39m\u{1b}[0m\u{1b}[22m".to_string(),
                    "\u{1b}[2m    \u{1b}[0m \u{1b}[90m  8 |\u{1b}[39m   })\u{1b}[33m;\u{1b}[39m\u{1b}[0m\u{1b}[22m".to_string(),
                    "\u{1b}[2m    \u{1b}[0m \u{1b}[90m  9 |\u{1b}[39m\u{1b}[0m\u{1b}[22m".to_string(),
                    "\u{1b}[2m    \u{1b}[0m \u{1b}[90m 10 |\u{1b}[39m   it(\u{1b}[32m\"foo\"\u{1b}[39m\u{1b}[33m,\u{1b}[39m \u{1b}[36masync\u{1b}[39m () \u{1b}[33m=>\u{1b}[39m {\u{1b}[0m\u{1b}[22m".to_string(),
                    "\u{1b}[2m\u{1b}[22m".to_string(),
                    "\u{1b}[2m      \u{1b}[2mat Object.<anonymous> (\u{1b}[22m\u{1b}[2m\u{1b}[0m\u{1b}[36msrc/test2.test.ts\u{1b}[39m\u{1b}[0m\u{1b}[2m:7:18)\u{1b}[22m\u{1b}[2m\u{1b}[22m".to_string(),
                ]
            },]
        );
    }

    #[test]
    fn test_more_colors() {
        let logs = r#"
2024-05-29T08:34:09.8655201Z   [1m[31m[7mFAIL[27m[39m[22m src/a.spec.tsx ([31m[7m14728 ms[27m[39m)
2024-05-29T08:34:09.8656607Z     utilityFunction
2024-05-29T08:34:09.8658244Z       [31m✕[39m should perform action correctly (29 ms)
2024-05-29T08:34:11.2518625Z ##[group][1m[32m[7mPASS[27m[39m[22m src/FeatureSection.spec.tsx ([31m[7m44752 ms[27m[39m)
2024-05-29T08:37:56.8027075Z [1mSummary of all failing tests[22m
2024-05-29T08:37:56.8042690Z [0m[7m[1m[31m FAIL [39m[22m[27m[0m [2mpackages/foo/src/[22m[1ma.spec.tsx[22m ([0m[1m[41m14.728 s[49m[22m[0m)
2024-05-29T08:37:56.8045558Z [1m[31m  [1m● [22m[1mutilityFunction › should perform action correctly[39m[22m
2024-05-29T08:37:56.8046501Z
2024-05-29T08:37:56.8046955Z     TypeError: Cannot read properties of undefined (reading 'property')
2024-05-29T08:37:56.8047659Z [2m[22m
2024-05-29T08:37:56.8048616Z [2m    [0m [90m 228 |[39m               [90m// To handle undefined properties safely[39m[22m
2024-05-29T08:37:56.8049807Z [2m     [90m 229 |[39m               isEnabled[33m:[39m[22m
2024-05-29T08:37:56.8051465Z [2m    [31m[1m>[22m[2m[39m[90m 230 |[39m                 object[33m.[39mproperty[33m.[39mmode [33m===[39m [32m'active'[39m[33m,[39m[22m
2024-05-29T08:37:56.8052724Z [2m     [90m     |[39m                                      [31m[1m^[22m[2m[39m[22m
2024-05-29T08:37:56.8053954Z [2m     [90m 231 |[39m               [33m...[39m(isEnabled [33m?[39m { isEnabled } [33m:[39m {})[33m,[39m[22m
2024-05-29T08:37:56.8055365Z [2m     [90m 232 |[39m             }[33m;[39m[22m
2024-05-29T08:37:56.8056071Z [2m     [90m 233 |[39m           })[33m,[39m[0m[22m
2024-05-29T08:37:56.8056615Z [2m[22m
2024-05-29T08:37:56.8062690Z [2m      [2mat property ([22m[2msrc/fileA.ts[2m:230:38)[22m[2m[22m
2024-05-29T08:37:56.8063920Z [2m          at Array.map (<anonymous>)[22m
2024-05-29T08:37:56.8065216Z [2m      [2mat map ([22m[2msrc/fileA.ts[2m:200:45)[22m[2m[22m
2024-05-29T08:37:56.8067038Z [2m          at Array.reduce (<anonymous>)[22m
2024-05-29T08:37:56.8068351Z [2m      [2mat reduce ([22m[2msrc/fileA.ts[2m:196:61)[22m[2m[22m
2024-05-29T08:37:56.8118331Z
2024-05-29T08:37:56.8118337Z
2024-05-29T08:37:56.8125241Z [1mTest Suites: [22m[1m[31m1 failed[39m[22m, [1m[33m1 skipped[39m[22m, [1m[32m100 passed[39m[22m, 100 of 100 total
2024-05-29T08:37:56.8127233Z [1mTests:       [22m[1m[31m1 failed[39m[22m, [1m[33m21 skipped[39m[22m, [1m[35m2 todo[39m[22m, [1m[32m100 passed[39m[22m, 100 total
            "#;
        let failing_tests = JestLogParser::parse(logs).unwrap();
        assert_eq!(
            failing_tests,
            vec![
                CheckError {
                    path: "src/a.spec.tsx".to_string(),
                    lines: vec![
                        "\u{1b}[1m\u{1b}[31m\u{1b}[7mFAIL\u{1b}[27m\u{1b}[39m\u{1b}[22m src/a.spec.tsx (\u{1b}[31m\u{1b}[7m14728 ms\u{1b}[27m\u{1b}[39m)".to_string(),
                        "  utilityFunction".to_string(),
                        "    \u{1b}[31m✕\u{1b}[39m should perform action correctly (29 ms)".to_string(),
                    ],
                },
                CheckError {
                    path: "packages/foo/src/a.spec.tsx".to_string(),
                    lines: vec![
                        "\u{1b}[0m\u{1b}[7m\u{1b}[1m\u{1b}[31m FAIL \u{1b}[39m\u{1b}[22m\u{1b}[27m\u{1b}[0m \u{1b}[2mpackages/foo/src/\u{1b}[22m\u{1b}[1ma.spec.tsx\u{1b}[22m (\u{1b}[0m\u{1b}[1m\u{1b}[41m14.728 s\u{1b}[49m\u{1b}[22m\u{1b}[0m)".to_string(),
                        "\u{1b}[1m\u{1b}[31m  \u{1b}[1m● \u{1b}[22m\u{1b}[1mutilityFunction › should perform action correctly\u{1b}[39m\u{1b}[22m".to_string(),
                        "".to_string(),
                        "    TypeError: Cannot read properties of undefined (reading 'property')".to_string(),
                        "\u{1b}[2m\u{1b}[22m".to_string(),
                        "\u{1b}[2m    \u{1b}[0m \u{1b}[90m 228 |\u{1b}[39m               \u{1b}[90m// To handle undefined properties safely\u{1b}[39m\u{1b}[22m".to_string(),
                        "\u{1b}[2m     \u{1b}[90m 229 |\u{1b}[39m               isEnabled\u{1b}[33m:\u{1b}[39m\u{1b}[22m".to_string(),
                        "\u{1b}[2m    \u{1b}[31m\u{1b}[1m>\u{1b}[22m\u{1b}[2m\u{1b}[39m\u{1b}[90m 230 |\u{1b}[39m                 object\u{1b}[33m.\u{1b}[39mproperty\u{1b}[33m.\u{1b}[39mmode \u{1b}[33m===\u{1b}[39m \u{1b}[32m'active'\u{1b}[39m\u{1b}[33m,\u{1b}[39m\u{1b}[22m".to_string(),
                        "\u{1b}[2m     \u{1b}[90m     |\u{1b}[39m                                      \u{1b}[31m\u{1b}[1m^\u{1b}[22m\u{1b}[2m\u{1b}[39m\u{1b}[22m".to_string(),
                        "\u{1b}[2m     \u{1b}[90m 231 |\u{1b}[39m               \u{1b}[33m...\u{1b}[39m(isEnabled \u{1b}[33m?\u{1b}[39m { isEnabled } \u{1b}[33m:\u{1b}[39m {})\u{1b}[33m,\u{1b}[39m\u{1b}[22m".to_string(),
                        "\u{1b}[2m     \u{1b}[90m 232 |\u{1b}[39m             }\u{1b}[33m;\u{1b}[39m\u{1b}[22m".to_string(),
                        "\u{1b}[2m     \u{1b}[90m 233 |\u{1b}[39m           })\u{1b}[33m,\u{1b}[39m\u{1b}[0m\u{1b}[22m".to_string(),
                        "\u{1b}[2m\u{1b}[22m".to_string(),
                        "\u{1b}[2m      \u{1b}[2mat property (\u{1b}[22m\u{1b}[2msrc/fileA.ts\u{1b}[2m:230:38)\u{1b}[22m\u{1b}[2m\u{1b}[22m".to_string(),
                        "\u{1b}[2m          at Array.map (<anonymous>)\u{1b}[22m".to_string(),
                        "\u{1b}[2m      \u{1b}[2mat map (\u{1b}[22m\u{1b}[2msrc/fileA.ts\u{1b}[2m:200:45)\u{1b}[22m\u{1b}[2m\u{1b}[22m".to_string(),
                        "\u{1b}[2m          at Array.reduce (<anonymous>)\u{1b}[22m".to_string(),
                        "\u{1b}[2m      \u{1b}[2mat reduce (\u{1b}[22m\u{1b}[2msrc/fileA.ts\u{1b}[2m:196:61)\u{1b}[22m\u{1b}[2m\u{1b}[22m".to_string(),
                    ],
                },
            ]
        );
    }

    #[test]
    fn test_find_next_non_ansi_char() {
        let str = " \u{1b}[32m\u{1b}[31m ";
        let start_col = 1;
        assert_eq!(find_next_non_ansi_char(str, start_col), Some(' '));
    }
}
