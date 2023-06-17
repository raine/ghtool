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
            let line_len = strip_ansi_escapes::strip(line).unwrap().len();
            let line_padding = w - line_len - 4;
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
