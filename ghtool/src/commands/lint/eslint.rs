use lazy_static::lazy_static;
use regex::Regex;

#[derive(PartialEq, Debug)]
enum State {
    LookingForFile,
    ParsingFile,
}

lazy_static! {
    /// Regex to match a timestamp and single space after it
    static ref TIMESTAMP: Regex =
        Regex::new(r"(?P<timestamp>\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}\.\d+Z)\s").unwrap();

    /// Regex to match a path at the end of line
    static ref PATH: Regex = Regex::new(
        r"\s(?P<path>/[a-zA-Z0-9._-]*/[a-zA-Z0-9./_-]*)$",
    )
    .unwrap();

    /// Regex to match eslint issue on a file line
    /// Example: 1:10 error Missing return type
    static ref ESLINT_ISSUE: Regex = Regex::new(
        r"\d+:\d+\s+\b(warning|error)\b",
    )
    .unwrap();
}

/// This struct represent a block like this in log output:
///
/// 2023-06-14T20:22:39.1727281Z /root_path/project_directory/module_1/submodule_1/fixtures/data/file_1.ts
/// 2023-06-14T20:22:39.1789066Z ##[warning]  1:42  warning  Missing return type on function  @typescript-eslint/explicit-module-boundary-types
///
/// Timestamp will not be included.
#[derive(Debug, Clone, PartialEq)]
pub struct EslintPath {
    pub path: String,
    pub lines: Vec<String>,
}

#[derive(Debug)]
pub struct EslintLogParser {
    state: State,
    current_path: Option<EslintPath>,
    all_paths: Vec<EslintPath>,
    current_path_start_col: usize,
    seen_eslint_issue_for_current_path: bool,
    current_path_lines: usize,
}

impl EslintLogParser {
    pub fn new() -> Self {
        EslintLogParser {
            state: State::LookingForFile,
            current_path: None,
            all_paths: Vec::new(),
            current_path_start_col: 0,
            current_path_lines: 0,
            seen_eslint_issue_for_current_path: false,
        }
    }

    fn get_line_from_path_col(&self, line: &str) -> String {
        line.chars().skip(self.current_path_start_col).collect()
    }

    /// Is the line empty when disregarding timestamp
    fn is_empty_line(&self, line: &str) -> bool {
        line.chars().nth(self.current_path_start_col).is_none()
    }

    fn parse_line(&mut self, raw_line: &str) {
        let line_no_ansi =
            String::from_utf8(strip_ansi_escapes::strip(raw_line.as_bytes()).unwrap()).unwrap();

        match self.state {
            State::LookingForFile => {
                if let Some(caps) = PATH.captures(&line_no_ansi) {
                    self.current_path_start_col = caps.name("path").unwrap().start();
                    let path = self.get_line_from_path_col(&line_no_ansi);
                    let line = TIMESTAMP.replace(raw_line, "");
                    self.current_path = Some(EslintPath {
                        lines: vec![line.to_string()],
                        path,
                    });
                    self.state = State::ParsingFile;
                }
            }
            State::ParsingFile => {
                self.current_path_lines += 1;

                if ESLINT_ISSUE.is_match(&line_no_ansi) {
                    let line = TIMESTAMP.replace(raw_line, "").to_string();
                    self.current_path.as_mut().unwrap().lines.push(line);
                    self.seen_eslint_issue_for_current_path = true;
                } else if self.current_path_lines == 1 {
                    // If the line directly under path does not match ESLINT_ISSUE, reset back to
                    // looking for file. In certain cases this avoids the problem of never getting
                    // back to looking for file state because some path is matched early in the
                    // logs.
                    self.state = State::LookingForFile;
                    self.seen_eslint_issue_for_current_path = false;
                    self.current_path_lines = 0;
                } else if self.is_empty_line(&line_no_ansi) {
                    // Empty line after starting to "parse a file" means the file will change
                    //
                    // Example:
                    // 2023-06-14T20:22:39.1727281Z /root_path/project_directory/module_1/submodule_1/fixtures/data/file_1.ts
                    // 2023-06-14T20:22:39.1789066Z ##[warning]  1:42  warning  Missing return type on function  @typescript-eslint/explicit-module-boundary-types
                    // 2023-06-14T20:22:39.1790470Z [empty line]
                    // 2023-06-14T20:22:39.1790995Z /root_path/project_directory/module_2/setupModule2Test.ts
                    // 2023-06-14T20:22:39.1792493Z ##[warning]  166:58  warning  Missing return type on function  @typescript-eslint/explicit-module-boundary-types
                    self.state = State::LookingForFile;

                    if self.seen_eslint_issue_for_current_path {
                        let current_eslint_path = std::mem::take(&mut self.current_path);
                        self.all_paths.push(current_eslint_path.unwrap());
                    } else {
                        self.current_path = None;
                    }

                    self.seen_eslint_issue_for_current_path = false;
                    self.current_path_lines = 0;
                }
            }
        }
    }

    pub fn parse(log: &str) -> Vec<EslintPath> {
        let mut parser = EslintLogParser::new();

        for line in log.lines() {
            parser.parse_line(line);
        }

        parser.get_output()
    }

    pub fn get_output(self) -> Vec<EslintPath> {
        self.all_paths
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_basic() {
        let log: &str = r#"
2023-06-14T20:10:57.9100220Z > project@0.0.1 lint:base
2023-06-14T20:10:57.9102305Z > eslint --ext .ts --ignore-pattern "node_modules" --ignore-pattern "coverage" --ignore-pattern "**/*.js" src test
2023-06-14T20:10:57.9102943Z 
2023-06-14T20:22:39.1725170Z 
2023-06-14T20:22:39.1727281Z /root_path/project_directory/module_1/submodule_1/fixtures/data/file_1.ts
2023-06-14T20:22:39.1789066Z ##[warning]  1:42  warning  Missing return type on function  @typescript-eslint/explicit-module-boundary-types
2023-06-14T20:22:39.1790470Z 
2023-06-14T20:22:39.1790995Z /root_path/project_directory/module_2/setupModule2Test.ts
2023-06-14T20:22:39.1792493Z ##[warning]  166:58  warning  Missing return type on function  @typescript-eslint/explicit-module-boundary-types
2023-06-14T20:22:39.1794354Z ##[warning]  309:55  warning  Missing return type on function  @typescript-eslint/explicit-module-boundary-types
2023-06-14T20:22:39.1795885Z ##[warning]  470:55  warning  Missing return type on function  @typescript-eslint/explicit-module-boundary-types
2023-06-14T20:22:39.1796538Z 
2023-06-14T20:22:39.1796973Z /root_path/project_directory/module_3/getSpecificUploadImageResponse.ts
2023-06-14T20:22:39.1798218Z ##[warning]  4:47  warning  Missing return type on function  @typescript-eslint/explicit-module-boundary-types
2023-06-14T20:22:39.1815738Z 
2023-06-14T20:22:39.1816392Z /root_path/project_directory/module_4/submodule_2/setupInitialDB.ts
2023-06-14T20:22:39.1818449Z ##[error]  1:1   error  Delete `import·*·as·fs·from·'fs';⏎`  prettier/prettier
2023-06-14T20:22:39.1819948Z ##[error]  1:13  error  'fs' is defined but never used       @typescript-eslint/no-unused-vars
2023-06-14T20:22:39.2063811Z
2023-06-14T20:22:39.2063811Z ✖ 132 problems (4 errors, 128 warnings)
2023-06-14T20:22:39.2064409Z   2 errors and 0 warnings potentially fixable with the `--fix` option."#;

        let output = EslintLogParser::parse(log);
        assert_eq!(
            output,
            vec![
                EslintPath {
                    path: "/root_path/project_directory/module_1/submodule_1/fixtures/data/file_1.ts".to_string(),
                    lines: vec![
                        "/root_path/project_directory/module_1/submodule_1/fixtures/data/file_1.ts".to_string(),
                        "##[warning]  1:42  warning  Missing return type on function  @typescript-eslint/explicit-module-boundary-types"
                            .to_string(),
                    ],
                },
                EslintPath {
                    path: "/root_path/project_directory/module_2/setupModule2Test.ts".to_string(),
                    lines: vec![
                        "/root_path/project_directory/module_2/setupModule2Test.ts".to_string(),
                        "##[warning]  166:58  warning  Missing return type on function  @typescript-eslint/explicit-module-boundary-types"
                            .to_string(),
                        "##[warning]  309:55  warning  Missing return type on function  @typescript-eslint/explicit-module-boundary-types"
                            .to_string(),
                        "##[warning]  470:55  warning  Missing return type on function  @typescript-eslint/explicit-module-boundary-types"
                            .to_string(),
                    ],
                },
                EslintPath {
                    path: "/root_path/project_directory/module_3/getSpecificUploadImageResponse.ts".to_string(),
                    lines: vec![
                        "/root_path/project_directory/module_3/getSpecificUploadImageResponse.ts".to_string(),
                        "##[warning]  4:47  warning  Missing return type on function  @typescript-eslint/explicit-module-boundary-types"
                            .to_string(),
                    ],
                },
                EslintPath {
                    path: "/root_path/project_directory/module_4/submodule_2/setupInitialDB.ts".to_string(),
                    lines: vec![
                        "/root_path/project_directory/module_4/submodule_2/setupInitialDB.ts".to_string(),
                        "##[error]  1:1   error  Delete `import·*·as·fs·from·'fs';⏎`  prettier/prettier"
                            .to_string(),
                        "##[error]  1:13  error  'fs' is defined but never used       @typescript-eslint/no-unused-vars"
                            .to_string(),
                    ],
                },
            ]
        );
    }

    #[test]
    fn test_parse_corner_case() {
        let log = r#"
2023-06-14T20:10:38.3206108Z ##[debug]Cleaning runner temp folder: /home/runner/work/_temp
2023-06-14T20:10:38.3472682Z ##[debug]Starting: Set up job
2023-06-14T20:10:41.2671897Z [command]/usr/bin/git config --global --add safe.directory /home/runner/work/test/test
2023-06-14T20:10:41.2671897Z
2023-06-14T20:22:39.1727281Z /root_path/project_directory/module_1/submodule_1/fixtures/data/file_1.ts
2023-06-14T20:22:39.1789066Z ##[warning]  1:42  warning  Missing return type on function  @typescript-eslint/explicit-module-boundary-types
2023-06-14T20:10:41.2671897Z
    "#;
        let output = EslintLogParser::parse(log);
        assert_eq!(
                output,
                vec![
                    EslintPath {
                        path: "/root_path/project_directory/module_1/submodule_1/fixtures/data/file_1.ts".to_string(),
                        lines: vec![
                            "/root_path/project_directory/module_1/submodule_1/fixtures/data/file_1.ts".to_string(),
                            "##[warning]  1:42  warning  Missing return type on function  @typescript-eslint/explicit-module-boundary-types"
                                .to_string(),
                        ],
                    },

                ]

            );
    }

    #[test]
    fn test_parse_ansi_monorepo() {
        let log: &str = r#"
2023-06-16T15:54:54.4381752Z [34m@project/package:lint: [0m> @project/package@x.y.z lint:eslint /path/to/working/directory
2023-06-16T15:54:54.4383282Z [34m@project/package:lint: [0m> eslint -c .eslintrc.js .
2023-06-16T15:54:54.4385037Z [34m@project/package:lint: [0m
2023-06-16T15:54:54.4386084Z [34m@project/package:lint: [0m[0m[0m
2023-06-16T15:54:54.4387931Z [34m@project/package:lint: [0m[0m[4m/path/to/working/directory/src/components/ComponentWrapper.spec.tsx[24m[0m
2023-06-16T15:54:54.4389816Z [34m@project/package:lint: [0m[0m   [2m8:1[22m  [33mwarning[39m  Disabled test suite  [2mjest/no-disabled-tests[22m[0m
2023-06-16T15:54:54.4391533Z [34m@project/package:lint: [0m[0m  [2m41:7[22m  [33mwarning[39m  Disabled test        [2mjest/no-disabled-tests[22m[0m
2023-06-16T15:54:54.4393248Z [34m@project/package:lint: [0m[0m  [2m59:7[22m  [33mwarning[39m  Disabled test        [2mjest/no-disabled-tests[22m[0m
2023-06-16T15:54:54.4394749Z [34m@project/package:lint: [0m[0m[0m
2023-06-16T15:54:54.4396497Z [34m@project/package:lint: [0m[0m[4m/path/to/working/directory/src/hooks/useCustomHook.spec.ts[24m[0m
2023-06-16T15:54:54.4398548Z [34m@project/package:lint: [0m[0m  [2m6:46[22m  [33mwarning[39m  Unexpected any. Specify a different type  [2m@typescript-eslint/no-explicit-any[22m[0m
2023-06-16T15:54:54.4400116Z [34m@project/package:lint: [0m[0m
2023-06-16T15:54:54.4401725Z [34m@project/package:lint: [0m[0m[33m[1m✖ 4 problems (0 errors, 4 warnings)[22m[39m[0m
2023-06-14T20:22:39.2063811Z ✖ 132 problems (4 errors, 128 warnings)"#;

        let output = EslintLogParser::parse(log);
        assert_eq!(output, vec![
            EslintPath {
                path: "/path/to/working/directory/src/components/ComponentWrapper.spec.tsx".to_string(),
                lines: vec![
                    "\u{1b}[34m@project/package:lint: \u{1b}[0m\u{1b}[0m\u{1b}[4m/path/to/working/directory/src/components/ComponentWrapper.spec.tsx\u{1b}[24m\u{1b}[0m".to_string(),
                    "\u{1b}[34m@project/package:lint: \u{1b}[0m\u{1b}[0m   \u{1b}[2m8:1\u{1b}[22m  \u{1b}[33mwarning\u{1b}[39m  Disabled test suite  \u{1b}[2mjest/no-disabled-tests\u{1b}[22m\u{1b}[0m".to_string(),
                    "\u{1b}[34m@project/package:lint: \u{1b}[0m\u{1b}[0m  \u{1b}[2m41:7\u{1b}[22m  \u{1b}[33mwarning\u{1b}[39m  Disabled test        \u{1b}[2mjest/no-disabled-tests\u{1b}[22m\u{1b}[0m".to_string(),
                    "\u{1b}[34m@project/package:lint: \u{1b}[0m\u{1b}[0m  \u{1b}[2m59:7\u{1b}[22m  \u{1b}[33mwarning\u{1b}[39m  Disabled test        \u{1b}[2mjest/no-disabled-tests\u{1b}[22m\u{1b}[0m".to_string()
                ],
            },
            EslintPath {
                path: "/path/to/working/directory/src/hooks/useCustomHook.spec.ts".to_string(),
                lines: vec![
                    "\u{1b}[34m@project/package:lint: \u{1b}[0m\u{1b}[0m\u{1b}[4m/path/to/working/directory/src/hooks/useCustomHook.spec.ts\u{1b}[24m\u{1b}[0m".to_string(),
                    "\u{1b}[34m@project/package:lint: \u{1b}[0m\u{1b}[0m  \u{1b}[2m6:46\u{1b}[22m  \u{1b}[33mwarning\u{1b}[39m  Unexpected any. Specify a different type  \u{1b}[2m@typescript-eslint/no-explicit-any\u{1b}[22m\u{1b}[0m".to_string()
                ],
            },
        ]);
    }
}
