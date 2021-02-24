use crate::github::Repo;
use anyhow::Result;
use std::path::Path;

/*
 * Most of this logic for updating a repo is taken from libgit2 "pull" example and is listed as
 * public domain. It has since been modified to include things like force resetting a repo.
 */

fn do_fetch<'a>(
    repo: &'a git2::Repository,
    refs: &[&str],
    remote: &'a mut git2::Remote,
) -> Result<git2::AnnotatedCommit<'a>, git2::Error> {
    let mut fo = git2::FetchOptions::new();
    // Always fetch all tags.
    // Perform a download and also update tips
    fo.download_tags(git2::AutotagOption::All);
    remote.fetch(refs, Some(&mut fo), None)?;
    let fetch_head = repo.find_reference("FETCH_HEAD")?;
    Ok(repo.reference_to_annotated_commit(&fetch_head)?)
}

pub fn clone_repo<P: AsRef<Path>>(path: P, repo: &Repo) -> Result<()> {
    git2::Repository::clone(&repo.clone_url, path.as_ref())?;
    Ok(())
}

fn normal_merge(
    repo: &git2::Repository,
    local: &git2::AnnotatedCommit,
    remote: &git2::AnnotatedCommit,
) -> Result<(), git2::Error> {
    let remote_obj = repo.find_commit(remote.id())?.into_object();
    let mut checkout = git2::build::CheckoutBuilder::new();
    checkout.force();
    repo.reset(&remote_obj, git2::ResetType::Hard, Some(&mut checkout))?;
    Ok(())
}

fn do_merge<'a>(
    repo: &'a git2::Repository,
    remote_branch: &str,
    fetch_commit: git2::AnnotatedCommit<'a>,
) -> Result<(), git2::Error> {
    let head_commit = repo.reference_to_annotated_commit(&repo.head()?)?;
    normal_merge(&repo, &head_commit, &fetch_commit)?;
    Ok(())
}

pub fn update_repo<P: AsRef<Path>>(path: P, repo: &Repo) -> Result<()> {
    let r = git2::Repository::open(path.as_ref())?;
    let remote_branch = &repo.default_branch;
    let mut remote = r.find_remote("origin")?;
    let fetch_commit = do_fetch(&r, &[remote_branch], &mut remote)?;
    do_merge(&r, &remote_branch, fetch_commit)?;

    Ok(())
}
