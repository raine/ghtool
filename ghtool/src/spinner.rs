use indicatif::ProgressStyle;

pub fn make_spinner_style() -> ProgressStyle {
    ProgressStyle::with_template("{spinner:.yellow.bold} {msg}")
        .unwrap()
        .tick_chars("⠁⠂⠄⡀⢀⠠⠐⠈ ")
}
