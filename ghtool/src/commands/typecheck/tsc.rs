use lazy_static::lazy_static;
use regex::Regex;

use crate::commands::CheckError;

const TIMESTAMP_PATTERN: &str = r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z";
const ANSI_RESET: &str = r"\u{1b}\[0m";

lazy_static! {
    /// Regex to match a timestamp and single space after it
    static ref TIMESTAMP: Regex = Regex::new(&format!(r"{}\s", TIMESTAMP_PATTERN)).unwrap();

    /// Regex to match an error line of the TypeScript compiler (tsc) log
    static ref TSC_ERROR_LINE: Regex = Regex::new(&format!(
        r"(?i){TIMESTAMP_PATTERN}\s+(?P<error>##\[error\]).*?({ANSI_RESET})?(?P<path>[a-zA-Z0-9._/-]*)\(\d+,\d+\):\serror\sTS\d+",
        //                                                ^^^^^^^^^^^^^^^^^^ See test_extract_failing_files_3

    ))
    .unwrap();
}

#[derive(PartialEq, Debug)]
enum State {
    LookingForError,
    ParsingError,
}

#[derive(Debug)]
pub struct TscLogParser {
    state: State,
    current_error: Option<CheckError>,
    all_errors: Vec<CheckError>,
    error_tag_start_col: usize,
    error_line_count: usize,
}

impl TscLogParser {
    pub fn new() -> Self {
        TscLogParser {
            state: State::LookingForError,
            current_error: None,
            all_errors: Vec::new(),
            error_tag_start_col: 0,
            error_line_count: 0,
        }
    }

    fn parse_line(&mut self, full_line: &str) -> Result<(), eyre::Error> {
        let line = TIMESTAMP.replace(full_line, "");

        match self.state {
            State::LookingForError => {
                if let Some(caps) = TSC_ERROR_LINE.captures(full_line) {
                    let path = caps.name("path").unwrap().as_str().to_string();
                    let without_error_tag = line.strip_prefix("##[error]").unwrap_or(&line);
                    self.error_tag_start_col = caps.name("error").unwrap().start();
                    self.current_error = Some(CheckError {
                        lines: vec![without_error_tag.to_string()],
                        path,
                    });
                    self.state = State::ParsingError;
                }
            }
            State::ParsingError => {
                self.error_line_count += 1;

                if TSC_ERROR_LINE.is_match(full_line) {
                    self.reset_to_looking_for_errors();
                    self.parse_line(full_line)?;
                } else if full_line.chars().nth(self.error_tag_start_col) == Some(' ') {
                    // ##[error]src/index.ts(3,21): error TS2769: No overload matches this call.
                    //   Overload 1 of 2, '(object: any, showHidden?: boolean | undefined, ...
                    // ^ Needs to be whitespace to be parsed as current error's line
                    self.current_error
                        .as_mut()
                        .unwrap()
                        .lines
                        .push(line.to_string());
                } else if self.error_line_count == 1 {
                    // The first line after seeing an error should either:
                    // a) be a new error (first if condition)
                    // b) be indented which means it's part of the error (second else if condition)
                    // In any other case it would be some unrelated output and we want to get back
                    // to looking for errors. See test_extract_failing_files_4.
                    self.reset_to_looking_for_errors()
                }
            }
        }
        Ok(())
    }

    fn reset_to_looking_for_errors(&mut self) {
        let current_error = std::mem::take(&mut self.current_error);
        self.all_errors.push(current_error.unwrap());
        self.state = State::LookingForError;
        self.error_tag_start_col = 0;
        self.error_line_count = 0;
    }

    pub fn parse(log: &str) -> Result<Vec<CheckError>, eyre::Error> {
        let mut parser = TscLogParser::new();

        for line in log.lines() {
            parser.parse_line(line)?;
        }

        if let Some(current_error) = parser.current_error.take() {
            parser.all_errors.push(current_error);
        }

        Ok(parser.all_errors)
    }
}

// Tests
#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_extract_failing_files_1() {
        let logs = r#"
2023-06-26T16:57:36.5365262Z ##[error]src/index.ts(3,21): error TS2769: No overload matches this call.
2023-06-26T16:57:36.5460952Z   Overload 1 of 2, '(object: any, showHidden?: boolean | undefined, depth?: number | null | undefined, color?: boolean | undefined): string', gave the following error.
2023-06-26T16:57:36.5462190Z     Argument of type '"test"' is not assignable to parameter of type 'boolean | undefined'.
2023-06-26T16:57:36.5465097Z ##[error]src/index.ts(10,3): error TS2322: Type 'number' is not assignable to type 'string'.
2023-06-26T16:57:36.5533457Z ##[error]Process completed with exit code 2."#;

        let failing_files = TscLogParser::parse(logs).unwrap();
        assert_eq!(
            failing_files,
            vec![
                CheckError {
                    path: "src/index.ts".to_string(),
                    lines: vec![
                        "src/index.ts(3,21): error TS2769: No overload matches this call.".to_string(),
                        "  Overload 1 of 2, '(object: any, showHidden?: boolean | undefined, depth?: number | null | undefined, color?: boolean | undefined): string', gave the following error.".to_string(),
                        "    Argument of type '\"test\"' is not assignable to parameter of type 'boolean | undefined'.".to_string(),
                    ]
                },
                CheckError {
                    path: "src/index.ts".to_string(),
                    lines: vec![
                        "src/index.ts(10,3): error TS2322: Type 'number' is not assignable to type 'string'.".to_string(),
                    ]
                },
            ]
        );
    }

    #[test]
    fn test_extract_failing_files_2() {
        let logs = r#"
2023-06-26T16:57:36.5465097Z ##[error]src/index.ts(10,3): error TS2322: Type 'number' is not assignable to type 'string'.
2023-06-26T16:57:36.5365262Z ##[error]src/index.ts(3,21): error TS2769: No overload matches this call.
2023-06-26T16:57:36.5460952Z   Overload 1 of 2, '(object: any, showHidden?: boolean | undefined, depth?: number | null | undefined, color?: boolean | undefined): string', gave the following error.
2023-06-26T16:57:36.5462190Z     Argument of type '"test"' is not assignable to parameter of type 'boolean | undefined'."#;

        let failing_files = TscLogParser::parse(logs).unwrap();
        assert_eq!(
            failing_files,
            vec![
                CheckError {
                    path: "src/index.ts".to_string(),
                    lines: vec![
                        "src/index.ts(10,3): error TS2322: Type 'number' is not assignable to type 'string'.".to_string(),
                    ]
                },
                CheckError {
                    path: "src/index.ts".to_string(),
                    lines: vec![
                        "src/index.ts(3,21): error TS2769: No overload matches this call.".to_string(),
                        "  Overload 1 of 2, '(object: any, showHidden?: boolean | undefined, depth?: number | null | undefined, color?: boolean | undefined): string', gave the following error.".to_string(),
                        "    Argument of type '\"test\"' is not assignable to parameter of type 'boolean | undefined'.".to_string(),
                    ]
                },
            ]
        );
    }

    #[test]
    fn test_extract_failing_files_3() {
        let logs = r#"
2023-06-21T14:10:03.3218056Z ##[error][32m@owner/package:typecheck: [0msrc/index.ts(63,7): error TS1117: An object literal cannot have multiple properties with the same name."#;

        let failing_files = TscLogParser::parse(logs).unwrap();
        assert_eq!(failing_files, vec![
            CheckError {
                path: "src/index.ts".to_string(),
                lines: vec![
                    "\u{1b}[32m@owner/package:typecheck: \u{1b}[0msrc/index.ts(63,7): error TS1117: An object literal cannot have multiple properties with the same name.".to_string()
                ],
            },
        ]);
    }

    #[test]
    fn test_extract_failing_files_4() {
        let logs = r#"
2023-06-27T08:32:59.2543883Z ##[error][34m@project:typecheck: [0msrc/components/Component.spec.tsx(58,8): error TS2739: Type '{ foo: string; }' is missing the following properties from type 'Props': bar
2023-06-27T08:33:50.2166735Z [34m@project:typecheck: [0m[41m[30m ELIFECYCLE [39m[49m [31mCommand failed with exit code 1.[39m
2023-06-27T08:33:50.2437013Z [91m[34m@project:typecheck: [0mERROR: command finished with error: command (/home/runner) pnpm run typecheck exited (1)[0m
2023-06-27T08:33:50.3894539Z [34mproject:typecheck: [0m[41m[30m ELIFECYCLE [39m[49m [31mCommand failed.[39m
2023-06-27T08:33:50.3968735Z [91mcommand (/home/runner) pnpm run typecheck exited (1)[0m
2023-06-27T08:33:50.3983800Z 
2023-06-27T08:33:50.3984922Z  Tasks:    145 successful, 147 total
2023-06-27T08:33:50.3985487Z Cached:    73 cached, 147 total
2023-06-27T08:33:50.3985812Z   Time:    3m7.006s"#;

        let failing_files = TscLogParser::parse(logs).unwrap();
        assert_eq!(failing_files, vec![
            CheckError {
                path: "src/components/Component.spec.tsx".to_string(),
                lines: vec![
                    "\u{1b}[34m@project:typecheck: \u{1b}[0msrc/components/Component.spec.tsx(58,8): error TS2739: Type '{ foo: string; }' is missing the following properties from type 'Props': bar".to_string(),
                ],
            },
        ]);
    }
}
