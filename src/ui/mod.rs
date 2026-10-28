use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Clear, List, ListItem, ListState, Paragraph, Wrap},
    Frame,
};

use crate::app::{ActivePane, AppState, InputMode};
use crate::domain::package::FlagOrigin;

pub fn draw(frame: &mut Frame, app: &mut AppState)
{
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Tree + details
            Constraint::Length(4), // Status + controls
        ])
        .split(frame.area());

    let main = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(45), Constraint::Percentage(55)])
        .split(chunks[1]);

    // Page sizes follow the real terminal size (used by PageUp/PageDown).
    app.tree_page = main[0].height.saturating_sub(2) as usize;

    // ListState is moved out while rendering so we can read `app` freely
    // and still hand ratatui a mutable scroll state; put back afterwards.
    let mut tree_state = std::mem::take(&mut app.tree_state);
    let mut flag_state = std::mem::take(&mut app.flag_state);

    draw_header(frame, app, chunks[0]);
    draw_tree(frame, app, main[0], &mut tree_state);
    let flag_page = draw_details(frame, app, main[1], &mut flag_state);
    draw_footer(frame, app, chunks[2]);

    if app.input_mode == InputMode::InstallInput {
        draw_install_modal(frame, app);
    } else if app.input_mode == InputMode::MaskPrompt {
        draw_mask_prompt_modal(frame, app);
    } else if app.input_mode == InputMode::InstallUseFlags {
        let mut install_state = std::mem::take(&mut app.install_flag_state);
        draw_install_use_flags_modal(frame, app, &mut install_state);
        app.install_flag_state = install_state;
    } else if app.input_mode == InputMode::UninstallPrompt {
        draw_uninstall_modal(frame, app);
    }

    app.tree_state = tree_state;
    app.flag_state = flag_state;
    app.flag_page = flag_page;
}

fn pane_border(app: &AppState, pane: ActivePane) -> Style
{
    if app.active_pane == pane
    {
        return Style::default().fg(Color::Green);
    }

    return Style::default().fg(Color::DarkGray);
}

fn highlight(app: &AppState, pane: ActivePane) -> Style
{
    if app.active_pane == pane
    {
        return Style::default().bg(Color::Green).fg(Color::Black).add_modifier(Modifier::BOLD);
    }

    return Style::default().bg(Color::DarkGray).fg(Color::White);
}

fn draw_header(frame: &mut Frame, app: &AppState, area: Rect)
{
    let repo = match app.portage.main_repo()
    {
        Some(p) => Span::styled(p.display().to_string(), Style::default().fg(Color::Cyan)),
        None => Span::styled("no repo found (no descriptions)", Style::default().fg(Color::Red)),
    };

    let line = Line::from(vec![
        Span::styled(" Portage Manager ", Style::default().fg(Color::Black).bg(Color::Cyan).add_modifier(Modifier::BOLD)),
        Span::raw("  root: "),
        Span::styled(app.portage.root().display().to_string(), Style::default().fg(Color::Magenta)),
        Span::raw("  repo: "),
        repo,
    ]);

    let widget = Paragraph::new(line).block(Block::default().borders(Borders::ALL));
    frame.render_widget(widget, area);
}

fn draw_tree(frame: &mut Frame, app: &AppState, area: Rect, state: &mut ListState)
{
    let mut items: Vec<ListItem> = Vec::new();

    for item in &app.flat
    {
        let pkg = app.packages.get(&item.atom);
        let is_world = pkg.map_or(false, |p| p.is_world);
        let is_masked = app.current_mask_state(&item.atom);

        let glyph = if item.is_cycle
        {
            "↻ "
        }
        else if item.has_children
        {
            if item.is_expanded { "▼ " } else { "▶ " }
        }
        else
        {
            "• "
        };

        let name_style = if is_masked
        {
            Style::default().fg(Color::Red).add_modifier(Modifier::CROSSED_OUT)
        }
        else if is_world
        {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        }
        else
        {
            Style::default().fg(Color::Gray)
        };

        let mut spans = vec![
            Span::raw("  ".repeat(item.depth)),
            Span::styled(glyph, Style::default().fg(Color::DarkGray)),
            Span::styled(item.atom.clone(), name_style),
        ];

        if is_masked
        {
            spans.push(Span::styled(" [MASKED]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)));
        }
        if item.depth > 0 && is_world
        {
            spans.push(Span::styled(" [world]", Style::default().fg(Color::Green)));
        }
        if item.is_cycle
        {
            spans.push(Span::styled(" (circular)", Style::default().fg(Color::Red)));
        }
        if let Some(p) = pkg
        {
            if !p.description.is_empty()
            {
                spans.push(Span::styled(format!("  {}", p.description), Style::default().fg(Color::DarkGray)));
            }
        }

        items.push(ListItem::new(Line::from(spans)));
    }

    let mut title = format!(" @world: {} ", app.roots.len());
    if !app.tree_filter.is_empty()
    {
        title = format!(" @world: {} of {}  filter: \"{}\" ", count_roots(app), app.roots.len(), app.tree_filter);
    }

    let list = List::new(items)
        .highlight_style(highlight(app, ActivePane::Tree))
        .block(Block::default().borders(Borders::ALL).border_style(pane_border(app, ActivePane::Tree)).title(title));

    state.select(if app.flat.is_empty() { None } else { Some(app.selected_tree_row) });
    frame.render_stateful_widget(list, area, state);
}

fn count_roots(app: &AppState) -> usize
{
    return app.flat.iter().filter(|i| i.depth == 0).count();
}

// Returns the flag page size (in items) for PageUp/PageDown.
fn draw_details(frame: &mut Frame, app: &AppState, area: Rect, state: &mut ListState) -> usize
{
    let pkg = match app.selected_package()
    {
        Some(p) => p,
        None =>
        {
            let empty = Paragraph::new("No package selected").block(Block::default().borders(Borders::ALL));
            frame.render_widget(empty, area);
            return 1;
        }
    };

    let atom = pkg.atom();
    let is_masked = app.current_mask_state(&atom);
    let accepts_testing = app.current_testing_state(&atom);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(7), // package info + mask status
            Constraint::Min(6),    // flag list
            Constraint::Length(6), // selected flag description
        ])
        .split(area);

    // --- 1. Package info + Mask/Keyword controls ---------------------------
    let label = Style::default().fg(Color::DarkGray);
    let status = if pkg.is_world
    {
        Span::styled("in @world (you installed it)", Style::default().fg(Color::Green))
    }
    else
    {
        Span::styled("dependency", Style::default().fg(Color::Blue))
    };

    let mask_badge = if is_masked
    {
        Span::styled("[MASKED] in package.mask", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD))
    }
    else
    {
        Span::styled("Not masked", Style::default().fg(Color::Green))
    };

    let keyword_badge = if accepts_testing
    {
        Span::styled("~testing (package.accept_keywords)", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
    }
    else
    {
        Span::styled("stable", Style::default().fg(Color::Cyan))
    };

    let info = vec![
        Line::from(vec![
            Span::styled(pkg.cpv(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  ::{}", pkg.repository), label),
            Span::styled("  slot ", label),
            Span::styled(pkg.slot.clone(), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(vec![
            Span::styled("Status:   ", label),
            status,
            Span::raw("   "),
            Span::styled("Mask [m]: ", label),
            mask_badge,
            Span::raw("   "),
            Span::styled("Keywords [t]: ", label),
            keyword_badge,
        ]),
        Line::from(vec![Span::styled("Homepage: ", label), Span::raw(pkg.homepage.clone())]),
        Line::from(vec![Span::styled(pkg.description.clone(), Style::default().fg(Color::White))]),
    ];

    let info_widget = Paragraph::new(info)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title(" Package "));
    frame.render_widget(info_widget, chunks[0]);

    // --- 2. USE flag list (2 lines per flag) -------------------------------
    let visible = app.visible_flag_indices();
    let mut items: Vec<ListItem> = Vec::new();

    if pkg.use_flags.is_empty()
    {
        items.push(ListItem::new("  (this package has no USE flags)"));
    }

    for idx in &visible
    {
        let flag = &pkg.use_flags[*idx];
        let staged = app.staged_value(&atom, &flag.name);
        let enabled = staged.unwrap_or(flag.enabled);

        let check = if enabled
        {
            Span::styled("[✓] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
        }
        else
        {
            Span::styled("[ ] ", Style::default().fg(Color::DarkGray))
        };

        let (origin_text, origin_color) = if staged.is_some()
        {
            ("staged", Color::Yellow)
        }
        else
        {
            match flag.origin
            {
                FlagOrigin::PackageUse => ("package.use", Color::Cyan),
                FlagOrigin::MakeConf => ("make.conf", Color::Magenta),
                FlagOrigin::ProfileDefault => ("default", Color::DarkGray),
            }
        };

        let name_style = if enabled
        {
            Style::default().fg(Color::White).add_modifier(Modifier::BOLD)
        }
        else
        {
            Style::default().fg(Color::Gray)
        };

        let first = Line::from(vec![
            check,
            Span::styled(flag.name.clone(), name_style),
            Span::styled(format!("  ({})", origin_text), Style::default().fg(origin_color)),
        ]);
        let second = Line::from(Span::styled(format!("    {}", flag.description), Style::default().fg(Color::DarkGray)));

        items.push(ListItem::new(Text::from(vec![first, second])));
    }

    let mut title = format!(" USE flags: {} ", pkg.use_flags.len());
    if !app.flag_filter.is_empty()
    {
        title = format!(" USE flags: {} of {}  filter: \"{}\" ", visible.len(), pkg.use_flags.len(), app.flag_filter);
    }

    let list = List::new(items)
        .highlight_style(highlight(app, ActivePane::UseFlags))
        .block(Block::default().borders(Borders::ALL).border_style(pane_border(app, ActivePane::UseFlags)).title(title));

    state.select(if visible.is_empty() { None } else { Some(app.selected_flag_row) });
    frame.render_stateful_widget(list, chunks[1], state);

    // --- 3. Full description of the highlighted flag -----------------------
    let detail = match app.selected_flag()
    {
        Some(flag) =>
        {
            let on_off = |b: bool| if b { "on" } else { "off" };
            vec![
                Line::from(Span::styled(flag.name.clone(), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))),
                Line::from(flag.description.clone()),
                Line::from(Span::styled(
                    format!("installed with: {}   IUSE default: {}", on_off(flag.enabled), on_off(flag.default_on)),
                    Style::default().fg(Color::DarkGray),
                )),
            ]
        }
        None => vec![Line::from("")],
    };

    let detail_widget = Paragraph::new(detail)
        .wrap(Wrap { trim: true })
        .block(Block::default().borders(Borders::ALL).title(" Flag "));
    frame.render_widget(detail_widget, chunks[2]);

    let flag_page = (chunks[1].height.saturating_sub(2) / 2) as usize;
    return flag_page.max(1);
}

fn key(k: &'static str) -> Span<'static>
{
    return Span::styled(k, Style::default().fg(Color::Cyan));
}

fn draw_footer(frame: &mut Frame, app: &AppState, area: Rect)
{
    let badge = if app.staged.is_empty()
    {
        Span::styled(" no unsaved changes ", Style::default().fg(Color::Black).bg(Color::Green))
    }
    else
    {
        Span::styled(
            format!(" {} unsaved ", app.staged.len()),
            Style::default().fg(Color::Black).bg(Color::Yellow).add_modifier(Modifier::BOLD),
        )
    };

    let status = Line::from(vec![badge, Span::raw(" "), Span::styled(app.status_message.clone(), Style::default().fg(Color::Yellow))]);

    let controls = if app.input_mode == InputMode::Search
    {
        let what = if app.active_pane == ActivePane::Tree { "packages" } else { "USE flags" };
        Line::from(vec![
            Span::styled(format!("Search {}: /", what), Style::default().fg(Color::Yellow)),
            Span::styled(format!("{}█", app.active_filter()), Style::default().fg(Color::White).add_modifier(Modifier::BOLD)),
            Span::raw("   "),
            key("[Enter]"),
            Span::raw(" keep  "),
            key("[Esc]"),
            Span::raw(" clear"),
        ])
    }
    else
    {
        Line::from(vec![
            key("[Tab]"), Span::raw(" pane  "),
            key("[↑↓/jk]"), Span::raw(" move  "),
            key("[Space]"), Span::raw(" flag  "),
            key("[m]"), Span::raw(" mask  "),
            key("[t]"), Span::raw(" ~testing  "),
            key("[s]"), Span::raw(" save  "),
            key("[a]"), Span::raw(" add  "),
            key("[d]"), Span::raw(" uninstall  "),
            key("[u]"), Span::raw(" sync & update  "),
            Span::styled("[q]", Style::default().fg(Color::Red)), Span::raw(" quit"),
        ])
    };

    let widget = Paragraph::new(vec![status, controls]).block(Block::default().borders(Borders::ALL));
    frame.render_widget(widget, area);
}

fn draw_install_modal(frame: &mut Frame, app: &AppState)
{
    let area = centered_rect(60, 20, frame.area());
    frame.render_widget(Clear, area);

    let text = vec![
        Line::from(Span::styled("Enter package atom to install via emerge:", Style::default().fg(Color::White))),
        Line::from(""),
        Line::from(vec![
            Span::styled("> ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::styled(format!("{}█", app.install_query), Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "Checks mask & testing status first. If masked, prompts you before emerge.",
            Style::default().fg(Color::DarkGray),
        )),
        Line::from(vec![
            Span::styled("[Enter]", Style::default().fg(Color::Cyan)),
            Span::raw(" search & install   "),
            Span::styled("[Esc]", Style::default().fg(Color::Red)),
            Span::raw(" cancel"),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" 📦 Install Package ");

    let widget = Paragraph::new(text).block(block).wrap(Wrap { trim: true });
    frame.render_widget(widget, area);
}

fn draw_mask_prompt_modal(frame: &mut Frame, app: &AppState)
{
    let area = centered_rect(65, 30, frame.area());
    frame.render_widget(Clear, area);

    let mut lines = vec![
        Line::from(Span::styled(
            format!("Package '{}' is currently MASKED in Gentoo:", app.install_query),
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    if let Some(report) = &app.pending_mask_report
    {
        for reason in report.summary()
        {
            lines.push(Line::from(vec![
                Span::styled("  • ", Style::default().fg(Color::Yellow)),
                Span::styled(reason, Style::default().fg(Color::White)),
            ]));
        }
    }

    lines.push(Line::from(""));
    lines.push(Line::from(Span::styled(
        "Do you want to unmask this package and proceed with emerge?",
        Style::default().fg(Color::Cyan),
    )));
    lines.push(Line::from(""));
    lines.push(Line::from(vec![
        Span::styled("[y / Enter]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
        Span::raw(" Unmask and launch emerge"),
    ]));
    lines.push(Line::from(vec![
        Span::styled("[n / Esc]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
        Span::raw(" Cancel and return to TUI (emerge will NOT be invoked)"),
    ]));

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow))
        .title(" ⚠️ Package Mask Warning ");

    let widget = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });
    frame.render_widget(widget, area);
}

fn draw_install_use_flags_modal(frame: &mut Frame, app: &AppState, state: &mut ListState)
{
    let area = centered_rect(80, 75, frame.area());
    frame.render_widget(Clear, area);

    let atom = app.install_target_atom.as_deref().unwrap_or("package");

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header title & package name
            Constraint::Min(6),    // Flags list
            Constraint::Length(3), // Controls footer
        ])
        .split(area);

    // 1. Header
    let header_text = vec![
        Line::from(vec![
            Span::styled("Configure USE flags for: ", Style::default().fg(Color::DarkGray)),
            Span::styled(atom, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
            Span::styled(format!("  ({} flags declared in ebuild)", app.install_use_flags.len()), Style::default().fg(Color::Cyan)),
        ]),
        Line::from(Span::styled("Flags toggled here will be saved to package.use before emerge runs.", Style::default().fg(Color::DarkGray))),
    ];
    let header = Paragraph::new(header_text).block(Block::default().borders(Borders::NONE));
    frame.render_widget(header, chunks[0]);

    // 2. Flags list
    let mut items: Vec<ListItem> = Vec::new();
    if app.install_use_flags.is_empty() {
        items.push(ListItem::new("  (No configurable USE flags declared in ebuild for this package)"));
    } else {
        for flag in &app.install_use_flags {
            let is_toggled = app.install_flags_toggled.contains(&flag.name);

            let check = if flag.enabled {
                Span::styled("[✓] ", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD))
            } else {
                Span::styled("[ ] ", Style::default().fg(Color::DarkGray))
            };

            let name_style = if flag.enabled {
                Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::White)
            };

            let def_badge = if flag.default_on {
                Span::styled(" (default: on) ", Style::default().fg(Color::DarkGray))
            } else {
                Span::styled(" (default: off) ", Style::default().fg(Color::DarkGray))
            };

            let tag = if is_toggled {
                Span::styled(" [customized] ", Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD))
            } else {
                Span::raw("")
            };

            let mut line1 = vec![
                check,
                Span::styled(&flag.name, name_style),
                tag,
                def_badge,
            ];

            if !flag.description.is_empty() {
                line1.push(Span::styled(format!("- {}", flag.description), Style::default().fg(Color::DarkGray)));
            }

            items.push(ListItem::new(Line::from(line1)));
        }
    }

    let list_title = format!(" USE Flags ({}) ", app.install_use_flags.len());
    let list = List::new(items)
        .highlight_style(Style::default().bg(Color::Rgb(30, 45, 60)).add_modifier(Modifier::BOLD))
        .block(Block::default().borders(Borders::ALL).border_style(Style::default().fg(Color::Cyan)).title(list_title));

    state.select(if app.install_use_flags.is_empty() { None } else { Some(app.selected_install_flag_row) });
    frame.render_stateful_widget(list, chunks[1], state);

    // 3. Footer controls
    let footer_text = vec![
        Line::from(vec![
            key("[Space]"), Span::raw(" Toggle Flag   "),
            key("[↑↓/jk]"), Span::raw(" Move   "),
            key("[PgUp/PgDn]"), Span::raw(" Page   "),
            Span::styled("[Enter]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" Save & Emerge   "),
            Span::styled("[Esc]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw(" Cancel (do NOT emerge)"),
        ]),
    ];
    let footer = Paragraph::new(footer_text).block(Block::default().borders(Borders::ALL));
    frame.render_widget(footer, chunks[2]);
}

fn draw_uninstall_modal(frame: &mut Frame, app: &AppState)
{
    let area = centered_rect(65, 30, frame.area());
    frame.render_widget(Clear, area);

    let atom = app.pending_uninstall_atom.as_deref().unwrap_or("package");

    let lines = vec![
        Line::from(Span::styled(
            "Are you sure you want to uninstall this package?",
            Style::default().fg(Color::Red).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Package: ", Style::default().fg(Color::White)),
            Span::styled(atom, Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD)),
        ]),
        Line::from(""),
        Line::from(Span::styled("Commands that will be executed:", Style::default().fg(Color::DarkGray))),
        Line::from(vec![
            Span::styled("  • sudo emerge --deselect ", Style::default().fg(Color::Cyan)),
            Span::styled(atom, Style::default().fg(Color::White)),
            Span::styled(" (removes from @world)", Style::default().fg(Color::DarkGray)),
        ]),
        Line::from(vec![
            Span::styled("  • sudo emerge --ask --verbose --depclean ", Style::default().fg(Color::Cyan)),
            Span::styled(atom, Style::default().fg(Color::White)),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("[y / Enter]", Style::default().fg(Color::Green).add_modifier(Modifier::BOLD)),
            Span::raw(" Confirm and uninstall   "),
            Span::styled("[n / Esc]", Style::default().fg(Color::Red).add_modifier(Modifier::BOLD)),
            Span::raw(" Cancel and return to TUI"),
        ]),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Red))
        .title(" 🗑️ Confirm Uninstall ");

    let widget = Paragraph::new(lines).block(block).wrap(Wrap { trim: true });
    frame.render_widget(widget, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect
{
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    return Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1];
}
