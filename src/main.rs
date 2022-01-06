use anyhow::{anyhow, Context, Result};
use rayon::iter::ParallelBridge;
use rayon::prelude::*;
use slog::*;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

mod config;
mod git;
mod github;
use config::Config;
use github::{Github, Repo, RepoType};

#[derive(Clone)]
struct Ctx {
    config: Arc<Config>,
    log: Arc<Logger>,
    dir: PathBuf,
    gh: Arc<Github>,
    processed: Arc<AtomicUsize>,
    errors: Arc<Mutex<Vec<anyhow::Error>>>,
}

fn create_logger() -> Logger {
    let plain = slog_term::PlainSyncDecorator::new(std::io::stdout());
    Logger::root(slog_term::FullFormat::new(plain).build().fuse(), o!())
}

fn sync_repo(ctx: &mut Ctx, repo: &Repo) -> Result<()> {
    if let Some(ref ignore_list) = ctx.config.ignore {
        if ignore_list.contains(&repo.full_name) {
            return Ok(());
        }
    }

    ctx.processed.fetch_add(1, Ordering::SeqCst);

    let path = ctx.dir.join(&repo.full_name);

    if let Err(e) = git::clone_or_update(&path, repo) {
        return Err(anyhow!("[{}] failed sync - {}", repo.full_name, e));
    };

    info!(ctx.log, "synced {} into {:?}", repo.name, path);
    Ok(())
}

fn process_owner_repos(ctx: Ctx, repos: &[Repo]) {
    let errors: Vec<_> = repos
        .into_par_iter()
        .map_with(ctx.clone(), sync_repo)
        .filter(|r| r.is_err())
        .map(Result::unwrap_err)
        .collect();

    let mut lock = ctx.errors.lock().unwrap();
    lock.extend(errors.into_iter());
}

fn process_repos<N: AsRef<str>>(ctx: Ctx, name: N, rt: RepoType) {
    let name = name.as_ref();

    let errors: Vec<_> = ctx
        .gh
        .get_repos(name, rt)
        .into_iter()
        .par_bridge()
        .map_with(ctx.clone(), |c, r| sync_repo(c, &r?))
        .filter(|r| r.is_err())
        .map(Result::unwrap_err)
        .collect();

    let mut lock = ctx.errors.lock().unwrap();
    lock.extend(errors.into_iter());
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
    let gh = Arc::new(Github::new(config.user.clone(), config.token.clone()));

    let mut ctx = Ctx {
        config: Arc::new(config),
        log: Arc::new(create_logger()),
        dir: PathBuf::new(),
        gh,
        processed: Arc::new(AtomicUsize::new(0)),
        errors: Default::default(),
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

    if let Some(ref owner) = ctx.config.owner {
        for (ref o, opts) in owner {
            let (repos, errors): (Vec<_>, Vec<_>) = opts
                .repos
                .par_iter()
                .map(|n| ctx.gh.get_single_repo(o, n))
                .partition(Result::is_ok);
            let repos: Vec<_> = repos.into_iter().map(Result::unwrap).collect();

            let mut lock = ctx.errors.lock().unwrap();
            lock.extend(errors.into_iter().map(Result::unwrap_err));
            drop(lock);

            process_owner_repos(ctx.clone(), &repos);
        }
    }

    if let Some(ref orgs) = ctx.config.organizations {
        for org in orgs {
            process_repos(ctx.clone(), org, RepoType::Org);
        }
    }

    if let Some(ref users) = ctx.config.users {
        for user in users {
            process_repos(ctx.clone(), user, RepoType::User);
        }
    }

    // Print summary
    let errors = ctx.errors.lock().unwrap();
    for error in errors.iter() {
        error!(ctx.log, "{}", error);
    }

    info!(
        ctx.log,
        "finished processing {} repo(s), encountered {} error(s)",
        ctx.processed.load(Ordering::SeqCst),
        errors.len()
    );

    if errors.len() != 0 {
        std::process::exit(1);
    }
    Ok(())
}
