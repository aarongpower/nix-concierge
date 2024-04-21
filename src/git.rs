use std::path::{Path, PathBuf};

use eyre::{Result, WrapErr};
use git2::{BranchType, Repository, StatusOptions};
use git_url_parse::normalize_url;

/// Transforms git url with whatever transport into a generic URL
/// Useful to compare that two remote git repos are the same even if
/// they are using different transports.
///
/// For example, both of the following git URLs become `github.com/username/repo`.
///   - `git@github.com:username/repo.git`
///   - `https://github.com/username/repo`
fn normalize_git_url(url: &str) -> Option<String> {
    let url = normalize_url(url).expect("unable to normalize git url");
    let host = url
        .host_str()
        .expect("could not get git url host string")
        .to_string();
    let path = url
        .path()
        .trim_end_matches(".git")
        .trim_start_matches("/")
        .to_string();
    Some(format!("{host}/{path}"))
}

fn is_same_repo(a: &str, b: &str) -> bool {
    let repo_a = normalize_git_url(a);
    let repo_b = normalize_git_url(b);

    repo_a == repo_b
}

pub fn repo_has_remote(local_path: PathBuf, remote_url: &str) -> Result<bool> {
    let remotes = get_repo_remote_urls(local_path.clone())
        .wrap_err_with(|| format!("Failed to get repo remote URLs: {local_path:?}"))?;
    Ok(remotes
        .into_iter()
        .any(|r| is_same_repo(r.as_str(), remote_url)))
}

pub fn is_git_repo<P: AsRef<Path>>(path: P) -> bool {
    Repository::discover(path).is_ok()
}

fn get_repo_remote_urls(path: PathBuf) -> Result<Vec<String>> {
    let repo = Repository::open(path.clone())
        .wrap_err_with(|| format!("Failed to open local reto at {path:?}"))?;
    let remotes = repo
        .remotes()
        .wrap_err_with(|| format!("Error getting remotes from repo at {path:?}"))?;

    let remote_urls: Vec<String> = remotes
        .iter()
        .filter_map(|r| r)
        .filter_map(|n| repo.find_remote(n).ok())
        .filter_map(|r| r.url().map(|u| u.to_string()))
        .collect();

    Ok(remote_urls)
}

pub fn is_working_tree_clean<P: AsRef<Path>>(path: P) -> Result<bool> {
    let path = path.as_ref();
    let repo = Repository::open(path)?;
    let mut opts = StatusOptions::new();
    opts.include_untracked(true).recurse_untracked_dirs(true);
    let statuses = repo
        .statuses(Some(&mut opts))
        .wrap_err_with(|| format!("Failed getting statuses for repo {:?}", path))?;

    // Check if there are any statuses indicating changes
    Ok(statuses.is_empty())
}

pub enum RepoStatus {
    Ahead,
    Behind,
    Same,
    Complex,
}

pub fn repo_status<P: AsRef<Path>, S: AsRef<str>>(path: P, branch_name: S) -> Result<RepoStatus> {
    let path = path.as_ref();
    let branch_name = branch_name.as_ref();
    let repo = Repository::open(path)
        .wrap_err_with(|| format!("Failed getting repo {:?} to check status.", path))?;

    let mut remote = repo
        .find_remote("origin")
        .wrap_err_with(|| format!("Failed to get remote 'origin' for repo {:?}", path))?;

    remote
        .fetch(
            &[format!(
                "refs/heads/{}:refs/remotes/origin/{}",
                branch_name, branch_name
            )],
            None,
            None,
        )
        .wrap_err_with(|| format!("Failed to fetch updates for repo {:?}", path))?;

    let local_commit = repo
        .find_branch(branch_name, BranchType::Local)
        .wrap_err_with(|| format!("Failed to get local branch {}", branch_name))?
        .get()
        .peel_to_commit()
        .wrap_err_with(|| format!("Failed to get latest commit."))?
        .id();

    let remote_branch_name = format!("origin/{}", branch_name);
    let remote_commit = repo
        .find_reference(&remote_branch_name)
        .wrap_err_with(|| format!("Failed to find reference {remote_branch_name}"))?
        .peel_to_commit()
        .wrap_err_with(|| format!("Failed to get latest remote commit."))?
        .id();

    let (ahead, behind) = repo
        .graph_ahead_behind(local_commit, remote_commit)
        .wrap_err_with(|| "Failed to get graph ahead behind.")?;

    if ahead > 0 && behind == 0 {
        Ok(RepoStatus::Ahead)
    } else if behind > 0 && ahead == 0 {
        Ok(RepoStatus::Behind)
    } else if ahead == 0 && behind == 0 {
        Ok(RepoStatus::Same)
    } else {
        Ok(RepoStatus::Complex)
    }
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::*;

    #[test]
    fn should_normalize_git_ssh_url() {
        let url = "git@github.com:username/repo.git";
        assert_eq!(
            normalize_git_url(url),
            Some("github.com/username/repo".to_string())
        )
    }

    #[test]
    fn should_normalize_git_https_url() {
        let url = "https://github.com/username/repo";
        assert_eq!(
            normalize_git_url(url),
            Some("github.com/username/repo".to_string())
        )
    }

    #[test]
    fn should_match_repos_with_different_schemes() {
        let ssh_url = "git@github.com:username/repo.git";
        let https_url = "https://github.com/username/repo";
        assert!(is_same_repo(ssh_url, https_url))
    }

    #[test]
    #[should_panic]
    fn should_fail_to_match_different_repos() {
        let url_a = "git@github.com:username/repo.git";
        let url_b = "https://github.com/billgates/windows";
        assert!(is_same_repo(url_a, url_b))
    }

    #[test]
    fn should_get_repo_remote_urls() {
        let tmp_repo = setup_temp_repo_with_remote("git@github.com:username/repo.git");

        let urls = get_repo_remote_urls(tmp_repo.path().to_path_buf()).unwrap();

        assert_eq!(urls, vec!["git@github.com:username/repo.git"])
    }

    #[test]
    fn should_match_local_repo_with_remote() {
        let remote_url = "https://example.com/git/repo.git";
        let temp_repo = setup_temp_repo_with_remote(remote_url);

        let has_matching_remote =
            repo_has_remote(temp_repo.path().to_path_buf(), remote_url).unwrap();

        let remotes_string =
            join_git_string_array(Repository::open(temp_repo).unwrap().remotes().unwrap());

        assert!(
            has_matching_remote,
            "Repo remotes {:?} does not include expected {remote_url}",
            remotes_string
        );
    }

    fn join_git_string_array(a: git2::string_array::StringArray) -> String {
        let mut result = String::new();
        let count = a.len();

        for i in 0..count {
            if let Some(item) = a.get(i) {
                if i > 0 {
                    result.push_str(",");
                }
                result.push_str(item);
            }
        }

        result
    }

    fn setup_temp_repo_with_remote(remote_url: &str) -> tempfile::TempDir {
        // create temp dir
        let temp_dir = tempdir().unwrap();
        let repo_path = temp_dir.path();

        // initialize new repo
        let repo = Repository::init(repo_path).unwrap();

        // add remote with the provided url
        repo.remote("origin", remote_url).unwrap();

        // return the temp dir containing the repo
        temp_dir
    }
}
