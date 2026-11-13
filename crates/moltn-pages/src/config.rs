//! The optional `moltn.toml`: where the frontend and the pages live, and how `moltn dev`
//! runs the application. Without the file, every value keeps its default.
//!
//! ```toml
//! [front]
//! dir = "web"                      # frontend folder, relative to moltn.toml
//!
//! [pages]
//! dir = "src/pages"                # relative to front.dir
//! typescript = "src/generated/moltn.ts"   # relative to front.dir
//!
//! [dev]
//! port = 3000
//! vite_port = 5173
//! cargo = ["run", "-p", "my-app"]  # cargo arguments that run the application
//! watch = ["server/src", "server/Cargo.toml"]   # changes that restart it
//! ```
//!
//! The file is looked up in the starting folder, then in its parents: one `moltn.toml` at
//! the root of a workspace serves both the `moltn` command and the application's `build.rs`.

use std::path::{Path, PathBuf};

use serde::Deserialize;

/// Name of the configuration file.
pub const FILE: &str = "moltn.toml";

/// `moltn.toml` could not be read or is invalid.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ConfigError {
    /// The file exists but could not be read.
    #[error("reading {}: {source}", path.display())]
    Io {
        /// File.
        path: PathBuf,
        /// I/O error.
        source: std::io::Error,
    },
    /// Syntax error, unknown key or wrong type; the message names the key.
    #[error("invalid {}: {message}", path.display())]
    Invalid {
        /// File.
        path: PathBuf,
        /// Parse error.
        message: String,
    },
}

/// Effective configuration: the content of `moltn.toml` over the defaults.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// `[front]`
    pub front: Front,
    /// `[pages]`
    pub pages: Pages,
    /// `[dev]`
    pub dev: Dev,
    /// Folder every path is relative to: the one holding `moltn.toml`, or the starting folder.
    #[serde(skip)]
    pub root: PathBuf,
    /// The file that was read, `None` when only defaults apply.
    #[serde(skip)]
    pub file: Option<PathBuf>,
}

/// `[front]`: the frontend project.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Front {
    /// Frontend folder, relative to `moltn.toml`. Default: `web`.
    pub dir: PathBuf,
}

impl Default for Front {
    fn default() -> Self {
        Self { dir: "web".into() }
    }
}

/// `[pages]`: file-based routing.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Pages {
    /// Pages folder, relative to the frontend folder. Default: `src/pages`.
    pub dir: PathBuf,
    /// Generated TypeScript module, relative to the frontend folder.
    /// Default: `generated/moltn.ts` next to the pages folder.
    pub typescript: Option<PathBuf>,
}

impl Default for Pages {
    fn default() -> Self {
        Self {
            dir: "src/pages".into(),
            typescript: None,
        }
    }
}

/// `[dev]`: what `moltn dev` (and `moltn gen`) run and watch.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct Dev {
    /// Application port. Default: 3000.
    pub port: u16,
    /// Vite port. Default: 5173.
    pub vite_port: u16,
    /// Cargo arguments that run the application. Default: `["run"]`.
    pub cargo: Vec<String>,
    /// Files and folders whose changes restart the application, relative to `moltn.toml`.
    /// Default: `src`, `Cargo.toml`, `build.rs`.
    pub watch: Vec<PathBuf>,
}

impl Default for Dev {
    fn default() -> Self {
        Self {
            port: 3000,
            vite_port: 5173,
            cargo: vec!["run".into()],
            watch: vec!["src".into(), "Cargo.toml".into(), "build.rs".into()],
        }
    }
}

impl Config {
    /// Read the `moltn.toml` of `start` or of its nearest parent that has one. Without any,
    /// the defaults apply, relative to `start`.
    ///
    /// ```no_run
    /// let config = moltn_pages::config::Config::load(".")?;
    /// println!("pages: {}", config.pages_dir().display());
    /// # Ok::<(), moltn_pages::config::ConfigError>(())
    /// ```
    pub fn load(start: impl AsRef<Path>) -> Result<Self, ConfigError> {
        let start = start.as_ref();
        let Some(file) = start
            .ancestors()
            .map(|dir| dir.join(FILE))
            .find(|f| f.is_file())
        else {
            return Ok(Self {
                root: start.to_path_buf(),
                ..Self::default()
            });
        };
        let text = std::fs::read_to_string(&file).map_err(|source| ConfigError::Io {
            path: file.clone(),
            source,
        })?;
        let mut config: Self = toml::from_str(&text).map_err(|e| ConfigError::Invalid {
            path: file.clone(),
            message: e.to_string(),
        })?;
        // `ancestors()` d'un chemin relatif finit par "" : le dossier courant.
        config.root = file.parent().unwrap_or(Path::new("")).to_path_buf();
        config.file = Some(file);
        Ok(config)
    }

    /// Frontend folder.
    pub fn front_dir(&self) -> PathBuf {
        self.root.join(&self.front.dir)
    }

    /// Pages folder.
    pub fn pages_dir(&self) -> PathBuf {
        self.front_dir().join(&self.pages.dir)
    }

    /// Generated TypeScript module (`moltn.ts`).
    pub fn typescript(&self) -> PathBuf {
        match &self.pages.typescript {
            Some(path) => self.front_dir().join(path),
            None => default_typescript(&self.pages_dir()),
        }
    }
}

/// Convention : `generated/moltn.ts` à côté du dossier des pages.
pub(crate) fn default_typescript(pages: &Path) -> PathBuf {
    pages
        .parent()
        .unwrap_or(Path::new(""))
        .join("generated")
        .join("moltn.ts")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_without_file() {
        let dir = tempfile::tempdir().unwrap();
        let config = Config::load(dir.path()).unwrap();
        assert_eq!(config.file, None);
        assert_eq!(config.pages_dir(), dir.path().join("web/src/pages"));
        assert_eq!(
            config.typescript(),
            dir.path().join("web/src/generated/moltn.ts")
        );
        assert_eq!(config.dev, Dev::default());
    }

    #[test]
    fn found_in_a_parent_and_paths_follow_it() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join(FILE),
            "[front]\ndir = \"front\"\n[pages]\ndir = \"app/pages\"\n[dev]\nport = 4000\ncargo = [\"run\", \"-p\", \"api\"]\nwatch = [\"server/src\"]\n",
        )
        .unwrap();
        let server = root.path().join("server");
        std::fs::create_dir(&server).unwrap();

        let config = Config::load(&server).unwrap();
        assert_eq!(config.root, root.path());
        assert_eq!(config.pages_dir(), root.path().join("front/app/pages"));
        // Le fichier TypeScript suit le dossier des pages tant qu'il n'est pas précisé.
        assert_eq!(
            config.typescript(),
            root.path().join("front/app/generated/moltn.ts")
        );
        assert_eq!(config.dev.port, 4000);
        assert_eq!(config.dev.vite_port, 5173);
        assert_eq!(config.dev.cargo, ["run", "-p", "api"]);
        assert_eq!(config.dev.watch, [PathBuf::from("server/src")]);
    }

    #[test]
    fn unknown_key_is_an_error_that_names_it() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(FILE), "[pages]\ndirectory = \"x\"\n").unwrap();
        let message = Config::load(dir.path()).unwrap_err().to_string();
        assert!(
            message.contains("directory") && message.contains("`dir`"),
            "{message}"
        );
    }
}
