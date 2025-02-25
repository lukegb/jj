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

use crate::common::TestEnvironment;

pub mod common;

#[test]
fn test_print() {
    let test_env = TestEnvironment::default();
    test_env.jj_cmd_success(test_env.env_root(), &["init", "repo", "--git"]);
    let repo_path = test_env.env_root().join("repo");

    std::fs::write(repo_path.join("file1"), "a\n").unwrap();
    test_env.jj_cmd_success(&repo_path, &["new"]);
    std::fs::write(repo_path.join("file1"), "b\n").unwrap();
    std::fs::create_dir(repo_path.join("dir")).unwrap();
    std::fs::write(repo_path.join("dir").join("file2"), "c\n").unwrap();

    let stdout = test_env.jj_cmd_success(&repo_path, &["print", "file1", "-r", "@-"]);
    insta::assert_snapshot!(stdout, @r###"
    a
    "###);
    let stdout = test_env.jj_cmd_success(&repo_path, &["print", "file1"]);
    insta::assert_snapshot!(stdout, @r###"
    b
    "###);
    let subdir_file = if cfg!(unix) {
        "dir/file2"
    } else {
        "dir\\file2"
    };
    let stdout = test_env.jj_cmd_success(&repo_path, &["print", subdir_file]);
    insta::assert_snapshot!(stdout, @r###"
    c
    "###);
    let stderr = test_env.jj_cmd_failure(&repo_path, &["print", "nonexistent"]);
    insta::assert_snapshot!(stderr, @r###"
    Error: No such path
    "###);
    let stderr = test_env.jj_cmd_failure(&repo_path, &["print", "dir"]);
    insta::assert_snapshot!(stderr, @r###"
    Error: Path exists but is not a file
    "###);

    // Can print a conflict
    test_env.jj_cmd_success(&repo_path, &["new"]);
    std::fs::write(repo_path.join("file1"), "c\n").unwrap();
    test_env.jj_cmd_success(&repo_path, &["rebase", "-r", "@", "-d", "@--"]);
    let stdout = test_env.jj_cmd_success(&repo_path, &["print", "file1"]);
    insta::assert_snapshot!(stdout, @r###"
    <<<<<<<
    %%%%%%%
    -b
    +a
    +++++++
    c
    >>>>>>>
    "###);
}
