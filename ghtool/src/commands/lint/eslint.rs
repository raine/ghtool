use lazy_static::lazy_static;
use regex::Regex;

#[derive(PartialEq, Debug)]
enum State {
    LookingForFile,
    ParsingFile,
}

lazy_static! {
    /// Regex to match a path at the end of line
    static ref PATH_PATTERN: Regex = Regex::new(
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
#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct EslintPath {
    pub path: String,
    pub issues: Vec<String>,
}

#[derive(Debug)]
pub struct EslintLogParser {
    state: State,
    current_path: Option<EslintPath>,
    all_paths: Vec<EslintPath>,
    path_start_col: usize,
    seen_eslint_issue_for_current_path: bool,
}

impl EslintLogParser {
    pub fn new() -> Self {
        EslintLogParser {
            state: State::LookingForFile,
            current_path: None,
            all_paths: Vec::new(),
            path_start_col: 0,
            seen_eslint_issue_for_current_path: false,
        }
    }

    fn get_line_without_ts(&self, line: &str) -> String {
        line.chars().skip(self.path_start_col).collect()
    }

    /// Is the line empty when disregarding timestamp
    fn is_empty_line(&self, line: &str) -> bool {
        line.chars().nth(self.path_start_col).is_none()
    }

    fn parse_line(&mut self, line: &str) {
        match self.state {
            State::LookingForFile => {
                if let Some(caps) = PATH_PATTERN.captures(line) {
                    self.path_start_col = caps.name("path").unwrap().start();
                    let path = self.get_line_without_ts(line);
                    self.current_path = Some(EslintPath {
                        path,
                        issues: Vec::new(),
                    });
                    self.state = State::ParsingFile;
                }
            }
            State::ParsingFile => {
                if ESLINT_ISSUE.is_match(line) {
                    let issue = self.get_line_without_ts(line);
                    self.current_path.as_mut().unwrap().issues.push(issue);
                    self.seen_eslint_issue_for_current_path = true;
                } else if self.is_empty_line(line) {
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
                    issues: vec![
                        "##[warning]  1:42  warning  Missing return type on function  @typescript-eslint/explicit-module-boundary-types"
                            .to_string(),
                    ],
                },
                EslintPath {
                    path: "/root_path/project_directory/module_2/setupModule2Test.ts".to_string(),
                    issues: vec![
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
                    issues: vec![
                        "##[warning]  4:47  warning  Missing return type on function  @typescript-eslint/explicit-module-boundary-types"
                            .to_string(),
                    ],
                },
                EslintPath {
                    path: "/root_path/project_directory/module_4/submodule_2/setupInitialDB.ts".to_string(),
                    issues: vec![
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
                        issues: vec![
                            "##[warning]  1:42  warning  Missing return type on function  @typescript-eslint/explicit-module-boundary-types"
                                .to_string(),
                        ],
                    },

                ]

            );
    }

    fn test_parse_ansi_monorepo() {
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
                    issues: vec![
                        "##[warning]  1:42  warning  Missing return type on function  @typescript-eslint/explicit-module-boundary-types"
                            .to_string(),
                    ],
                },
                EslintPath {
                    path: "/root_path/project_directory/module_2/setupModule2Test.ts".to_string(),
                    issues: vec![
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
                    issues: vec![
                        "##[warning]  4:47  warning  Missing return type on function  @typescript-eslint/explicit-module-boundary-types"
                            .to_string(),
                    ],
                },
                EslintPath {
                    path: "/root_path/project_directory/module_4/submodule_2/setupInitialDB.ts".to_string(),
                    issues: vec![
                        "##[error]  1:1   error  Delete `import·*·as·fs·from·'fs';⏎`  prettier/prettier"
                            .to_string(),
                        "##[error]  1:13  error  'fs' is defined but never used       @typescript-eslint/no-unused-vars"
                            .to_string(),
                    ],
                },
            ]
        );
    }
}
