use indicatif::ProgressStyle;

const TICK_CHARS: &str = "⠁⠂⠄⡀⢀⠠⠐⠈ ";

pub fn make_spinner_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.yellow.bold} {msg}")
        .unwrap()
        .tick_chars(TICK_CHARS)
}

pub fn make_job_spinner() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.yellow.bold} {msg} {elapsed:.dim}")
        .unwrap()
        .tick_chars(TICK_CHARS)
}

pub fn make_job_completed_spinner() -> ProgressStyle {
    ProgressStyle::with_template("{prefix:.green} {msg} {elapsed}")
        .unwrap()
        .tick_chars(TICK_CHARS)
}

pub fn make_job_failed_spinner() -> ProgressStyle {
    ProgressStyle::with_template("{prefix:.red} {msg} {elapsed}")
        .unwrap()
        .tick_chars(TICK_CHARS)
}
