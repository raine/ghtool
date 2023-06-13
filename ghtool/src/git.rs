use eyre::Result;
use std::path::PathBuf;

#[derive(Debug)]
pub struct Repository {
    pub owner: String,
    pub name: String,
    pub hostname: String,
}

impl std::fmt::Display for Repository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let key = format!("{}/{}/{}", self.hostname, self.owner, self.name);
        write!(f, "{}", key)
    }
}

pub struct Git {
    pub directory: PathBuf,
}

// Example url: git@github.com:raine/tgreddit.git
fn parse_repository(url: &str) -> Result<Repository> {
    let mut parts = url.trim().split(':');
    let host = parts.next();
    let mut parts = parts.next().unwrap().split('/');
    let owner = parts.next().unwrap().to_string();
    let name = parts
        .next()
        .unwrap()
        .strip_suffix(".git")
        .unwrap()
        .to_string();
    let hostname = host.unwrap().split('@').nth(1).unwrap().to_string();
    Ok(Repository {
        owner,
        name,
        hostname,
    })
}

impl Git {
    pub fn new(directory: PathBuf) -> Self {
        Self { directory }
    }

    pub fn get_branch(&self) -> Result<String> {
        let output = std::process::Command::new("git")
            .arg("rev-parse")
            .arg("--abbrev-ref")
            .arg("HEAD")
            .current_dir(&self.directory)
            .output()?;
        let branch = String::from_utf8(output.stdout)?;
        Ok(branch.trim().to_string())
    }

    pub fn get_remote(&self) -> Result<Repository> {
        let output = std::process::Command::new("git")
            .arg("remote")
            .arg("get-url")
            .arg("origin")
            .current_dir(&self.directory)
            .output()?;
        let url = String::from_utf8(output.stdout)?;
        let repository = parse_repository(&url)?;
        Ok(repository)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_repository() {
        let url = "git@github.com:raine/tgreddit.git";
        let repository = parse_repository(url).unwrap();
        assert_eq!(repository.owner, "raine");
        assert_eq!(repository.name, "tgreddit");
        assert_eq!(repository.hostname, "github.com");
    }
}
