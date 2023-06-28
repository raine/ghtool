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
                    let current_fail = std::mem::take(&mut self.current_fail);
                    self.all_fails.push(current_fail.unwrap());
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
}
