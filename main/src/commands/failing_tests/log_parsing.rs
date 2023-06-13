use lazy_static::lazy_static;
use regex::Regex;

const TIMESTAMP_PATTERN: &str = r"(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z)";

lazy_static! {
    /// Regex to match a timestamp at the start of a line including the whitespace after it
    static ref TIMESTAMP: Regex = Regex::new(TIMESTAMP_PATTERN).unwrap();

    /// Regex to match a failing jest test. The path needs to contain at least one slash.
    /// Example: 2021-05-04T18:24:29.000Z FAIL src/components/MyComponent/MyComponent.test.tsx
    static ref JEST_FAIL_LINE: Regex = Regex::new(&format!(
        r"{TIMESTAMP_PATTERN}\s+(?P<fail>FAIL)\s+(?P<path>[a-zA-Z0-9._-]*/[a-zA-Z0-9./_-]*)",
    ))
    .unwrap();
}

pub fn extract_failing_tests(logs: &str) -> Result<Vec<Vec<String>>, eyre::Error> {
    let mut fail_start_col = 0;
    let mut in_test_case = false;
    let mut current_fail_lines = Vec::new();
    let mut failing_tests_inner = Vec::new();

    // Collect failing tests from the logs by reading lines from a line that matches JEST_FAIL
    // until there is a line where there is something else than whitespace in the same column
    // as the FAIL match.
    //
    // 2021-05-04T18:24:29.000Z FAIL src/components/MyComponent/MyComponent.test.tsx
    // 2021-05-04T18:24:29.000Z   ● Test suite failed to run
    // 2021-05-04T18:24:29.000Z     TypeError: Cannot read property 'foo' of undefined
    // 2021-05-04T18:24:29.000Z
    // 2021-05-04T18:24:29.000Z       1 | import React from 'react';
    // 2021-05-04T18:24:29.000Z PASS src/components/MyComponent/MyComponent.test.tsx
    for full_line in logs.lines() {
        let line_no_ansi = String::from_utf8(strip_ansi_escapes::strip(full_line.as_bytes())?)?;
        let line_no_timestamp = TIMESTAMP.replace(full_line, "");

        if let Some(caps) = JEST_FAIL_LINE.captures(&line_no_ansi) {
            fail_start_col = caps.name("fail").unwrap().start();
            current_fail_lines.push(line_no_timestamp.to_string());
            in_test_case = true;
        } else if in_test_case {
            if line_no_ansi.len() > fail_start_col
                && line_no_ansi.chars().nth(fail_start_col) != Some(' ')
            {
                failing_tests_inner.push(current_fail_lines);
                current_fail_lines = Vec::new();
                in_test_case = false;
            } else {
                current_fail_lines.push(line_no_timestamp.to_string());
            }
        }
    }

    Ok(failing_tests_inner)
}

pub fn extract_failing_test_files(logs: &str) -> Result<Vec<String>, eyre::Error> {
    let mut test_files = Vec::new();

    for full_line in logs.lines() {
        let line_no_ansi = String::from_utf8(
            strip_ansi_escapes::strip(full_line.as_bytes())
                .map_err(|_| eyre::eyre!("Error when stripping ansi escapes"))?,
        )?;

        if let Some(caps) = JEST_FAIL_LINE.captures(&line_no_ansi) {
            test_files.push(caps.name("path").unwrap().as_str().to_string());
        }
    }

    Ok(test_files)
}
