use anyhow::{anyhow, Result};
use erased_serde::Serialize;
use regex::Regex;
use reqwest::blocking::{Client, Request, Response};
use reqwest::{header, Method, Url};
use serde::Deserialize;
use std::sync::Arc;

/// Endpoint for the Github API.
const ENDPOINT: &str = "https://api.github.com";
/// API Version info for Accept Header.
const APIVERSION: &str = "application/vnd.github.v3+json";

#[derive(Debug, Deserialize)]
pub struct Repo {
    pub id: u32,
    pub name: String,
    pub full_name: String,
    pub clone_url: String,
    pub default_branch: String,
}

pub enum RepoType {
    User,
    Org,
}

pub struct GithubRepos {
    client: Arc<Github>,
    path: String,
    repos: <Vec<Repo> as IntoIterator>::IntoIter,
    per_page: u32,
    page: u32,
    last: u32,
}

impl GithubRepos {
    fn new(client: &Github, name: &str, rt: RepoType) -> Self {
        let path = match rt {
            RepoType::User => format!("users/{}/repos", name),
            RepoType::Org => format!("orgs/{}/repos", name),
        };

        Self {
            client: Arc::new(client.clone()),
            path,
            repos: vec![].into_iter(),
            per_page: 100, // github max
            page: 0,
            last: 0,
        }
    }

    fn get_page(&mut self, page: u32) -> Result<()> {
        lazy_static::lazy_static! {
            static ref PAGE: Regex = Regex::new(r#"page=(\d+)>; rel="last"$"#).unwrap();
        }
        let page = ("page", page);
        let per_page = ("per_page", self.per_page);

        let query: Vec<&dyn Serialize> =
            vec![&("type", "public"), &per_page, &page];

        let req = self.client.request(Method::GET, &self.path, Some(&query))?;
        let res = self.client.execute(req)?;

        if let Some(header) = res.headers().get("link") {
            for line in header.to_str()?.split(',') {
                if let Some(cap) = PAGE.captures(line) {
                    self.last = cap[1].parse::<u32>()?;
                }
            }
        } else {
            self.last = 1;
        }

        let repos: Vec<Repo> = res.json()?;
        self.repos = repos.into_iter();

        Ok(())
    }

    fn try_next(&mut self) -> Result<Option<Repo>> {
        if let Some(repo) = self.repos.next() {
            return Ok(Some(repo));
        }

        if self.page > 0 && self.page > self.last {
            return Ok(None);
        }

        self.page += 1;
        self.get_page(self.page)?;
        Ok(self.repos.next())
    }
}

impl Iterator for GithubRepos {
    type Item = Result<Repo>;

    fn next(&mut self) -> Option<Self::Item> {
        match self.try_next() {
            Ok(Some(repo)) => Some(Ok(repo)),
            Ok(None) => None,
            Err(err) => Some(Err(err)),
        }
    }
}

#[derive(Clone)]
pub struct Github {
    client: Arc<Client>,
    user: Option<String>,
    token: Option<String>,
}

impl Github {
    /// Create a new `Github` client.
    pub fn new<U, T>(user: U, token: T) -> Self
    where
        U: Into<Option<String>>,
        T: Into<Option<String>>,
    {
        let mut headers = header::HeaderMap::new();
        headers.append(
            header::ACCEPT,
            header::HeaderValue::from_static(APIVERSION),
        );

        Self {
            user: user.into(),
            token: token.into(),
            client: Arc::new(
                Client::builder()
                    .user_agent("github-sync")
                    .default_headers(headers)
                    .build()
                    .unwrap(),
            ),
        }
    }

    /// Form a request to the github API.
    fn request(
        &self,
        method: Method,
        path: &str,
        query: Option<&dyn Serialize>,
    ) -> Result<Request> {
        let base = Url::parse(ENDPOINT)?;
        let url = base.join(path)?;

        let mut builder = self.client.request(method, url);

        if self.user.is_some() {
            builder = builder
                .basic_auth(self.user.clone().unwrap(), self.token.clone());
        }

        if let Some(query) = query {
            builder = builder.query(query);
        }

        let req = builder.build()?;

        Ok(req)
    }

    /// Execute a `Request` via the internal `Client`.
    fn execute(&self, req: Request) -> Result<Response> {
        let res = self.client.execute(req)?;
        match res.status() {
            reqwest::StatusCode::OK => (),
            sc => {
                let url = res.url().as_str().to_string();
                let msg = res.text().expect("failed to get error body");
                return Err(anyhow!("{} ({}) - {}", url, sc, msg));
            }
        };

        Ok(res)
    }

    /// Get a repo by owner and name.
    pub fn get_single_repo<O, R>(&self, owner: O, repo: R) -> Result<Repo>
    where
        O: AsRef<str>,
        R: AsRef<str>,
    {
        let path = format!("repos/{}/{}", owner.as_ref(), repo.as_ref());
        let req = self.request(Method::GET, &path, None)?;
        let res = self.execute(req)?;
        let repo: Repo = res.json()?;

        Ok(repo)
    }

    /// Get all of the public repos for a github user or organization.
    pub fn get_repos(&self, name: &str, rt: RepoType) -> GithubRepos {
        GithubRepos::new(self, name, rt)
    }
}
