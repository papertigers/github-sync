use crate::github::Repo;
use anyhow::Result;
use std::path::Path;

fn do_fetch<'a>(
    repo: &'a git2::Repository,
    refs: &[&str],
    remote: &'a mut git2::Remote,
) -> Result<Option<git2::AnnotatedCommit<'a>>, git2::Error> {
    let mut fo = git2::FetchOptions::new();
    fo.prune(git2::FetchPrune::On);
    remote.fetch(refs, Some(&mut fo), None)?;
    if let Ok(fetch_head) = repo.find_reference("FETCH_HEAD") {
        return Ok(Some(repo.reference_to_annotated_commit(&fetch_head)?));
    }

    Ok(None)
}

fn update_repo<P: AsRef<Path>>(path: P) -> Result<()> {
    let r = git2::Repository::open(path.as_ref())?;
    let mut remote = r.find_remote("origin")?;
    do_fetch(&r, &[], &mut remote)?;

    Ok(())
}

pub fn clone_or_update<P: AsRef<Path>>(path: P, repo: &Repo) -> Result<()> {
    let path = path.as_ref();
    if !path.exists() {
        let mut builder = git2::build::RepoBuilder::new();
        builder
            .bare(true)
            .remote_create(|repo, name, url| repo.remote_with_fetch(name, url, "+refs/*:refs/*"));
        builder.clone(&repo.clone_url, path)?;
    } else {
        update_repo(path)?
    }

    Ok(())
}
