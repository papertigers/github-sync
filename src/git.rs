use crate::github::Repo;
use anyhow::Result;
use std::path::Path;

fn do_fetch<'a>(
    repo: &'a git2::Repository,
    refs: &[&str],
    remote: &'a mut git2::Remote,
) -> Result<Option<git2::AnnotatedCommit<'a>>, git2::Error> {
    let mut fo = git2::FetchOptions::new();
    fo.download_tags(git2::AutotagOption::All);
    remote.fetch(refs, Some(&mut fo), None)?;
    if let Ok(fetch_head) = repo.find_reference("FETCH_HEAD") {
        return Ok(Some(repo.reference_to_annotated_commit(&fetch_head)?));
    }

    Ok(None)
}

fn do_reset<'a>(
    repo: &'a git2::Repository,
    fetch_commit: git2::AnnotatedCommit<'a>,
) -> Result<(), git2::Error> {
    let remote_obj = repo.find_commit(fetch_commit.id())?.into_object();
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.force();
    repo.reset(&remote_obj, git2::ResetType::Hard, None)?;
    Ok(())
}

fn update_repo<P: AsRef<Path>>(path: P) -> Result<()> {
    let r = git2::Repository::open(path.as_ref())?;
    let mut remote = r.find_remote("origin")?;
    if let Some(fetch_commit) = do_fetch(&r, &[], &mut remote)? {
        do_reset(&r, fetch_commit)?;
    }

    Ok(())
}

pub fn clone_or_update<P: AsRef<Path>>(path: P, repo: &Repo) -> Result<()> {
    let path = path.as_ref();
    if !path.exists() {
        let mut git_cmd = git2::build::RepoBuilder::new();
        git_cmd.branch(&repo.default_branch);
        if let Err(ge) = git_cmd.clone(&repo.clone_url, path) {
            match ge.code() {
                // We know github thinks the repo exists, but it's likely an
                // empty repo and we are trying to clone a specific branch
                // that doesn't yet exist.
                git2::ErrorCode::NotFound => return Ok(()),
                _ => return Err(ge.into()),
            }
        }
    } else {
        update_repo(path)?
    }

    Ok(())
}
