// Copyright 2022 The Jujutsu Authors
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
// https://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use common::{get_stderr_string, get_stdout_string, TestEnvironment};
use regex::Regex;

pub mod common;

#[test]
fn test_log_with_empty_revision() {
    let test_env = TestEnvironment::default();
    test_env.jj_cmd_success(test_env.env_root(), &["init", "repo", "--git"]);
    let repo_path = test_env.env_root().join("repo");

    let stderr = test_env.jj_cmd_cli_error(&repo_path, &["log", "-r="]);
    insta::assert_snapshot!(stderr, @r###"
    error: The argument '--revisions <REVISIONS>' requires a value but none was supplied

    For more information try '--help'
    "###);
}

#[test]
fn test_log_with_or_without_diff() {
    let test_env = TestEnvironment::default();
    test_env.jj_cmd_success(test_env.env_root(), &["init", "repo", "--git"]);
    let repo_path = test_env.env_root().join("repo");

    std::fs::write(repo_path.join("file1"), "foo\n").unwrap();
    test_env.jj_cmd_success(&repo_path, &["describe", "-m", "add a file"]);
    test_env.jj_cmd_success(&repo_path, &["new", "-m", "a new commit"]);
    std::fs::write(repo_path.join("file1"), "foo\nbar\n").unwrap();

    let stdout = test_env.jj_cmd_success(&repo_path, &["log", "-T", "description"]);
    insta::assert_snapshot!(stdout, @r###"
    @ a new commit
    o add a file
    o (no description set)
    "###);

    let stdout = test_env.jj_cmd_success(&repo_path, &["log", "-T", "description", "-p"]);
    insta::assert_snapshot!(stdout, @r###"
    @ a new commit
    | Modified regular file file1:
    |    1    1: foo
    |         2: bar
    o add a file
    | Added regular file file1:
    |         1: foo
    o (no description set)
    "###);

    let stdout = test_env.jj_cmd_success(&repo_path, &["log", "-T", "description", "--no-graph"]);
    insta::assert_snapshot!(stdout, @r###"
    a new commit
    add a file
    (no description set)
    "###);

    let stdout = test_env.jj_cmd_success(
        &repo_path,
        &["log", "-T", "description", "--no-graph", "-p", "--git"],
    );
    insta::assert_snapshot!(stdout, @r###"
    a new commit
    diff --git a/file1 b/file1
    index 257cc5642c...3bd1f0e297 100644
    --- a/file1
    +++ b/file1
    @@ -1,1 +1,2 @@
     foo
    +bar
    add a file
    diff --git a/file1 b/file1
    new file mode 100644
    index 0000000000..257cc5642c
    --- /dev/null
    +++ b/file1
    @@ -1,0 +1,1 @@
    +foo
    (no description set)
    "###);

    // `-s` implies `-p`, with or without graph
    let stdout = test_env.jj_cmd_success(&repo_path, &["log", "-T", "description", "-s"]);
    insta::assert_snapshot!(stdout, @r###"
    @ a new commit
    | M file1
    o add a file
    | A file1
    o (no description set)
    "###);
    let stdout = test_env.jj_cmd_success(
        &repo_path,
        &["log", "-T", "description", "--no-graph", "-s"],
    );
    insta::assert_snapshot!(stdout, @r###"
    a new commit
    M file1
    add a file
    A file1
    (no description set)
    "###);

    // `--git` implies `-p`, with or without graph
    let stdout = test_env.jj_cmd_success(
        &repo_path,
        &["log", "-T", "description", "-r", "@", "--git"],
    );
    insta::assert_snapshot!(stdout, @r###"
    @ a new commit
    ~ diff --git a/file1 b/file1
      index 257cc5642c...3bd1f0e297 100644
      --- a/file1
      +++ b/file1
      @@ -1,1 +1,2 @@
       foo
      +bar
    "###);
    let stdout = test_env.jj_cmd_success(
        &repo_path,
        &["log", "-T", "description", "-r", "@", "--no-graph", "--git"],
    );
    insta::assert_snapshot!(stdout, @r###"
    a new commit
    diff --git a/file1 b/file1
    index 257cc5642c...3bd1f0e297 100644
    --- a/file1
    +++ b/file1
    @@ -1,1 +1,2 @@
     foo
    +bar
    "###);
}

#[test]
fn test_log_reversed() {
    let test_env = TestEnvironment::default();
    test_env.jj_cmd_success(test_env.env_root(), &["init", "repo", "--git"]);
    let repo_path = test_env.env_root().join("repo");

    test_env.jj_cmd_success(&repo_path, &["describe", "-m", "first"]);
    test_env.jj_cmd_success(&repo_path, &["new", "-m", "second"]);

    let stdout = test_env.jj_cmd_success(&repo_path, &["log", "-T", "description", "--reversed"]);
    insta::assert_snapshot!(stdout, @r###"
    o (no description set)
    o first
    @ second
    "###);

    let stdout = test_env.jj_cmd_success(
        &repo_path,
        &["log", "-T", "description", "--reversed", "--no-graph"],
    );
    insta::assert_snapshot!(stdout, @r###"
    (no description set)
    first
    second
    "###);
}

#[test]
fn test_log_filtered_by_path() {
    let test_env = TestEnvironment::default();
    test_env.jj_cmd_success(test_env.env_root(), &["init", "repo", "--git"]);
    let repo_path = test_env.env_root().join("repo");

    std::fs::write(repo_path.join("file1"), "foo\n").unwrap();
    test_env.jj_cmd_success(&repo_path, &["describe", "-m", "first"]);
    test_env.jj_cmd_success(&repo_path, &["new", "-m", "second"]);
    std::fs::write(repo_path.join("file1"), "foo\nbar\n").unwrap();
    std::fs::write(repo_path.join("file2"), "baz\n").unwrap();

    let stdout = test_env.jj_cmd_success(&repo_path, &["log", "-T", "description", "file1"]);
    insta::assert_snapshot!(stdout, @r###"
    @ second
    o first
    ~ 
    "###);

    let stdout = test_env.jj_cmd_success(&repo_path, &["log", "-T", "description", "file2"]);
    insta::assert_snapshot!(stdout, @r###"
    @ second
    ~ 
    "###);

    let stdout = test_env.jj_cmd_success(&repo_path, &["log", "-T", "description", "-s", "file1"]);
    insta::assert_snapshot!(stdout, @r###"
    @ second
    | M file1
    o first
    ~ A file1
    "###);

    let stdout = test_env.jj_cmd_success(
        &repo_path,
        &["log", "-T", "description", "-s", "file2", "--no-graph"],
    );
    insta::assert_snapshot!(stdout, @r###"
    second
    A file2
    "###);

    // file() revset doesn't filter the diff.
    let stdout = test_env.jj_cmd_success(
        &repo_path,
        &[
            "log",
            "-T",
            "description",
            "-s",
            "-rfile(file2)",
            "--no-graph",
        ],
    );
    insta::assert_snapshot!(stdout, @r###"
    second
    M file1
    A file2
    "###);
}

#[test]
fn test_log_warn_path_might_be_revset() {
    let test_env = TestEnvironment::default();
    test_env.jj_cmd_success(test_env.env_root(), &["init", "repo", "--git"]);
    let repo_path = test_env.env_root().join("repo");

    std::fs::write(repo_path.join("file1"), "foo\n").unwrap();

    // Don't warn if the file actually exists.
    let assert = test_env
        .jj_cmd(&repo_path, &["log", "file1", "-T", "description"])
        .assert()
        .success();
    insta::assert_snapshot!(get_stdout_string(&assert), @r###"
    @ (no description set)
    ~ 
    "###);
    insta::assert_snapshot!(get_stderr_string(&assert), @"");

    // Warn for `jj log .` specifically, for former Mercurial users.
    let assert = test_env
        .jj_cmd(&repo_path, &["log", ".", "-T", "description"])
        .assert()
        .success();
    insta::assert_snapshot!(get_stdout_string(&assert), @r###"
    @ (no description set)
    ~ 
    "###);
    insta::assert_snapshot!(get_stderr_string(&assert), @r###"warning: The argument "." is being interpreted as a path, but this is often not useful because all non-empty commits touch '.'.  If you meant to show the working copy commit, pass -r '@' instead."###);

    // ...but checking `jj log .` makes sense in a subdirectory.
    let subdir = repo_path.join("dir");
    std::fs::create_dir_all(&subdir).unwrap();
    let assert = test_env.jj_cmd(&subdir, &["log", "."]).assert().success();
    insta::assert_snapshot!(get_stdout_string(&assert), @"");
    insta::assert_snapshot!(get_stderr_string(&assert), @"");

    // Warn for `jj log @` instead of `jj log -r @`.
    let assert = test_env
        .jj_cmd(&repo_path, &["log", "@", "-T", "description"])
        .assert()
        .success();
    insta::assert_snapshot!(get_stdout_string(&assert), @"");
    insta::assert_snapshot!(get_stderr_string(&assert), @r###"
    warning: The argument "@" is being interpreted as a path. To specify a revset, pass -r "@" instead.
    "###);

    // Warn when there's no path with the provided name.
    let assert = test_env
        .jj_cmd(&repo_path, &["log", "file2", "-T", "description"])
        .assert()
        .success();
    insta::assert_snapshot!(get_stdout_string(&assert), @"");
    insta::assert_snapshot!(get_stderr_string(&assert), @r###"
    warning: The argument "file2" is being interpreted as a path. To specify a revset, pass -r "file2" instead.
    "###);

    // If an explicit revision is provided, then suppress the warning.
    let assert = test_env
        .jj_cmd(&repo_path, &["log", "@", "-r", "@", "-T", "description"])
        .assert()
        .success();
    insta::assert_snapshot!(get_stdout_string(&assert), @"");
    insta::assert_snapshot!(get_stderr_string(&assert), @r###"
    "###);
}

#[test]
fn test_default_revset() {
    let test_env = TestEnvironment::default();
    test_env.jj_cmd_success(test_env.env_root(), &["init", "repo", "--git"]);
    let repo_path = test_env.env_root().join("repo");

    std::fs::write(repo_path.join("file1"), "foo\n").unwrap();
    test_env.jj_cmd_success(&repo_path, &["describe", "-m", "add a file"]);

    // Set configuration to only show the root commit.
    test_env.add_config(
        br#"[ui]
        default-revset = "root"
        "#,
    );

    // Log should only contain one line (for the root commit), and not show the
    // commit created above.
    assert_eq!(
        1,
        test_env
            .jj_cmd_success(&repo_path, &["log", "-T", "commit_id"])
            .lines()
            .count()
    );
}

#[test]
fn test_log_author_timestamp() {
    let test_env = TestEnvironment::default();
    test_env.jj_cmd_success(test_env.env_root(), &["init", "repo", "--git"]);
    let repo_path = test_env.env_root().join("repo");

    test_env.jj_cmd_success(&repo_path, &["describe", "-m", "first"]);
    test_env.jj_cmd_success(&repo_path, &["new", "-m", "second"]);

    let stdout = test_env.jj_cmd_success(&repo_path, &["log", "-T", "author.timestamp()"]);
    insta::assert_snapshot!(stdout, @r###"
    @ 2001-02-03 04:05:09.000 +07:00
    o 2001-02-03 04:05:07.000 +07:00
    o 1970-01-01 00:00:00.000 +00:00
    "###);
}

#[test]
fn test_log_author_timestamp_ago() {
    let test_env = TestEnvironment::default();
    test_env.jj_cmd_success(test_env.env_root(), &["init", "repo", "--git"]);
    let repo_path = test_env.env_root().join("repo");

    test_env.jj_cmd_success(&repo_path, &["describe", "-m", "first"]);
    test_env.jj_cmd_success(&repo_path, &["new", "-m", "second"]);

    let stdout = test_env.jj_cmd_success(&repo_path, &["log", "-T", "author.timestamp().ago()"]);
    let line_re = Regex::new(r"@|o [0-9]+ years ago").unwrap();
    assert!(
        stdout.lines().all(|x| line_re.is_match(x)),
        "expected every line to match regex"
    );
}
