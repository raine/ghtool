use std::io::{self, Write};

use eyre::Result;

use crate::github;

pub fn bold(text: &str) -> String {
    format!("\x1b[1m{}\x1b[0m", text)
}

pub fn green(text: &str) -> String {
    format!("\x1b[32m{}\x1b[0m", text)
}

pub fn print_header(header: &str) {
    if let Some((w, _)) = term_size::dimensions() {
        let lines = header.split('\n').collect::<Vec<_>>();
        let horizontal_border = "─".repeat(w - 2);
        let border = format!("┌{}┐", horizontal_border);
        let end_border = format!("└{}┘", horizontal_border);
        println!("{}", border);
        for line in lines {
            let stripped_line = strip_ansi_escapes::strip(line).unwrap();
            let mut line = String::from_utf8(stripped_line).unwrap();
            let line_len = line.chars().count();
            if line_len > w - 4 {
                let truncated_line_len = w - 7; // For ellipsis and spaces
                line = line.chars().take(truncated_line_len).collect::<String>();
                line.push_str("...");
            }
            let line_padding = w - line.chars().count() - 4;
            let header_line = format!("│ {}{} │", line, " ".repeat(line_padding));
            println!("{}", header_line);
        }
        println!("{}", end_border);
    }
}

pub fn exit_with_error<T>(e: eyre::Error) -> T {
    eprintln!("{}", e);
    std::process::exit(1);
}

pub fn print_check_run_header(check_run: &github::SimpleCheckRun) {
    print_header(&format!(
        "{} {}\n{} {}",
        bold("Job:"),
        check_run.name,
        bold("Url:"),
        check_run.url.as_ref().unwrap()
    ));
}

pub fn print_all_checks_green() {
    eprintln!("{} All checks are green", green("✓"));
}

pub fn read_stdin() -> Result<String> {
    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(input.trim().to_string())
}

pub fn prompt_for_user_to_continue(prompt_message: &str) -> io::Result<()> {
    print!("{}", prompt_message);
    io::stdout().flush()?;

    let mut input = String::new();
    io::stdin().read_line(&mut input)?;
    Ok(())
}
