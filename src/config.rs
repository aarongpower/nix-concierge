use std::path::PathBuf;

use eyre::{eyre, Context, Result};
use git2::Repository;
use url::Url;

use crate::fs::is_directory_empty;
use crate::git::{is_git_repo, is_working_tree_clean, repo_has_remote, repo_status, RepoStatus};

// at some later point this will be handled by some kind of
// config management. For now, hard code all the things because it is just me using it.

/// Idempotent function to clone a repo to a target dir
/// If there is already an existing repo there, it will check that the remote matches
/// If the remote does not match, it will return an error and user must manually remediate
/// If the remote does matches then it does nothing, it is up to the user to manage the contents of the repo
#[allow(dead_code)]
fn deploy_config_repo(target_path: PathBuf, repo_url: Url) -> Result<()> {
    let clone_repo = || {
        Repository::clone(repo_url.as_str(), target_path.clone()).wrap_err_with(|| {
            format!(
                "Failed cloning repository {:?} to {:?}",
                repo_url, target_path
            )
        })
    };

    // If target dir does not exist then create it and clone repo
    if !target_path.exists() {
        println!("*** Config dir {target_path:?} does not exist, creating.");
        std::fs::create_dir_all(target_path.clone()).wrap_err_with(|| {
            format!("Failed creating dir for config repo at {:?}", target_path)
        })?;
        let _ = clone_repo()?;
        return Ok(());
    }

    // we can now assume the dir exists
    // first, we check if it is empty, if so then clone
    if is_directory_empty(target_path.clone())? {
        let _ = clone_repo()?;
        return Ok(());
    }

    // if it is not empty, bail if it is not a git repo
    if !is_git_repo(target_path.clone()) {
        return Err(eyre!(format!(
            "Target config dir {:?} is not a git repo and is not empty",
            target_path
        )));
    }

    // bail if it is not the repo we expect, i.e., it does not have the correct remote
    if !repo_has_remote(target_path.clone(), repo_url.as_str())? {
        return Err(eyre!(format!(
            "Target config dir {:?} is a git repo but does not have expected remote {:?}",
            target_path.clone(),
            repo_url.clone()
        )));
    }

    // So we have a repo and it has the correct remote
    // There are a few scenarios here
    //   - Working tree is not empty (i.e., there are uncommited changes) - don't do anything with git and use nix to build config
    //   - Working tree is empty and we are up to date with remote - use nix to build config, then commit and push changed flake.lock
    //   - Working tree is empty and we are behind remote - pull from repo and use nix do build config, then commit and push changed flake.lock
    //   - Working tree is empty and we are ahead of remote - use nix to build config, commit changed flake.lock and push to remote

    if !is_working_tree_clean(target_path.clone()).wrap_err_with(|| {
        format!(
            "Failed to check if working tree is clean for {:?}",
            target_path.clone()
        )
    })? {
        println!("*** Working tree is not clean, deploying config but won't interact with git.");
        todo!("Run deployment.");
    }

    // Ok we can now assume the working tree is empty
    // Let's figure out our status in comparison to the origin
    let repo_status = repo_status(target_path.clone(), "origin")
        .wrap_err_with(|| format!("Failed to get repo status for repo {:?}", target_path))?;

    // before we deploy, we want to pull if we're behind
    if let RepoStatus::Behind = repo_status {
        println!("Local repo is behind remote. Pulling changes before deployment.");
        todo!("Pull latest changes from remote")
    }

    // if repo status is complex, then bail because we don't want to accidentally mess things up
    if let RepoStatus::Complex = repo_status {
        return Err(eyre!("Repo {:?} has complex status. Local has commits that are ahead of remote, and remote also has commits that are ahead of local. This will have to be rectified before concierge can complete deployment.", target_path.clone()))?;
    }

    // now we can run the deployment
    println!("*** Deploying config to nix dir and building with nix.");

    // commit changes to flake.lock
    println!("Updating flake.lock, and committing.");

    // push to remote
    println!("Pushing changes to remote repo.");

    Ok(())
}
