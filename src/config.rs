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

use std::borrow::Cow;
use std::path::PathBuf;
use std::process::Command;
use std::{env, fmt};

use jujutsu_lib::settings::UserSettings;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ConfigError {
    #[error(transparent)]
    ConfigReadError(#[from] config::ConfigError),
    #[error("Both {0} and {1} exist. Please consolidate your configs in one of them.")]
    AmbiguousSource(PathBuf, PathBuf),
}

fn config_path() -> Result<Option<PathBuf>, ConfigError> {
    if let Ok(config_path) = env::var("JJ_CONFIG") {
        // TODO: We should probably support colon-separated (std::env::split_paths)
        // paths here
        Ok(Some(PathBuf::from(config_path)))
    } else {
        // TODO: Should we drop the final `/config.toml` and read all files in the
        // directory?
        let platform_specific_config_path = dirs::config_dir()
            .map(|config_dir| config_dir.join("jj").join("config.toml"))
            .filter(|path| path.exists());
        let home_config_path = dirs::home_dir()
            .map(|home_dir| home_dir.join(".jjconfig.toml"))
            .filter(|path| path.exists());
        match (&platform_specific_config_path, &home_config_path) {
            (Some(xdg_config_path), Some(home_config_path)) => Err(ConfigError::AmbiguousSource(
                xdg_config_path.clone(),
                home_config_path.clone(),
            )),
            _ => Ok(platform_specific_config_path.or(home_config_path)),
        }
    }
}

/// Environment variables that should be overridden by config values
fn env_base() -> config::Config {
    let mut builder = config::Config::builder();
    if env::var("NO_COLOR").is_ok() {
        // "User-level configuration files and per-instance command-line arguments
        // should override $NO_COLOR." https://no-color.org/
        builder = builder.set_override("ui.color", "never").unwrap();
    }
    if let Ok(value) = env::var("PAGER") {
        builder = builder.set_override("ui.pager", value).unwrap();
    }
    if let Ok(value) = env::var("VISUAL") {
        builder = builder.set_override("ui.editor", value).unwrap();
    } else if let Ok(value) = env::var("EDITOR") {
        builder = builder.set_override("ui.editor", value).unwrap();
    }

    builder.build().unwrap()
}

fn default_mergetool_config() -> config::Config {
    config::Config::builder()
        .add_source(config::File::from_str(
            r#"
                [merge-tools]
                meld.merge-args    = ["$left", "$base", "$right",
                                      "-o", "$output", "--auto-merge"]
                kdiff3.merge-args  = ["$base", "$left", "$right",
                                      "-o", "$output", "--auto"]
                vimdiff.program = "vim"
                vimdiff.merge-args = ["-f", "-d", "$output", "-M",
                                      "$left", "$base", "$right",
                                      "-c", "wincmd J", "-c", "set modifiable",
                                      "-c", "set write"]
                vimdiff.merge-tool-edits-conflict-markers=true
            "#,
            config::FileFormat::Toml,
        ))
        .build()
        .unwrap()
}

/// Environment variables that override config values
fn env_overrides() -> config::Config {
    let mut builder = config::Config::builder();
    if let Ok(value) = env::var("JJ_USER") {
        builder = builder.set_override("user.name", value).unwrap();
    }
    if let Ok(value) = env::var("JJ_EMAIL") {
        builder = builder.set_override("user.email", value).unwrap();
    }
    if let Ok(value) = env::var("JJ_TIMESTAMP") {
        builder = builder.set_override("user.timestamp", value).unwrap();
    }
    if let Ok(value) = env::var("JJ_OP_TIMESTAMP") {
        builder = builder.set_override("operation.timestamp", value).unwrap();
    }
    if let Ok(value) = env::var("JJ_OP_HOSTNAME") {
        builder = builder.set_override("operation.hostname", value).unwrap();
    }
    if let Ok(value) = env::var("JJ_OP_USERNAME") {
        builder = builder.set_override("operation.username", value).unwrap();
    }
    if let Ok(value) = env::var("JJ_EDITOR") {
        builder = builder.set_override("ui.editor", value).unwrap();
    }
    builder.build().unwrap()
}

pub fn read_config() -> Result<UserSettings, ConfigError> {
    let mut config_builder = config::Config::builder()
        .add_source(default_mergetool_config())
        .add_source(env_base());

    if let Some(config_path) = config_path()? {
        let mut files = vec![];
        if config_path.is_dir() {
            if let Ok(read_dir) = config_path.read_dir() {
                // TODO: Walk the directory recursively?
                for dir_entry in read_dir.flatten() {
                    let path = dir_entry.path();
                    if path.is_file() {
                        files.push(path);
                    }
                }
            }
            files.sort();
        } else {
            files.push(config_path);
        }
        for file in files {
            // TODO: Accept other formats and/or accept only certain file extensions?
            config_builder = config_builder.add_source(
                config::File::from(file)
                    .required(false)
                    .format(config::FileFormat::Toml),
            );
        }
    };

    let config = config_builder.add_source(env_overrides()).build()?;
    Ok(UserSettings::from_config(config))
}

/// Command name and arguments specified by config.
#[derive(Clone, Debug, Eq, Hash, PartialEq, serde::Deserialize)]
#[serde(untagged)]
pub enum FullCommandArgs {
    String(String),
    Vec(NonEmptyCommandArgsVec),
}

impl FullCommandArgs {
    /// Returns arguments including the command name.
    ///
    /// The list is not empty, but each element may be an empty string.
    pub fn args(&self) -> Cow<[String]> {
        match self {
            // Handle things like `EDITOR=emacs -nw` (TODO: parse shell escapes)
            FullCommandArgs::String(s) => s.split(' ').map(|s| s.to_owned()).collect(),
            FullCommandArgs::Vec(a) => Cow::Borrowed(&a.0),
        }
    }

    /// Returns process builder configured with this.
    pub fn to_command(&self) -> Command {
        let full_args = self.args();
        let mut cmd = Command::new(&full_args[0]);
        cmd.args(&full_args[1..]);
        cmd
    }
}

impl<T: AsRef<str> + ?Sized> From<&T> for FullCommandArgs {
    fn from(s: &T) -> Self {
        FullCommandArgs::String(s.as_ref().to_owned())
    }
}

impl fmt::Display for FullCommandArgs {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FullCommandArgs::String(s) => write!(f, "{s}"),
            // TODO: format with shell escapes
            FullCommandArgs::Vec(a) => write!(f, "{}", a.0.join(" ")),
        }
    }
}

/// Wrapper to reject an array without command name.
// Based on https://github.com/serde-rs/serde/issues/939
#[derive(Clone, Debug, Eq, Hash, PartialEq, serde::Deserialize)]
#[serde(try_from = "Vec<String>")]
pub struct NonEmptyCommandArgsVec(Vec<String>);

impl TryFrom<Vec<String>> for NonEmptyCommandArgsVec {
    type Error = &'static str;

    fn try_from(args: Vec<String>) -> Result<Self, Self::Error> {
        if args.is_empty() {
            Err("command arguments should not be empty")
        } else {
            Ok(NonEmptyCommandArgsVec(args))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_command_args() {
        let config = config::Config::builder()
            .set_override("empty_array", Vec::<String>::new())
            .unwrap()
            .set_override("empty_string", "")
            .unwrap()
            .set_override("array", vec!["emacs", "-nw"])
            .unwrap()
            .set_override("string", "emacs -nw")
            .unwrap()
            .build()
            .unwrap();

        assert!(config.get::<FullCommandArgs>("empty_array").is_err());

        let args: FullCommandArgs = config.get("empty_string").unwrap();
        assert_eq!(args, FullCommandArgs::String("".to_owned()));
        assert_eq!(args.args(), [""].as_ref());

        let args: FullCommandArgs = config.get("array").unwrap();
        assert_eq!(
            args,
            FullCommandArgs::Vec(NonEmptyCommandArgsVec(
                ["emacs", "-nw",].map(|s| s.to_owned()).to_vec()
            ))
        );
        assert_eq!(args.args(), ["emacs", "-nw"].as_ref());

        let args: FullCommandArgs = config.get("string").unwrap();
        assert_eq!(args, FullCommandArgs::String("emacs -nw".to_owned()));
        assert_eq!(args.args(), ["emacs", "-nw"].as_ref());
    }
}
