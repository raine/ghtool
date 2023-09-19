use eyre::Result;
use ghtool::commands::jest::JestLogParser;

fn main() -> Result<()> {
    let file_path = std::env::args().nth(1).unwrap();
    let log = std::fs::read_to_string(file_path).unwrap();
    let parsed = JestLogParser::parse(&log)?;
    println!("{parsed:#?}");
    Ok(())
}
