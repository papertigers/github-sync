# github-sync

The goal of this tool is to iterate over all of an Organization's repositories,
a user's repositories or a list of individual repositories and keep them in
sync locally. One can use the `-t` flag to specify a number of threads which
will control how many threads are actively pulling repos locally. Currently we
have opted to log errors at the end of a run rather than fail hard.

### Usage

```
Usage: github-sync [options] -c CONFIG

Options:
    -c CONFIG           config file
    -d DIRECTORY        directory to sync git repos in
    -t THREADS          max number of threads used to sync repos (default 1)
```

### Example Config

Note: a user and token are not required but github severely limits API calls
without them. For generating a token see the [creating a personal access token]
article from github.

See [example.toml](example.toml) for all configuration options.

[creating a personal access token]: https://docs.github.com/en/github/authenticating-to-github/creating-a-personal-access-token

```toml
user = "papertigers"
token = "foobarbazfoobarbazfoobarbazfoobarbaz"

[owner.papertigers]
repos = [
    "notracking",
]
```


### Example Run

```
❯ github-sync -d /tmp -c test.toml
Feb 25 14:19:19.193 INFO synced notracking into "/tmp/papertigers/notracking"
Feb 25 14:19:19.193 INFO finished processing 1 repo(s), encountered 0 error(s)
```
