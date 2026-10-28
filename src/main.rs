mod app;
mod backend;
mod domain;
mod ui;

use std::io::stdout;
use std::path::Path;
use std::time::Duration;

use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{ActivePane, AppState, InputMode};
use backend::runner::{run_emerge_interactively, EmergeAction};
use backend::Portage;

fn main() -> Result<(), Box<dyn std::error::Error>> 
{
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("portage-tui {}", env!("CARGO_PKG_VERSION"));
        println!("Interactive TUI for Gentoo Portage package and USE flag management\n");
        println!("USAGE:");
        println!("  portage-tui [OPTIONS] [ROOT_PATH]\n");
        println!("OPTIONS:");
        println!("  -h, --help       Print help information");
        println!("  -V, --version    Print version information\n");
        println!("KEYBINDINGS:");
        println!("  [a]              Install new package (checks mask & selects USE flags)");
        println!("  [d] / [x]        Uninstall package (emerge --deselect && emerge --depclean)");
        println!("  [u]              Update system (emaint --auto sync && emerge @world)");
        println!("  [Space]          Toggle highlighted USE flag");
        println!("  [m]              Toggle package mask in package.mask");
        println!("  [t]              Toggle ~testing keyword in package.accept_keywords");
        println!("  [s]              Save staged changes to /etc/portage");
        println!("  [/]              Search packages / USE flags");
        println!("  [Tab]            Switch between tree and USE flag panes");
        println!("  [q]              Quit");
        return Ok(());
    }

    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("portage-tui {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }

    // Explicit root argument, or real Gentoo "/", or "mock_portage" for dev/testing.
    let root = if let Some(custom) = args.get(1) {
        Path::new(custom)
    } else if Path::new("/var/db/pkg").exists() {
        Path::new("/")
    } else {
        Path::new("mock_portage")
    };

    let portage = Portage::open(root);
    let mut app = AppState::new(portage)?;

    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run the loop in a closure so the terminal is ALWAYS restored, even on error.
    let result = (|| -> Result<(), Box<dyn std::error::Error>>
    {
        while !app.should_quit
        {
            terminal.draw(|f| ui::draw(f, &mut app))?;

            if event::poll(Duration::from_millis(100))?
            {
                if let Event::Key(key) = event::read()?
                {
                    if key.kind == KeyEventKind::Press
                    {
                        handle_key(&mut app, key)?;
                    }
                }
            }
        }

        return Ok(());
    })();

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result?;

    if !app.staged.is_empty()
    {
        println!("Note: {} unsaved change(s) were discarded.", app.staged.len());
    }

    return Ok(());
}

fn handle_key(app: &mut AppState, key: KeyEvent) -> Result<(), Box<dyn std::error::Error>>
{
    if key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL)
    {
        app.should_quit = true;
        return Ok(());
    }

    // 1. Mask Prompt Modal (Asking user if they want to unmask a package)
    if app.input_mode == InputMode::MaskPrompt {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                let atom = app.install_query.clone();
                match app.apply_unmask_for_pending() {
                    Ok(msg) => {
                        app.status_message = format!("{}. Now configure USE flags for {}", msg, atom);
                        app.pending_mask_report = None;
                        app.start_install_use_flags(&atom);
                    }
                    Err(e) => {
                        app.status_message = format!("Error unmasking: {}", e);
                        app.input_mode = InputMode::Normal;
                        app.pending_mask_report = None;
                    }
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                // User chose not to unmask: DO NOT invoke emerge, return to TUI!
                app.status_message = format!("Canceled install of masked package '{}'. (emerge was NOT invoked)", app.install_query);
                app.cancel_install_input();
            }
            _ => {}
        }
        return Ok(());
    }

    // 2. Install Query Modal (Typing package name)
    if app.input_mode == InputMode::InstallInput {
        match key.code {
            KeyCode::Enter => {
                let query = app.install_query.trim().to_string();
                if !query.is_empty() {
                    // Query repo BEFORE attempting emerge
                    match app.portage.query_package(&query) {
                        backend::repo_index::ResolveResult::Exact(pkg) => {
                            let atom = format!("{}/{}", pkg.category, pkg.name);
                            app.install_query = atom.clone();

                            let report = app.portage.check_mask_status(&atom);
                            if report.is_masked() {
                                // Package is MASKED in repo / profiles / config!
                                app.pending_mask_report = Some(report);
                                app.input_mode = InputMode::MaskPrompt;
                            } else {
                                // Safe to install: proceed to USE flag selection before emerge!
                                app.start_install_use_flags(&atom);
                            }
                        }
                        backend::repo_index::ResolveResult::Ambiguous(candidates) => {
                            let preview = candidates.iter().take(4).cloned().collect::<Vec<_>>().join(", ");
                            app.status_message = format!("Ambiguous query '{}'. Did you mean: {}?", query, preview);
                        }
                        backend::repo_index::ResolveResult::NotFound => {
                            // Package does NOT exist in any repository! Do NOT invoke emerge!
                            app.status_message = format!("Package '{}' not found in any Gentoo repository. (emerge was NOT invoked)", query);
                        }
                    }
                }
            }
            KeyCode::Esc => app.cancel_install_input(),
            KeyCode::Backspace => app.install_pop(),
            KeyCode::Char(c) => app.install_push(c),
            _ => {}
        }
        return Ok(());
    }

    // 3. Install USE Flag Selection Modal (Select flags before running emerge)
    if app.input_mode == InputMode::InstallUseFlags {
        match key.code {
            KeyCode::Char(' ') => app.toggle_install_use_flag(),
            KeyCode::Up | KeyCode::Char('k') => app.move_install_flag(-1),
            KeyCode::Down | KeyCode::Char('j') => app.move_install_flag(1),
            KeyCode::PageUp => app.page_install_flag(-1),
            KeyCode::PageDown => app.page_install_flag(1),
            KeyCode::Enter => {
                let atom = match app.install_target_atom.clone() {
                    Some(a) => a,
                    None => return Ok(()),
                };

                // 1. Save any changed USE flags to /etc/portage/package.use BEFORE emerge
                if let Err(e) = app.save_install_use_flags_if_changed() {
                    app.status_message = format!("Failed to save USE flags: {}", e);
                }

                // 2. Return to normal mode and launch emerge interactively
                app.cancel_install_use_flags();
                if run_emerge_interactively(EmergeAction::InstallPackage(atom))? {
                    app.refresh_from_disk()?;
                }
            }
            KeyCode::Esc | KeyCode::Char('q') => {
                app.cancel_install_use_flags();
            }
            _ => {}
        }
        return Ok(());
    }

    // 4. Uninstall Prompt Modal (Confirm package unmerge / depclean)
    if app.input_mode == InputMode::UninstallPrompt {
        match key.code {
            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                let atom = match app.pending_uninstall_atom.take() {
                    Some(a) => a,
                    None => return Ok(()),
                };
                app.input_mode = InputMode::Normal;
                if run_emerge_interactively(EmergeAction::UninstallPackage(atom))? {
                    app.refresh_from_disk()?;
                }
            }
            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                app.cancel_uninstall();
            }
            _ => {}
        }
        return Ok(());
    }

    // 3. Search filter input
    if app.input_mode == InputMode::Search
    {
        match key.code
        {
            KeyCode::Enter => app.finish_search(),
            KeyCode::Esc => app.clear_search(),
            KeyCode::Backspace => app.search_pop(),
            KeyCode::Char(c) => app.search_push(c),
            KeyCode::Up => app.move_active(-1),
            KeyCode::Down => app.move_active(1),
            _ => {}
        }
        return Ok(());
    }

    // 4. Normal navigation & action hotkeys
    match key.code
    {
        KeyCode::Char('q') => app.should_quit = true,
        KeyCode::Tab | KeyCode::BackTab => app.switch_pane(),
        KeyCode::Char('/') => app.start_search(),
        KeyCode::Esc => app.clear_search(),
        KeyCode::Char('s') => app.save_changes(),

        // [m] -> Toggle Package Mask in package.mask
        KeyCode::Char('m') => app.toggle_mask(),

        // [t] -> Toggle ~testing keyword in package.accept_keywords
        KeyCode::Char('t') => app.toggle_testing_keyword(),

        // [u] -> emaint --auto sync + emerge update @world
        KeyCode::Char('u') => {
            if run_emerge_interactively(EmergeAction::SyncAndUpdateWorld)?
            {
                app.refresh_from_disk()?;
            }
        }

        // [a] -> Add / Install new package
        KeyCode::Char('a') => {
            app.start_install_input();
        }

        // [d] or [x] -> Uninstall / depclean package
        KeyCode::Char('d') | KeyCode::Char('x') => {
            app.start_uninstall();
        }

        KeyCode::Up | KeyCode::Char('k') => app.move_active(-1),
        KeyCode::Down | KeyCode::Char('j') => app.move_active(1),
        KeyCode::PageUp => app.page_active(-1),
        KeyCode::PageDown => app.page_active(1),
        KeyCode::Home | KeyCode::Char('g') => app.move_active(isize::MIN / 2),
        KeyCode::End | KeyCode::Char('G') => app.move_active(isize::MAX / 2),

        KeyCode::Enter if app.active_pane == ActivePane::Tree => app.toggle_expand(),
        KeyCode::Right | KeyCode::Char('l') if app.active_pane == ActivePane::Tree => app.set_expanded(true),
        KeyCode::Left | KeyCode::Char('h') if app.active_pane == ActivePane::Tree => app.collapse_or_parent(),

        // Space toggles the highlighted flag; from the tree pane it jumps to the flags.
        KeyCode::Char(' ') =>
        {
            if app.active_pane == ActivePane::UseFlags
            {
                app.toggle_use_flag();
            }
            else
            {
                app.switch_pane();
            }
        }
        _ => {}
    }

    return Ok(());
}
