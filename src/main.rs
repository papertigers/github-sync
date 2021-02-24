use anyhow::{anyhow, Context, Result};
use rayon::iter::ParallelBridge;
use rayon::prelude::*;
use slog::*;
use std::path::PathBuf;
use std::sync::Arc;

mod config;
mod git;
mod github;
use github::{Github, Repo, RepoType};

#[derive(Clone)]
struct Ctx {
    log: Arc<Logger>,
    dir: PathBuf,
    gh: Arc<Github>,
}

fn create_logger() -> Logger {
    let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());
    Logger::root(slog_term::FullFormat::new(plain).build().fuse(), o!())
}

fn sync_repo(ctx: &mut Ctx, repo: &Repo) -> Result<()> {
    let path = ctx.dir.join(&repo.full_name);

    // attempt to clone the repo first
    if let Err(e) = git::clone_repo(&path, repo) {
        match e.downcast_ref::<git2::Error>() {
            None => return Err(e),
            Some(rc) => {
                if rc.code() == git2::ErrorCode::Exists {
                    git::update_repo(&path, repo)?;
                } else {
                    return Err(anyhow!("{} - {}", repo.name, e));
                }
            }
        }
    };

    info!(ctx.log, "synced {} into {:?}", repo.name, path);
    Ok(())
}

fn process_owner_repos(ctx: Ctx, repos: &[Repo]) {
    let errors: Vec<_> = repos
        .into_par_iter()
        .map_with(ctx.clone(), |c, r| sync_repo(c, r))
        .filter(|r| r.is_err())
        .collect();

    // XXX For now log errors in a non fatal way.
    for e in errors {
        error!(ctx.log, "{:?}", e);
    }
}

fn process_repos<N: AsRef<str>>(ctx: Ctx, name: N, rt: RepoType) -> Result<()> {
    let name = name.as_ref();

    let errors: Vec<_> = ctx
        .gh
        .get_repos(name, rt)
        .into_iter()
        .par_bridge()
        .map_with(ctx.clone(), |c, r| sync_repo(c, &r?))
        .filter(|r| r.is_err())
        .collect();

    // XXX For now log errors in a non fatal way.
    for e in errors {
        error!(ctx.log, "{:?}", e);
    }

    Ok(())
}

fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();
    let program = &args[0].clone();
    let brief = format!("Usage: {} [options] -c CONFIG", program);

    let mut opts = getopts::Options::new();
    opts.reqopt("c", "", "config file", "CONFIG");
    opts.optopt("d", "", "directory to sync git repos in", "DIRECTORY");
    opts.optopt(
        "t",
        "",
        "max number of threads used to sync repos (default 1)",
        "THREADS",
    );

    let matches = match opts.parse(&args[1..]) {
        Ok(m) => m,
        Err(f) => {
            anyhow::bail!("{}\n{}", f, opts.usage(&brief));
        }
    };

    // Safe unwrap because "CONFIG" is a required argument.
    let config = config::Config::from_file(matches.opt_str("c").unwrap())?;

    let mut ctx = Ctx {
        log: Arc::new(create_logger()),
        dir: PathBuf::new(),
        gh: Arc::new(Github::new(config.user, config.token)),
    };

    if let Some(path) = matches.opt_str("d") {
        ctx.dir.push(path);
    }

    if ctx.dir.as_os_str().is_empty() {
        ctx.dir = std::env::current_dir()?;
    }

    let threads = matches.opt_get_default("t", 1)?;
    rayon::ThreadPoolBuilder::new()
        .num_threads(threads)
        .build_global()
        .context("failed to build thread pool")?;

    if let Some(owner) = config.owner {
        for (ref o, opts) in owner {
            let (repos, errors): (Vec<_>, Vec<_>) = opts
                .repos
                .par_iter()
                .map(|n| ctx.gh.get_single_repo(o, n))
                .partition(Result::is_ok);
            let repos: Vec<_> = repos.into_iter().map(Result::unwrap).collect();
            let errors: Vec<_> = errors.into_iter().map(Result::unwrap_err).collect();

            // XXX For now log errors in a non fatal way.
            for e in errors {
                error!(ctx.log, "{}", e);
            }

            process_owner_repos(ctx.clone(), &repos);
        }
    }

    if let Some(orgs) = config.organizations {
        for org in orgs {
            process_repos(ctx.clone(), org, RepoType::Org)?;
        }
    }

    if let Some(users) = config.users {
        for user in users {
            process_repos(ctx.clone(), user, RepoType::User)?;
        }
    }

    Ok(())
}
