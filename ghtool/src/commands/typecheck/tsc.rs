use lazy_static::lazy_static;
use regex::Regex;

const TIMESTAMP_PATTERN: &str = r"\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z";
const ANSI_RESET: &str = r"\u{1b}\[0m";

lazy_static! {
    static ref TIMESTAMP: Regex = Regex::new(&format!(r"{}\s", TIMESTAMP_PATTERN)).unwrap();
    static ref TSC_ERROR_LINE: Regex = Regex::new(&format!(
        r"(?i){TIMESTAMP_PATTERN}\s+(?P<error>##\[error\]).*?({ANSI_RESET})?(?P<path>[a-zA-Z0-9._/-]*)\(\d+,\d+\):\serror\sTS\d+",
        //                                                ^^^^^^^^^^^^^^^^^^ See test_extract_failing_files_3

    ))
    .unwrap();
}

#[derive(Debug, Clone, PartialEq)]
pub struct TscError {
    pub path: String,
    pub lines: Vec<String>,
}

#[derive(PartialEq, Debug)]
enum State {
    LookingForError,
    ParsingError,
}

#[derive(Debug)]
pub struct TscLogParser {
    state: State,
    current_error: Option<TscError>,
    all_errors: Vec<TscError>,
    error_tag_start_col: usize,
}

impl TscLogParser {
    pub fn new() -> Self {
        TscLogParser {
            state: State::LookingForError,
            current_error: None,
            all_errors: Vec::new(),
            error_tag_start_col: 0,
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
                    self.current_error = Some(TscError {
                        lines: vec![without_error_tag.to_string()],
                        path,
                    });
                    self.state = State::ParsingError;
                }
            }
            State::ParsingError => {
                if TSC_ERROR_LINE.is_match(full_line) {
                    let current_error = std::mem::take(&mut self.current_error);
                    self.all_errors.push(current_error.unwrap());
                    self.state = State::LookingForError;
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
                }
            }
        }
        Ok(())
    }

    pub fn parse(log: &str) -> Result<Vec<TscError>, eyre::Error> {
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
                TscError {
                    path: "src/index.ts".to_string(),
                    lines: vec![
                        "src/index.ts(3,21): error TS2769: No overload matches this call.".to_string(),
                        "  Overload 1 of 2, '(object: any, showHidden?: boolean | undefined, depth?: number | null | undefined, color?: boolean | undefined): string', gave the following error.".to_string(),
                        "    Argument of type '\"test\"' is not assignable to parameter of type 'boolean | undefined'.".to_string(),
                    ]
                },
                TscError {
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
                TscError {
                    path: "src/index.ts".to_string(),
                    lines: vec![
                        "src/index.ts(10,3): error TS2322: Type 'number' is not assignable to type 'string'.".to_string(),
                    ]
                },
                TscError {
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
            TscError {
                path: "src/index.ts".to_string(),
                lines: vec![
                    "\u{1b}[32m@owner/package:typecheck: \u{1b}[0msrc/index.ts(63,7): error TS1117: An object literal cannot have multiple properties with the same name.".to_string()
                ],
            },
        ]);
    }
}
