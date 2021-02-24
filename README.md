# github-sync

The goal of this tool is to iterate over all of an Organization's repositories,
a user's repositories or a list of individual repositories and keep them in
sync locally. One can use the `-t` flag to specify a number of threads which
will control how many threads are actively pulling repos locally. Currently we
have opted to log errors rather than fail hard.

It currently seems to handle force pushes okay, but I have only done basic
testing. It currently doesn't pull all branches locally but that may be a nice
change to make in the future.

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
❯ cargo r -- -c example.toml -d /tmp
    Finished dev [unoptimized + debuginfo] target(s) in 0.21s
     Running `target/debug/github-sync -c example.toml -d /tmp`
Feb 24 14:37:19.738 INFO synced notracking into "/tmp/papertigers/notracking"
```
