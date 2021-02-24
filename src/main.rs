use anyhow::{Context, Result};
use rayon::iter::ParallelBridge;
use rayon::prelude::*;
use slog::*;
use std::path::PathBuf;
use std::sync::Arc;

mod config;
mod github;
use github::{Github, Repo};

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

fn process_repo(ctx: &mut Ctx, repo: Repo) -> Result<()> {
    let path = ctx.dir.join(repo.full_name);
    info!(ctx.log, "processing {} into {:?}", repo.name, path);

    // TODO clone or update the repo

    Ok(())
}

fn process_org<O: AsRef<str>>(ctx: Ctx, org: O) -> Result<()> {
    let org = org.as_ref();

    let errors: Vec<_> = ctx
        .gh
        .get_org_repos(org)
        .into_iter()
        .par_bridge()
        .map_with(ctx.clone(), |c, r| process_repo(c, r?))
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
        "max number of threads used to sync repos, defaults to 1",
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

    for org in config.organizations {
        process_org(ctx.clone(), org)?;
    }

    Ok(())
}
