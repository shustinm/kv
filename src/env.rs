use crate::paths::*;
use inquire::{Confirm, Select, Text};
use owo_colors::OwoColorize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

pub fn validate_env_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("Env name cannot be empty".to_string());
    }
    if name == "." || name == ".." {
        return Err("Env name cannot be '.' or '..'".to_string());
    }
    if !name
        .chars()
        .all(|c| c.is_alphanumeric() || c == '-' || c == '_')
    {
        return Err(
            "Env name must only contain alphanumeric characters, hyphens, and underscores"
                .to_string(),
        );
    }
    Ok(())
}

fn is_git_url(s: &str) -> bool {
    s.contains("://") || s.starts_with("git@")
}

pub fn cmd_env_create(
    name: &str,
    from: Option<&str>,
    branch: Option<&str>,
) -> Result<PathBuf, String> {
    validate_env_name(name).map_err(|e| format!("Invalid env name: {}", e))?;

    let config_dir = env_config_dir(name);
    if config_dir.exists() {
        return Err(format!(
            "Env '{}' already exists at: {}",
            name,
            config_dir.display()
        ));
    }

    if branch.is_some() && !from.is_some_and(is_git_url) {
        return Err("--branch can only be used with a git URL source".to_string());
    }

    if let Some(source) = from {
        if is_git_url(source) {
            eprintln!("{} {}...", "Cloning".green(), source.dimmed());
            let mut cmd = Command::new("git");
            cmd.arg("clone");
            if let Some(b) = branch {
                cmd.arg("--branch").arg(b);
            }
            cmd.arg(source).arg(&config_dir);
            let status = cmd
                .status()
                .map_err(|e| format!("Failed to run git clone: {}", e))?;
            if !status.success() {
                return Err("git clone failed".to_string());
            }
            // Remove .git/ so the env starts as a clean config
            let git_dir = config_dir.join(".git");
            if git_dir.exists() {
                fs::remove_dir_all(&git_dir)
                    .map_err(|e| format!("Failed to remove .git directory: {}", e))?;
            }
        } else {
            // Try as env name first, then as path
            let source_dir = env_config_dir(source);
            let source_path = if source_dir.exists() {
                source_dir
            } else {
                let p = PathBuf::from(source);
                if !p.exists() {
                    return Err(format!("Source '{}' not found as env name or path", source));
                }
                p
            };
            copy_dir_recursive(&source_path, &config_dir)
                .map_err(|e| format!("Failed to copy from source: {}", e))?;
        }
    } else {
        fs::create_dir_all(&config_dir)
            .map_err(|e| format!("Failed to create env config dir: {}", e))?;
    }

    println!(
        "{} env '{}'. Populate config at: {}",
        "Created".green(),
        name.cyan().bold(),
        config_dir.display().to_string().dimmed()
    );
    Ok(config_dir)
}

pub fn cmd_env_init(name: &str) -> Result<PathBuf, String> {
    validate_env_name(name).map_err(|e| format!("Invalid env name: {}", e))?;

    let config_dir = env_config_dir(name);
    if config_dir.exists() {
        return Err(format!(
            "Env '{}' already exists at: {}",
            name,
            config_dir.display()
        ));
    }

    let items = vec![
        "Clean (empty config)",
        "Copy from existing config directory",
        "Clone from a git template",
    ];
    let selection = Select::new(&format!("How do you want to set up '{}'?", name), items)
        .prompt()
        .map_err(|e| format!("Prompt failed: {}", e))?;

    match selection {
        "Clean (empty config)" => cmd_env_create(name, None, None),
        "Copy from existing config directory" => {
            let home = std::env::var("HOME").unwrap_or_else(|_| "/tmp".to_string());
            let default_path = format!("{}/.config/nvim", home);
            let path = Text::new("Path to config directory:")
                .with_default(&default_path)
                .prompt()
                .map_err(|e| format!("Prompt failed: {}", e))?;
            cmd_env_create(name, Some(&path), None)
        }
        "Clone from a git template" => {
            let url = Text::new("Git URL:")
                .with_default("https://github.com/LazyVim/starter")
                .prompt()
                .map_err(|e| format!("Prompt failed: {}", e))?;
            let branch = Text::new("Branch (leave empty for default):")
                .prompt()
                .map_err(|e| format!("Prompt failed: {}", e))?;
            let branch = if branch.is_empty() {
                None
            } else {
                Some(branch)
            };
            cmd_env_create(name, Some(&url), branch.as_deref())
        }
        _ => unreachable!(),
    }
}

pub fn cmd_env_fork(source: &str, name: &str) -> Result<PathBuf, String> {
    validate_env_name(source).map_err(|e| format!("Invalid source env name: {}", e))?;
    validate_env_name(name).map_err(|e| format!("Invalid env name: {}", e))?;

    let source_config = env_config_dir(source);
    if !source_config.exists() {
        return Err(format!("Source env '{}' does not exist.", source));
    }

    let dest_config = env_config_dir(name);
    if dest_config.exists() {
        return Err(format!(
            "Env '{}' already exists at: {}",
            name,
            dest_config.display()
        ));
    }

    for (label, src, dst) in &env_all_dir_pairs(source, name) {
        if src.exists() {
            copy_dir_recursive(src, dst)
                .map_err(|e| format!("Failed to copy {} dir: {}", label, e))?;
            println!(
                "  Copied {}: {}",
                label.bold(),
                dst.display().to_string().dimmed()
            );
        }
    }

    println!(
        "{} env '{}' from '{}'.",
        "Forked".green(),
        name.cyan().bold(),
        source.cyan().bold()
    );
    Ok(dest_config)
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> std::io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let ty = entry.file_type()?;
        let dst_path = dst.join(entry.file_name());
        if ty.is_symlink() {
            let target = fs::read_link(entry.path())?;
            #[cfg(unix)]
            std::os::unix::fs::symlink(&target, &dst_path)?;
            #[cfg(windows)]
            {
                if target.is_dir() {
                    std::os::windows::fs::symlink_dir(&target, &dst_path)?;
                } else {
                    std::os::windows::fs::symlink_file(&target, &dst_path)?;
                }
            }
        } else if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &dst_path)?;
        } else {
            fs::copy(entry.path(), dst_path)?;
        }
    }
    Ok(())
}

fn dir_size(path: &Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    let mut total = 0u64;
    if let Ok(entries) = fs::read_dir(path) {
        for entry in entries.filter_map(|e| e.ok()) {
            let ft = match entry.file_type() {
                Ok(ft) => ft,
                Err(_) => continue,
            };
            if ft.is_dir() {
                total += dir_size(&entry.path());
            } else {
                total += entry.metadata().map(|m| m.len()).unwrap_or(0);
            }
        }
    }
    total
}

pub fn format_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;
    if bytes >= GB {
        format!("{:.1}G", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}M", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}K", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

pub struct EnvInfo {
    pub name: String,
    pub dirs: Vec<(String, PathBuf, u64)>,
    pub total_size: u64,
}

pub fn cmd_env_list() -> Vec<EnvInfo> {
    let envs_dir = envs_config_root();
    if !envs_dir.exists() {
        return Vec::new();
    }

    let mut entries: Vec<_> = match fs::read_dir(&envs_dir) {
        Ok(rd) => rd
            .filter_map(|e| e.ok())
            .filter(|e| e.path().is_dir())
            .collect(),
        Err(_) => return Vec::new(),
    };

    entries.sort_by_key(|e| e.file_name());

    entries
        .into_iter()
        .map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let dirs: Vec<_> = env_all_dirs(&name)
                .into_iter()
                .filter(|(_, dir)| dir.exists())
                .map(|(label, dir)| {
                    let size = dir_size(&dir);
                    (label.to_string(), dir, size)
                })
                .collect();
            let total_size = dirs.iter().map(|(_, _, s)| s).sum();
            EnvInfo {
                name,
                dirs,
                total_size,
            }
        })
        .collect()
}

pub fn cmd_env_delete(name: &str, force: bool) -> Result<(), String> {
    validate_env_name(name).map_err(|e| format!("Invalid env name: {}", e))?;

    let config_dir = env_config_dir(name);
    if !config_dir.exists() {
        return Err(format!("Env '{}' does not exist.", name));
    }

    let dirs = env_all_dirs(name);

    if !force {
        println!("The following directories will be deleted:");
        for (label, dir) in &dirs {
            if dir.exists() {
                let size = dir_size(dir);
                println!(
                    "  {}: {} ({})",
                    label.bold(),
                    dir.display().to_string().dimmed(),
                    format_size(size)
                );
            }
        }
        let confirmed = Confirm::new(&format!("Delete env '{}'?", name))
            .with_default(false)
            .prompt()
            .map_err(|e| format!("Prompt failed: {}", e))?;
        if !confirmed {
            println!("Aborted.");
            return Ok(());
        }
    }

    for (label, dir) in &dirs {
        if dir.exists() {
            fs::remove_dir_all(dir).unwrap_or_else(|e| {
                eprintln!("Failed to remove {} dir: {}", label, e);
            });
        }
    }

    println!("{} env '{}'.", "Deleted".green(), name.cyan().bold());
    Ok(())
}

pub fn cmd_env_rename(current: &str, new_name: &str) -> Result<(), String> {
    validate_env_name(current).map_err(|e| format!("Invalid current env name: {}", e))?;
    validate_env_name(new_name).map_err(|e| format!("Invalid new env name: {}", e))?;

    let src_config = env_config_dir(current);
    if !src_config.exists() {
        return Err(format!("Env '{}' does not exist.", current));
    }

    let dst_config = env_config_dir(new_name);
    if dst_config.exists() {
        return Err(format!("Env '{}' already exists.", new_name));
    }

    for (label, src, dst) in &env_all_dir_pairs(current, new_name) {
        if src.exists() {
            if let Some(parent) = dst.parent() {
                fs::create_dir_all(parent)
                    .map_err(|e| format!("Failed to create parent for {} dir: {}", label, e))?;
            }
            fs::rename(src, dst).map_err(|e| format!("Failed to rename {} dir: {}", label, e))?;
        }
    }

    println!(
        "{} env '{}' to '{}'.",
        "Renamed".green(),
        current.cyan().bold(),
        new_name.cyan().bold()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::env;
    use tempfile::TempDir;

    fn with_temp_xdg() -> TempDir {
        let tmp = TempDir::new().unwrap();
        let base = tmp.path();
        env::set_var("XDG_CONFIG_HOME", base.join("config"));
        env::set_var("XDG_DATA_HOME", base.join("data"));
        env::set_var("XDG_STATE_HOME", base.join("state"));
        env::set_var("XDG_CACHE_HOME", base.join("cache"));
        tmp
    }

    // --- validate_env_name ---

    #[test]
    fn test_validate_env_name_valid() {
        assert!(validate_env_name("my-env").is_ok());
        assert!(validate_env_name("test_env").is_ok());
        assert!(validate_env_name("env123").is_ok());
        assert!(validate_env_name("a").is_ok());
    }

    #[test]
    fn test_validate_env_name_empty() {
        assert!(validate_env_name("").is_err());
    }

    #[test]
    fn test_validate_env_name_dot() {
        assert!(validate_env_name(".").is_err());
        assert!(validate_env_name("..").is_err());
    }

    #[test]
    fn test_validate_env_name_invalid_chars() {
        assert!(validate_env_name("my/env").is_err());
        assert!(validate_env_name("my env").is_err());
        assert!(validate_env_name("my.env").is_err());
        assert!(validate_env_name("env@1").is_err());
    }

    // --- is_git_url ---

    #[test]
    fn test_is_git_url() {
        assert!(is_git_url("https://github.com/LazyVim/starter"));
        assert!(is_git_url("http://example.com/repo.git"));
        assert!(is_git_url("git://example.com/repo.git"));
        assert!(is_git_url("ssh://git@example.com/repo.git"));
        assert!(is_git_url("git@github.com:user/repo.git"));

        assert!(!is_git_url("my-env"));
        assert!(!is_git_url("/some/local/path"));
        assert!(!is_git_url("relative/path"));
    }

    // --- copy_dir_recursive ---

    #[test]
    fn test_copy_dir_recursive() {
        let tmp = TempDir::new().unwrap();
        let src = tmp.path().join("src");
        let dst = tmp.path().join("dst");

        fs::create_dir_all(src.join("sub")).unwrap();
        fs::write(src.join("a.txt"), "hello").unwrap();
        fs::write(src.join("sub/b.txt"), "world").unwrap();

        copy_dir_recursive(&src, &dst).unwrap();

        assert_eq!(fs::read_to_string(dst.join("a.txt")).unwrap(), "hello");
        assert_eq!(fs::read_to_string(dst.join("sub/b.txt")).unwrap(), "world");
    }

    // --- format_size ---

    #[test]
    fn test_format_size() {
        assert_eq!(format_size(0), "0B");
        assert_eq!(format_size(512), "512B");
        assert_eq!(format_size(1024), "1.0K");
        assert_eq!(format_size(1536), "1.5K");
        assert_eq!(format_size(1048576), "1.0M");
        assert_eq!(format_size(1073741824), "1.0G");
    }

    // --- dir_size ---

    #[test]
    fn test_dir_size() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join("sized");
        fs::create_dir_all(dir.join("sub")).unwrap();
        fs::write(dir.join("a.txt"), "hello").unwrap(); // 5 bytes
        fs::write(dir.join("sub/b.txt"), "world!").unwrap(); // 6 bytes

        assert_eq!(dir_size(&dir), 11);
        assert_eq!(dir_size(&tmp.path().join("nonexistent")), 0);
    }

    // --- cmd_env_create / cmd_env_delete integration ---

    #[test]
    #[serial]
    fn test_env_create_and_delete() {
        let _tmp = with_temp_xdg();

        let result = cmd_env_create("test-env", None, None);
        assert!(result.is_ok());
        let config_dir = result.unwrap();
        assert!(config_dir.exists());

        let dup = cmd_env_create("test-env", None, None);
        assert!(dup.is_err());
        assert!(dup.unwrap_err().contains("already exists"));

        let del = cmd_env_delete("test-env", true);
        assert!(del.is_ok());
        assert!(!config_dir.exists());

        let del2 = cmd_env_delete("test-env", true);
        assert!(del2.is_err());
        assert!(del2.unwrap_err().contains("does not exist"));
    }

    #[test]
    #[serial]
    fn test_env_create_with_invalid_name() {
        let _tmp = with_temp_xdg();

        assert!(cmd_env_create("bad/name", None, None).is_err());
        assert!(cmd_env_create("", None, None).is_err());
        assert!(cmd_env_create("..", None, None).is_err());
    }

    #[test]
    #[serial]
    fn test_env_create_from_path() {
        let _tmp = with_temp_xdg();
        let source = _tmp.path().join("my-source-cfg");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("init.lua"), "-- test config").unwrap();

        let result = cmd_env_create("from-path", Some(source.to_str().unwrap()), None);
        assert!(result.is_ok());

        let created = result.unwrap();
        assert_eq!(
            fs::read_to_string(created.join("init.lua")).unwrap(),
            "-- test config"
        );
    }

    #[test]
    #[serial]
    fn test_env_create_from_existing_env() {
        let _tmp = with_temp_xdg();

        cmd_env_create("source-env", None, None).unwrap();
        let source_config = env_config_dir("source-env");
        fs::write(source_config.join("init.lua"), "-- source").unwrap();

        let result = cmd_env_create("cloned-env", Some("source-env"), None);
        assert!(result.is_ok());
        let cloned = result.unwrap();
        assert_eq!(
            fs::read_to_string(cloned.join("init.lua")).unwrap(),
            "-- source"
        );
    }

    #[test]
    #[serial]
    fn test_env_delete_cleans_all_xdg_dirs() {
        let _tmp = with_temp_xdg();

        cmd_env_create("full-env", None, None).unwrap();

        fs::create_dir_all(env_data_dir("full-env")).unwrap();
        fs::create_dir_all(env_state_dir("full-env")).unwrap();
        fs::create_dir_all(env_cache_dir("full-env")).unwrap();

        assert!(env_config_dir("full-env").exists());
        assert!(env_data_dir("full-env").exists());
        assert!(env_state_dir("full-env").exists());
        assert!(env_cache_dir("full-env").exists());

        cmd_env_delete("full-env", true).unwrap();

        assert!(!env_config_dir("full-env").exists());
        assert!(!env_data_dir("full-env").exists());
        assert!(!env_state_dir("full-env").exists());
        assert!(!env_cache_dir("full-env").exists());
    }

    #[test]
    #[serial]
    fn test_env_create_rejects_branch_without_git_url() {
        let _tmp = with_temp_xdg();

        let source = _tmp.path().join("my-source");
        fs::create_dir_all(&source).unwrap();
        let result = cmd_env_create("test-branch", Some(source.to_str().unwrap()), Some("main"));
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .contains("--branch can only be used with a git URL"));

        let result2 = cmd_env_create("test-branch2", None, Some("main"));
        assert!(result2.is_err());
        assert!(result2
            .unwrap_err()
            .contains("--branch can only be used with a git URL"));
    }

    // --- env fork ---

    #[test]
    #[serial]
    fn test_env_fork_copies_all_dirs() {
        let _tmp = with_temp_xdg();

        cmd_env_create("fork-src", None, None).unwrap();
        fs::write(env_config_dir("fork-src").join("init.lua"), "-- config").unwrap();
        fs::create_dir_all(env_data_dir("fork-src")).unwrap();
        fs::write(env_data_dir("fork-src").join("data.txt"), "data").unwrap();
        fs::create_dir_all(env_state_dir("fork-src")).unwrap();
        fs::write(env_state_dir("fork-src").join("state.txt"), "state").unwrap();
        fs::create_dir_all(env_cache_dir("fork-src")).unwrap();
        fs::write(env_cache_dir("fork-src").join("cache.txt"), "cache").unwrap();

        let result = cmd_env_fork("fork-src", "fork-dst");
        assert!(result.is_ok());

        assert_eq!(
            fs::read_to_string(env_config_dir("fork-dst").join("init.lua")).unwrap(),
            "-- config"
        );
        assert_eq!(
            fs::read_to_string(env_data_dir("fork-dst").join("data.txt")).unwrap(),
            "data"
        );
        assert_eq!(
            fs::read_to_string(env_state_dir("fork-dst").join("state.txt")).unwrap(),
            "state"
        );
        assert_eq!(
            fs::read_to_string(env_cache_dir("fork-dst").join("cache.txt")).unwrap(),
            "cache"
        );
    }

    #[test]
    #[serial]
    fn test_env_fork_nonexistent_source() {
        let _tmp = with_temp_xdg();

        let result = cmd_env_fork("nonexistent", "new-env");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("does not exist"));
    }

    #[test]
    #[serial]
    fn test_env_fork_duplicate_dest() {
        let _tmp = with_temp_xdg();

        cmd_env_create("fork-a", None, None).unwrap();
        cmd_env_create("fork-b", None, None).unwrap();

        let result = cmd_env_fork("fork-a", "fork-b");
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("already exists"));
    }
}
