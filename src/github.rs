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
}

pub struct GithubOrgRepos {
    client: Arc<Github>,
    path: String,
    repos: <Vec<Repo> as IntoIterator>::IntoIter,
    per_page: u32,
    page: u32,
    last: u32,
}

impl GithubOrgRepos {
    fn new(client: &Github, org: &str) -> Self {
        Self {
            client: Arc::new(client.clone()),
            path: format!("orgs/{}/repos", org),
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

        let mut query: Vec<&dyn Serialize> = Vec::new();
        query.push(&("type", "public"));
        query.push(&per_page);
        query.push(&page);

        let req = self.client.request(Method::GET, &self.path, Some(&query))?;
        let res = self.client.execute(req)?;

        match res.status() {
            reqwest::StatusCode::OK => (),
            sc => {
                return Err(anyhow!(
                    "Github API Error ({}) - {}",
                    sc,
                    res.text().expect("failed to get error body")
                ))
            }
        };

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

impl Iterator for GithubOrgRepos {
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
        headers.append(header::ACCEPT, header::HeaderValue::from_static(APIVERSION));

        Self {
            user: user.into(),
            token: token.into(),
            client: Arc::new(
                Client::builder()
                    .user_agent("github-org-sync")
                    .default_headers(headers)
                    .build()
                    .unwrap(),
            ),
        }
    }

    /// Form a request to the github API.
    ///
    /// Note: In order to satisfy the query option in the None case one can pass None::<&()>.
    /// This is a wart, but one I am wiling to accept since request is an internal API call.
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
            builder = builder.basic_auth(self.user.clone().unwrap(), self.token.clone());
        }

        if let Some(query) = query {
            builder = builder.query(query);
        }

        let req = builder.build()?;

        Ok(req)
    }

    /// Execute a `Request` via the internal `Client`.
    fn execute(&self, req: Request) -> Result<Response> {
        Ok(self.client.execute(req)?)
    }

    /// Get all of the public repos for a github organization.
    pub fn get_org_repos(&self, org: &str) -> GithubOrgRepos {
        GithubOrgRepos::new(&self, org)
    }
}
