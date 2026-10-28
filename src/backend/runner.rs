use std::io::{self, stdout};
use std::process::Command;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};

pub enum EmergeAction
{
    SyncAndUpdateWorld,
    InstallPackage(String),
    UninstallPackage(String),
}

impl EmergeAction
{
    pub fn to_command_steps(&self) -> Vec<(String, Vec<String>)>
    {
        let is_root = unsafe { libc::geteuid() == 0 };
        let sudo_or_direct = |cmd: &str, args: &[&str]| -> (String, Vec<String>) {
            if !is_root {
                let mut v = vec![cmd.to_string()];
                v.extend(args.iter().map(|s| s.to_string()));
                ("sudo".to_string(), v)
            } else {
                (cmd.to_string(), args.iter().map(|s| s.to_string()).collect())
            }
        };

        match self {
            EmergeAction::SyncAndUpdateWorld => {
                // User requirement: "I want the 'update' doing 'sudo emaint --auto sync' first"
                vec![
                    sudo_or_direct("emaint", &["--auto", "sync"]),
                    sudo_or_direct("emerge", &["--ask", "--verbose", "--update", "--deep", "--newuse", "@world"]),
                ]
            }
            EmergeAction::InstallPackage(atom) => {
                vec![
                    sudo_or_direct("emerge", &["--ask", "--verbose", atom]),
                ]
            }
            EmergeAction::UninstallPackage(atom) => {
                vec![
                    // Deselect from @world first (if present), then invoke interactive depclean
                    sudo_or_direct("emerge", &["--deselect", atom]),
                    sudo_or_direct("emerge", &["--ask", "--verbose", "--depclean", atom]),
                ]
            }
        }
    }
}

// Executes emerge or emaint cleanly in a suspended terminal session.
pub fn run_emerge_interactively(action: EmergeAction) -> Result<bool, io::Error>
{
    let steps = action.to_command_steps();

    // 1. Leave alternate screen & disable raw mode
    disable_raw_mode()?;
    execute!(stdout(), LeaveAlternateScreen)?;

    println!("\x1b[1;36m============================================================\x1b[0m");
    println!("\x1b[1;36m               Portage Manager - Process Runner             \x1b[0m");
    println!("\x1b[1;36m============================================================\x1b[0m");

    let mut overall_success = true;

    for (program, args) in steps
    {
        println!("\x1b[1;32m[*] Running:\x1b[0m {} {}\n", program, args.join(" "));

        let status = match Command::new(&program).args(&args).status()
        {
            Ok(s) => s,
            Err(e) => {
                println!("\n\x1b[1;31m[-] Failed to execute {}: {}\x1b[0m", program, e);
                if e.kind() == io::ErrorKind::NotFound
                {
                    println!("\x1b[1;33m[!] Command '{}' not found in PATH (are you in Debian testing?).\x1b[0m", program);
                }
                overall_success = false;
                break;
            }
        };

        if !status.success()
        {
            println!("\n\x1b[1;31m[-] Step failed with exit code: {}\x1b[0m", status);
            overall_success = false;
            break;
        }
        println!("\n\x1b[1;32m[✓] Step completed successfully.\x1b[0m\n");
    }

    println!("\x1b[1;36m============================================================\x1b[0m");
    println!("Press [Enter] to return to Portage Manager...");
    let mut dummy = String::new();
    let _ = io::stdin().read_line(&mut dummy);

    // Restore raw mode and re-enter alternate screen for Ratatui
    execute!(stdout(), EnterAlternateScreen)?;
    enable_raw_mode()?;

    return Ok(overall_success);
}

#[cfg(test)]
mod tests
{
    use super::*;

    #[test]
    fn test_uninstall_command_steps()
    {
        let action = EmergeAction::UninstallPackage("games-emulation/fceux".to_string());
        let steps = action.to_command_steps();
        assert_eq!(steps.len(), 2);
        // Step 1: deselect
        assert!(steps[0].1.contains(&"--deselect".to_string()));
        assert!(steps[0].1.contains(&"games-emulation/fceux".to_string()));
        // Step 2: depclean
        assert!(steps[1].1.contains(&"--depclean".to_string()));
        assert!(steps[1].1.contains(&"games-emulation/fceux".to_string()));
    }

    #[test]
    fn test_install_command_steps()
    {
        let action = EmergeAction::InstallPackage("app-editors/neovim".to_string());
        let steps = action.to_command_steps();
        assert_eq!(steps.len(), 1);
        assert!(steps[0].1.contains(&"app-editors/neovim".to_string()));
    }
}
