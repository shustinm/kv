use clap::{Parser, Subcommand};
use clap_complete::Shell;
use std::env;
use std::ffi::OsString;
use std::path::PathBuf;

fn default_debug_dir() -> PathBuf {
    env::temp_dir().join("kvim")
}

fn default_kvim_conf() -> PathBuf {
    let home = env::var("HOME").unwrap_or_else(|_| String::from("/tmp"));
    PathBuf::from(home).join(".kvim.conf")
}

#[derive(Debug, Parser)]
#[command(
    name = "kv",
    version,
    about = "Launcher for KoalaVim (neovim configuration)",
    trailing_var_arg = true
)]
pub struct Cli {
    /// Verbose
    #[arg(short, long)]
    pub verbose: bool,

    /// Start KoalaVim in git mode
    #[arg(short, long)]
    pub git: bool,

    /// Start KoalaVim in git tree mode
    #[arg(short, long)]
    pub tree: bool,

    /// Start KoalaVim in git diff mode
    #[arg(long)]
    pub git_diff: bool,

    /// Start KoalaVim in ai mode
    #[arg(long)]
    pub ai: bool,

    /// Start KoalaVim in debug mode, output goes to --debug_dir/<time_stamp>
    #[arg(short, long)]
    pub debug: bool,

    /// Disable noice. disables notifications, helpful for debugging messages
    #[arg(short, long)]
    pub no_noice: bool,

    /// Change output log for debug
    #[arg(long, default_value_os_t = default_debug_dir())]
    pub debug_dir: PathBuf,

    /// Override debug file name (default is timestamp)
    #[arg(long)]
    pub debug_file: Option<String>,

    /// Launch with given kvim.conf
    #[arg(short, long, default_value_os_t = default_kvim_conf())]
    pub cfg: PathBuf,

    /// Launch with given lua cfg
    #[arg(short, long)]
    pub lua_cfg: Option<PathBuf>,

    /// Launch in a virtual koala env
    #[arg(long)]
    pub env: Option<String>,

    /// Arguments to pass to nvim binary.
    /// On mode (git/tree) arguments passed to KoalaVim.
    #[arg(allow_hyphen_values = true)]
    pub nvim_args: Vec<OsString>,

    /// Override nvim's binary path
    #[arg(long)]
    pub nvim_bin_path: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Manage virtual koala envs
    Env {
        #[command(subcommand)]
        action: EnvAction,
    },
    /// Initialize a koala env interactively
    Init {
        /// Name of the env to initialize (default: main)
        #[arg(long)]
        env: Option<String>,
    },
    /// Generate shell completions
    Completions {
        /// Shell to generate completions for
        shell: Shell,
    },
}

#[derive(Debug, Subcommand)]
pub enum EnvAction {
    /// Create a new virtual koala env
    Create {
        /// Name of the env
        name: String,
        /// Copy config from an existing env, path, or git URL
        #[arg(long)]
        from: Option<String>,
        /// Clone a specific branch/tag (only with git URL source)
        #[arg(long)]
        branch: Option<String>,
    },
    /// List virtual koala envs
    List,
    /// Fork an existing env (copies config, data, state, and cache)
    Fork {
        /// Name of the existing env to fork from
        source: String,
        /// Name for the new env
        name: String,
    },
    /// Delete a virtual koala env
    Delete {
        /// Name of the env to delete
        name: String,
        /// Skip confirmation prompt
        #[arg(short, long)]
        force: bool,
    },
    /// Rename a virtual koala env
    Rename {
        /// Current env name
        current: String,
        /// New env name
        new_name: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cli_default_parse() {
        let cli = Cli::try_parse_from(["kv"]).unwrap();
        assert!(!cli.verbose);
        assert!(!cli.git);
        assert!(cli.env.is_none());
        assert!(cli.command.is_none());
    }

    #[test]
    fn test_cli_env_flag() {
        let cli = Cli::try_parse_from(["kv", "--env", "myenv"]).unwrap();
        assert_eq!(cli.env.as_deref(), Some("myenv"));
    }

    #[test]
    fn test_cli_verbose_and_git() {
        let cli = Cli::try_parse_from(["kv", "-v", "-g"]).unwrap();
        assert!(cli.verbose);
        assert!(cli.git);
    }

    #[test]
    fn test_cli_env_create_subcommand() {
        let cli = Cli::try_parse_from(["kv", "env", "create", "test-env"]).unwrap();
        match cli.command {
            Some(Commands::Env {
                action:
                    EnvAction::Create {
                        ref name,
                        ref from,
                        ref branch,
                    },
            }) => {
                assert_eq!(name, "test-env");
                assert!(from.is_none());
                assert!(branch.is_none());
            }
            _ => panic!("Expected Env Create subcommand"),
        }
    }

    #[test]
    fn test_cli_env_create_with_from() {
        let cli =
            Cli::try_parse_from(["kv", "env", "create", "new-env", "--from", "old-env"]).unwrap();
        match cli.command {
            Some(Commands::Env {
                action:
                    EnvAction::Create {
                        ref name,
                        ref from,
                        ref branch,
                    },
            }) => {
                assert_eq!(name, "new-env");
                assert_eq!(from.as_deref(), Some("old-env"));
                assert!(branch.is_none());
            }
            _ => panic!("Expected Env Create subcommand"),
        }
    }

    #[test]
    fn test_cli_env_list_subcommand() {
        let cli = Cli::try_parse_from(["kv", "env", "list"]).unwrap();
        assert!(matches!(
            cli.command,
            Some(Commands::Env {
                action: EnvAction::List
            })
        ));
    }

    #[test]
    fn test_cli_env_delete_subcommand() {
        let cli = Cli::try_parse_from(["kv", "env", "delete", "old-env"]).unwrap();
        match cli.command {
            Some(Commands::Env {
                action: EnvAction::Delete { ref name, force },
            }) => {
                assert_eq!(name, "old-env");
                assert!(!force);
            }
            _ => panic!("Expected Env Delete subcommand"),
        }
    }

    #[test]
    fn test_cli_env_delete_force() {
        let cli = Cli::try_parse_from(["kv", "env", "delete", "old-env", "-f"]).unwrap();
        match cli.command {
            Some(Commands::Env {
                action: EnvAction::Delete { ref name, force },
            }) => {
                assert_eq!(name, "old-env");
                assert!(force);
            }
            _ => panic!("Expected Env Delete subcommand"),
        }
    }

    #[test]
    fn test_cli_env_rename_subcommand() {
        let cli = Cli::try_parse_from(["kv", "env", "rename", "old", "new"]).unwrap();
        match cli.command {
            Some(Commands::Env {
                action:
                    EnvAction::Rename {
                        ref current,
                        ref new_name,
                    },
            }) => {
                assert_eq!(current, "old");
                assert_eq!(new_name, "new");
            }
            _ => panic!("Expected Env Rename subcommand"),
        }
    }

    #[test]
    fn test_cli_completions_subcommand() {
        let cli = Cli::try_parse_from(["kv", "completions", "zsh"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Completions { .. })));
    }

    #[test]
    fn test_cli_nvim_args_passthrough() {
        let cli = Cli::try_parse_from(["kv", "--", "file.txt", "+42"]).unwrap();
        assert_eq!(cli.nvim_args.len(), 2);
        assert_eq!(cli.nvim_args[0], "file.txt");
        assert_eq!(cli.nvim_args[1], "+42");
    }

    #[test]
    fn test_cli_nvim_args_direct() {
        let cli = Cli::try_parse_from(["kv", "file.txt"]).unwrap();
        assert_eq!(cli.nvim_args.len(), 1);
        assert_eq!(cli.nvim_args[0], "file.txt");
    }

    #[test]
    fn test_cli_nvim_args_direct_multiple() {
        let cli = Cli::try_parse_from(["kv", "file1.txt", "file2.txt"]).unwrap();
        assert_eq!(cli.nvim_args.len(), 2);
        assert_eq!(cli.nvim_args[0], "file1.txt");
        assert_eq!(cli.nvim_args[1], "file2.txt");
    }

    #[test]
    fn test_cli_nvim_args_with_kv_flags() {
        let cli = Cli::try_parse_from(["kv", "-v", "file.txt"]).unwrap();
        assert!(cli.verbose);
        assert_eq!(cli.nvim_args.len(), 1);
        assert_eq!(cli.nvim_args[0], "file.txt");
    }

    #[test]
    fn test_cli_nvim_args_with_flags() {
        let cli = Cli::try_parse_from(["kv", "-v", "--", "-u", "NONE"]).unwrap();
        assert!(cli.verbose);
        assert_eq!(cli.nvim_args.len(), 2);
    }

    #[test]
    fn test_cli_env_create_with_branch() {
        let cli = Cli::try_parse_from([
            "kv",
            "env",
            "create",
            "lazyvim",
            "--from",
            "https://github.com/LazyVim/starter",
            "--branch",
            "stable",
        ])
        .unwrap();
        match cli.command {
            Some(Commands::Env {
                action:
                    EnvAction::Create {
                        ref name,
                        ref from,
                        ref branch,
                    },
            }) => {
                assert_eq!(name, "lazyvim");
                assert_eq!(from.as_deref(), Some("https://github.com/LazyVim/starter"));
                assert_eq!(branch.as_deref(), Some("stable"));
            }
            _ => panic!("Expected Env Create subcommand"),
        }
    }

    #[test]
    fn test_cli_init_subcommand() {
        let cli = Cli::try_parse_from(["kv", "init"]).unwrap();
        assert!(matches!(cli.command, Some(Commands::Init { env: None })));
    }

    #[test]
    fn test_cli_init_with_env() {
        let cli = Cli::try_parse_from(["kv", "init", "--env", "myenv"]).unwrap();
        match cli.command {
            Some(Commands::Init { ref env }) => {
                assert_eq!(env.as_deref(), Some("myenv"));
            }
            _ => panic!("Expected Init subcommand"),
        }
    }

    #[test]
    fn test_cli_env_fork_subcommand() {
        let cli = Cli::try_parse_from(["kv", "env", "fork", "source-env", "new-env"]).unwrap();
        match cli.command {
            Some(Commands::Env {
                action:
                    EnvAction::Fork {
                        ref source,
                        ref name,
                    },
            }) => {
                assert_eq!(source, "source-env");
                assert_eq!(name, "new-env");
            }
            _ => panic!("Expected Env Fork subcommand"),
        }
    }
}
