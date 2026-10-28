use std::collections::{HashMap, HashSet};

use ratatui::widgets::ListState;

use crate::backend::Portage;
use crate::domain::package::{MaskReport, Package, StagedAction, TreeNode, UseFlag};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ActivePane
{
    Tree,
    UseFlags,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InputMode
{
    Normal,
    Search,
    InstallInput,     // typing a package name to install via emerge
    MaskPrompt,       // package is masked: asking user if they want to unmask
    InstallUseFlags,  // selecting USE flags for the new package before emerge
    UninstallPrompt,  // confirming package uninstall
}

// One visible row of the tree (the tree flattened in display order).
#[derive(Debug, Clone)]
pub struct FlatTreeItem
{
    pub path: Vec<usize>, // indices from the root list down to this node
    pub atom: String,
    pub depth: usize,
    pub has_children: bool,
    pub is_expanded: bool,
    pub is_cycle: bool, // atom already appears among its ancestors
}

pub struct AppState
{
    pub portage: Portage,
    pub packages: HashMap<String, Package>, // shared store, keyed by "cat/pkg"
    pub world: HashSet<String>,
    pub roots: Vec<TreeNode>,
    pub flat: Vec<FlatTreeItem>,

    pub selected_tree_row: usize,
    pub selected_flag_row: usize,
    pub active_pane: ActivePane,
    pub input_mode: InputMode,
    pub tree_filter: String,
    pub flag_filter: String,

    // Install flow:
    pub install_query: String, // package atom being typed for emerge
    pub pending_mask_report: Option<MaskReport>, // if masked during install
    pub install_target_atom: Option<String>,
    pub install_use_flags: Vec<UseFlag>,
    pub install_flags_toggled: HashSet<String>,
    pub selected_install_flag_row: usize,
    pub install_flag_state: ListState,
    pub install_flag_page: usize,

    // Uninstall flow:
    pub pending_uninstall_atom: Option<String>,

    pub staged: Vec<StagedAction>,
    pub should_quit: bool,
    pub status_message: String,

    // Scroll state for ratatui lists + page sizes measured at draw time.
    pub tree_state: ListState,
    pub flag_state: ListState,
    pub tree_page: usize,
    pub flag_page: usize,
}

impl AppState
{
    pub fn new(portage: Portage) -> Result<Self, std::io::Error>
    {
        let world_atoms = portage.world_atoms()?;
        let world: HashSet<String> = world_atoms.iter().cloned().collect();

        let mut packages: HashMap<String, Package> = HashMap::new();
        let mut roots: Vec<TreeNode> = Vec::new();
        let mut missing = 0;

        for atom in &world_atoms
        {
            match portage.load_package(atom, true)
            {
                Some(pkg) =>
                {
                    packages.insert(atom.clone(), pkg);
                    roots.push(TreeNode::new(atom.clone()));
                }
                None => missing += 1,
            }
        }

        let mut status = format!(
            "Loaded {} @world packages ({} installed in total).",
            roots.len(),
            portage.installed_count()
        );
        if missing > 0
        {
            status.push_str(&format!(" {} world entries not installed.", missing));
        }

        let mut state = AppState {
            portage,
            packages,
            world,
            roots,
            flat: Vec::new(),
            selected_tree_row: 0,
            selected_flag_row: 0,
            active_pane: ActivePane::Tree,
            input_mode: InputMode::Normal,
            tree_filter: String::new(),
            flag_filter: String::new(),
            install_query: String::new(),
            pending_mask_report: None,
            install_target_atom: None,
            install_use_flags: Vec::new(),
            install_flags_toggled: HashSet::new(),
            selected_install_flag_row: 0,
            install_flag_state: ListState::default(),
            install_flag_page: 8,
            pending_uninstall_atom: None,
            staged: Vec::new(),
            should_quit: false,
            status_message: status,
            tree_state: ListState::default(),
            flag_state: ListState::default(),
            tree_page: 10,
            flag_page: 5,
        };

        state.rebuild_flat();
        return Ok(state);
    }

    // Refresh entire world tree from disk after emerge install / update
    pub fn refresh_from_disk(&mut self) -> Result<(), std::io::Error>
    {
        let root = self.portage.root().to_path_buf();
        self.portage = Portage::open(&root);

        let world_atoms = self.portage.world_atoms()?;
        self.world = world_atoms.iter().cloned().collect();
        self.packages.clear();
        self.roots.clear();

        for atom in &world_atoms
        {
            if let Some(pkg) = self.portage.load_package(atom, true)
            {
                self.packages.insert(atom.clone(), pkg);
                self.roots.push(TreeNode::new(atom.clone()));
            }
        }

        self.rebuild_flat();
        self.status_message = format!("Refreshed from disk: {} @world packages.", self.roots.len());
        return Ok(());
    }

    // ------------------------------------------------------------------------
    // Tree
    // ------------------------------------------------------------------------

    pub fn rebuild_flat(&mut self)
    {
        let mut out: Vec<FlatTreeItem> = Vec::new();
        let filter = self.tree_filter.to_lowercase();

        for (idx, root) in self.roots.iter().enumerate()
        {
            if !filter.is_empty() && !package_matches(self.packages.get(&root.atom), &root.atom, &filter)
            {
                continue;
            }

            let mut ancestors: Vec<String> = Vec::new();
            flatten_node(root, vec![idx], 0, &mut ancestors, &self.packages, &mut out);
        }

        self.flat = out;

        if self.selected_tree_row >= self.flat.len()
        {
            self.selected_tree_row = self.flat.len().saturating_sub(1);
        }
    }

    pub fn selected_item(&self) -> Option<&FlatTreeItem>
    {
        return self.flat.get(self.selected_tree_row);
    }

    pub fn selected_package(&self) -> Option<&Package>
    {
        let item = self.selected_item()?;
        return self.packages.get(&item.atom);
    }

    fn ensure_loaded(&mut self, cp: &str)
    {
        if !self.packages.contains_key(cp)
        {
            let is_world = self.world.contains(cp);
            if let Some(pkg) = self.portage.load_package(cp, is_world)
            {
                self.packages.insert(cp.to_string(), pkg);
            }
        }
    }

    pub fn set_expanded(&mut self, expanded: bool)
    {
        let item = match self.selected_item()
        {
            Some(i) => i.clone(),
            None => return,
        };

        if !item.has_children
        {
            return;
        }

        // Lazily load the children the first time this node is opened.
        let deps: Vec<String> = self.packages
            .get(&item.atom)
            .map(|p| p.dependencies.clone())
            .unwrap_or_default();

        for dep in &deps
        {
            self.ensure_loaded(dep);
        }

        let children: Vec<TreeNode> = deps
            .into_iter()
            .filter(|d| self.packages.contains_key(d))
            .map(TreeNode::new)
            .collect();

        if let Some(node) = node_at_mut(&mut self.roots, &item.path)
        {
            if node.children.is_none()
            {
                node.children = Some(children);
            }
            node.is_expanded = expanded;
        }

        self.rebuild_flat();
    }

    pub fn toggle_expand(&mut self)
    {
        let expanded = self.selected_item().map_or(false, |i| i.is_expanded);
        self.set_expanded(!expanded);
    }

    // Left arrow: collapse if open, otherwise jump to the parent row.
    pub fn collapse_or_parent(&mut self)
    {
        let item = match self.selected_item()
        {
            Some(i) => i.clone(),
            None => return,
        };

        if item.is_expanded
        {
            self.set_expanded(false);
            return;
        }

        if item.depth > 0
        {
            let mut row = self.selected_tree_row;
            while row > 0
            {
                row -= 1;
                if self.flat[row].depth < item.depth
                {
                    self.select_tree_row(row);
                    return;
                }
            }
        }
    }

    pub fn move_tree(&mut self, delta: isize)
    {
        let target = clamp_move(self.selected_tree_row, delta, self.flat.len());
        self.select_tree_row(target);
    }

    fn select_tree_row(&mut self, row: usize)
    {
        if row != self.selected_tree_row
        {
            self.selected_tree_row = row;
            self.selected_flag_row = 0;
            self.flag_state = ListState::default(); // new package: flags scroll back to top
        }
    }

    // ------------------------------------------------------------------------
    // Mask & Keyword Toggles ('m' and 't')
    // ------------------------------------------------------------------------

    pub fn current_mask_state(&self, atom: &str) -> bool
    {
        for action in &self.staged
        {
            if let StagedAction::ToggleMask { atom: a, masked } = action
            {
                if a == atom
                {
                    return *masked;
                }
            }
        }

        return self.packages.get(atom).map_or(false, |p| p.is_masked);
    }

    pub fn current_testing_state(&self, atom: &str) -> bool
    {
        for action in &self.staged
        {
            if let StagedAction::ToggleTesting { atom: a, testing } = action
            {
                if a == atom
                {
                    return *testing;
                }
            }
        }

        return self.packages.get(atom).map_or(false, |p| p.accepts_testing);
    }

    pub fn toggle_mask(&mut self)
    {
        let atom = match self.selected_package()
        {
            Some(p) => p.atom(),
            None => return,
        };

        let base = self.packages.get(&atom).map_or(false, |p| p.is_masked);
        let current = self.current_mask_state(&atom);
        let new_state = !current;

        self.staged.retain(|a| !matches!(a, StagedAction::ToggleMask { atom: x, .. } if x == &atom));
        if new_state != base
        {
            self.staged.push(StagedAction::ToggleMask { atom: atom.clone(), masked: new_state });
        }

        let verb = if new_state { "masked (added to package.mask)" } else { "unmasked (removed from package.mask)" };
        self.status_message = format!("{}: {}", atom, verb);
    }

    pub fn toggle_testing_keyword(&mut self)
    {
        let atom = match self.selected_package()
        {
            Some(p) => p.atom(),
            None => return,
        };

        let base = self.packages.get(&atom).map_or(false, |p| p.accepts_testing);
        let current = self.current_testing_state(&atom);
        let new_state = !current;

        self.staged.retain(|a| !matches!(a, StagedAction::ToggleTesting { atom: x, .. } if x == &atom));
        if new_state != base
        {
            self.staged.push(StagedAction::ToggleTesting { atom: atom.clone(), testing: new_state });
        }

        let verb = if new_state { "accepts ~testing keywords" } else { "stable keywords only" };
        self.status_message = format!("{}: {}", atom, verb);
    }

    // ------------------------------------------------------------------------
    // USE flags
    // ------------------------------------------------------------------------

    // Indices (into selected package's use_flags) that pass the flag filter.
    pub fn visible_flag_indices(&self) -> Vec<usize>
    {
        let pkg = match self.selected_package()
        {
            Some(p) => p,
            None => return Vec::new(),
        };

        let filter = self.flag_filter.to_lowercase();
        let mut out: Vec<usize> = Vec::new();
        for (idx, flag) in pkg.use_flags.iter().enumerate()
        {
            let matches = filter.is_empty()
                || flag.name.to_lowercase().contains(&filter)
                || flag.description.to_lowercase().contains(&filter);
            if matches
            {
                out.push(idx);
            }
        }

        return out;
    }

    pub fn selected_flag(&self) -> Option<&UseFlag>
    {
        let visible = self.visible_flag_indices();
        let idx = *visible.get(self.selected_flag_row)?;
        return self.selected_package()?.use_flags.get(idx);
    }

    // Staged value for a flag, if the user changed it and hasn't saved yet.
    pub fn staged_value(&self, atom: &str, flag: &str) -> Option<bool>
    {
        for action in &self.staged
        {
            if let StagedAction::ToggleUse { atom: a, flag: f, enable } = action
            {
                if a == atom && f == flag
                {
                    return Some(*enable);
                }
            }
        }

        return None;
    }

    pub fn move_flag(&mut self, delta: isize)
    {
        let count = self.visible_flag_indices().len();
        self.selected_flag_row = clamp_move(self.selected_flag_row, delta, count);
    }

    pub fn toggle_use_flag(&mut self)
    {
        let (atom, name, base) = match (self.selected_package(), self.selected_flag())
        {
            (Some(p), Some(f)) => (p.atom(), f.name.clone(), f.enabled),
            _ => return,
        };

        let current = self.staged_value(&atom, &name).unwrap_or(base);
        let new_value = !current;

        // Keep at most one staged entry per (package, flag); toggling back
        // to the on-disk value removes it entirely.
        self.staged.retain(|a| !matches!(a, StagedAction::ToggleUse { atom: x, flag: y, .. } if *x == atom && *y == name));
        if new_value != base
        {
            self.staged.push(StagedAction::ToggleUse { atom: atom.clone(), flag: name.clone(), enable: new_value });
        }

        let sign = if new_value { "+" } else { "-" };
        self.status_message = format!("{}: {}{} ({} change(s) staged)", atom, sign, name, self.staged.len());
    }

    pub fn save_changes(&mut self)
    {
        if self.staged.is_empty()
        {
            self.status_message = "Nothing to save.".to_string();
            return;
        }

        match self.portage.save_staged_changes(&self.staged)
        {
            Ok(msg) =>
            {
                for action in &self.staged
                {
                    match action
                    {
                        StagedAction::ToggleUse { atom, flag, enable } =>
                        {
                            if let Some(pkg) = self.packages.get_mut(atom)
                            {
                                if let Some(f) = pkg.use_flags.iter_mut().find(|f| &f.name == flag)
                                {
                                    f.enabled = *enable;
                                }
                            }
                        }
                        _ => {}
                    }
                }

                for pkg in self.packages.values_mut()
                {
                    self.portage.refresh_package_settings(pkg);
                }

                self.staged.clear();
                self.status_message = msg;
            }
            Err(e) =>
            {
                let is_perm = e.kind() == std::io::ErrorKind::PermissionDenied;
                if is_perm
                {
                    self.status_message = format!(
                        "Error saving: Permission denied. (Did you run 'newgrp portage' or run as root/sudo?)"
                    );
                }
                else
                {
                    self.status_message = format!("Error saving: {}", e);
                }
            }
        }
    }

    // ------------------------------------------------------------------------
    // Navigation shared by both panes
    // ------------------------------------------------------------------------

    pub fn switch_pane(&mut self)
    {
        self.active_pane = match self.active_pane
        {
            ActivePane::Tree => ActivePane::UseFlags,
            ActivePane::UseFlags => ActivePane::Tree,
        };
    }

    pub fn move_active(&mut self, delta: isize)
    {
        match self.active_pane
        {
            ActivePane::Tree => self.move_tree(delta),
            ActivePane::UseFlags => self.move_flag(delta),
        }
    }

    pub fn page_active(&mut self, direction: isize)
    {
        let page = match self.active_pane
        {
            ActivePane::Tree => self.tree_page,
            ActivePane::UseFlags => self.flag_page,
        };
        self.move_active(direction * page.max(1) as isize);
    }

    // ------------------------------------------------------------------------
    // Search ("/"): filters the active pane while typing
    // ------------------------------------------------------------------------

    pub fn start_search(&mut self)
    {
        self.input_mode = InputMode::Search;
    }

    pub fn finish_search(&mut self)
    {
        self.input_mode = InputMode::Normal;
    }

    pub fn search_push(&mut self, c: char)
    {
        self.active_filter_mut().push(c);
        self.filter_changed();
    }

    pub fn search_pop(&mut self)
    {
        self.active_filter_mut().pop();
        self.filter_changed();
    }

    pub fn clear_search(&mut self)
    {
        self.active_filter_mut().clear();
        self.input_mode = InputMode::Normal;
        self.filter_changed();
    }

    pub fn active_filter(&self) -> &str
    {
        match self.active_pane
        {
            ActivePane::Tree => return &self.tree_filter,
            ActivePane::UseFlags => return &self.flag_filter,
        }
    }

    fn active_filter_mut(&mut self) -> &mut String
    {
        match self.active_pane
        {
            ActivePane::Tree => return &mut self.tree_filter,
            ActivePane::UseFlags => return &mut self.flag_filter,
        }
    }

    fn filter_changed(&mut self)
    {
        match self.active_pane
        {
            ActivePane::Tree =>
            {
                self.selected_tree_row = 0;
                self.tree_state = ListState::default();
                self.selected_flag_row = 0;
                self.flag_state = ListState::default();
                self.rebuild_flat();
            }
            ActivePane::UseFlags =>
            {
                self.selected_flag_row = 0;
                self.flag_state = ListState::default();
            }
        }
    }

    // ------------------------------------------------------------------------
    // Install Modal ('a') & Mask Prompt
    // ------------------------------------------------------------------------

    pub fn start_install_input(&mut self)
    {
        self.install_query.clear();
        self.pending_mask_report = None;
        self.input_mode = InputMode::InstallInput;
    }

    pub fn install_push(&mut self, c: char)
    {
        self.install_query.push(c);
    }

    pub fn install_pop(&mut self)
    {
        self.install_query.pop();
    }

    pub fn cancel_install_input(&mut self)
    {
        self.install_query.clear();
        self.pending_mask_report = None;
        self.input_mode = InputMode::Normal;
    }

    pub fn apply_unmask_for_pending(&mut self) -> Result<String, std::io::Error>
    {
        let report = match &self.pending_mask_report
        {
            Some(r) => r.clone(),
            None => return Ok(String::new()),
        };

        let mut actions: Vec<StagedAction> = Vec::new();
        if report.is_profile_masked || report.is_user_masked
        {
            actions.push(StagedAction::UnmaskPackage { atom: report.atom.clone() });
        }
        if report.is_testing
        {
            actions.push(StagedAction::ToggleTesting { atom: report.atom.clone(), testing: true });
        }

        let msg = self.portage.save_staged_changes(&actions)?;
        return Ok(msg);
    }

    pub fn start_install_use_flags(&mut self, atom: &str)
    {
        self.install_target_atom = Some(atom.to_string());
        self.install_use_flags = self.portage.load_available_package_flags(atom);
        self.install_flags_toggled.clear();
        self.selected_install_flag_row = 0;
        self.install_flag_state = ListState::default();
        self.input_mode = InputMode::InstallUseFlags;
    }

    pub fn toggle_install_use_flag(&mut self)
    {
        if let Some(flag) = self.install_use_flags.get_mut(self.selected_install_flag_row) {
            flag.enabled = !flag.enabled;
            if self.install_flags_toggled.contains(&flag.name) {
                self.install_flags_toggled.remove(&flag.name);
            } else {
                self.install_flags_toggled.insert(flag.name.clone());
            }
        }
    }

    pub fn move_install_flag(&mut self, delta: isize)
    {
        let len = self.install_use_flags.len();
        self.selected_install_flag_row = clamp_move(self.selected_install_flag_row, delta, len);
    }

    pub fn page_install_flag(&mut self, direction: isize)
    {
        let page = self.install_flag_page.max(1) as isize;
        self.move_install_flag(direction * page);
    }

    pub fn cancel_install_use_flags(&mut self)
    {
        self.install_target_atom = None;
        self.install_use_flags.clear();
        self.install_flags_toggled.clear();
        self.input_mode = InputMode::Normal;
        self.status_message = "Installation cancelled. (emerge was NOT invoked)".to_string();
    }

    pub fn save_install_use_flags_if_changed(&mut self) -> Result<(), std::io::Error>
    {
        let atom = match &self.install_target_atom {
            Some(a) => a.clone(),
            None => return Ok(()),
        };

        if self.install_flags_toggled.is_empty() {
            return Ok(());
        }

        let mut actions: Vec<StagedAction> = Vec::new();
        for flag in &self.install_use_flags {
            if self.install_flags_toggled.contains(&flag.name) {
                actions.push(StagedAction::ToggleUse {
                    atom: atom.clone(),
                    flag: flag.name.clone(),
                    enable: flag.enabled,
                });
            }
        }

        if !actions.is_empty() {
            self.portage.save_staged_changes(&actions)?;
        }

        return Ok(());
    }

    // ------------------------------------------------------------------------
    // Uninstall Modal ('d')
    // ------------------------------------------------------------------------

    pub fn start_uninstall(&mut self)
    {
        if let Some(pkg) = self.selected_package() {
            self.pending_uninstall_atom = Some(pkg.atom());
            self.input_mode = InputMode::UninstallPrompt;
        }
    }

    pub fn cancel_uninstall(&mut self)
    {
        self.pending_uninstall_atom = None;
        self.input_mode = InputMode::Normal;
        self.status_message = "Uninstall cancelled.".to_string();
    }
}

// ----------------------------------------------------------------------------
// Free helpers
// ----------------------------------------------------------------------------

fn package_matches(pkg: Option<&Package>, atom: &str, filter: &str) -> bool
{
    if atom.to_lowercase().contains(filter)
    {
        return true;
    }

    return pkg.map_or(false, |p| p.description.to_lowercase().contains(filter));
}

fn flatten_node(
    node: &TreeNode,
    path: Vec<usize>,
    depth: usize,
    ancestors: &mut Vec<String>,
    packages: &HashMap<String, Package>,
    out: &mut Vec<FlatTreeItem>,
)
{
    let is_cycle = ancestors.contains(&node.atom);
    let has_children = !is_cycle && packages.get(&node.atom).map_or(false, |p| !p.dependencies.is_empty());
    let is_expanded = node.is_expanded && has_children;

    out.push(FlatTreeItem {
        path: path.clone(),
        atom: node.atom.clone(),
        depth,
        has_children,
        is_expanded,
        is_cycle,
    });

    if is_expanded
    {
        if let Some(children) = &node.children
        {
            ancestors.push(node.atom.clone());
            for (idx, child) in children.iter().enumerate()
            {
                let mut child_path = path.clone();
                child_path.push(idx);
                flatten_node(child, child_path, depth + 1, ancestors, packages, out);
            }
            ancestors.pop();
        }
    }
}

fn node_at_mut<'a>(roots: &'a mut Vec<TreeNode>, path: &[usize]) -> Option<&'a mut TreeNode>
{
    let (first, rest) = path.split_first()?;
    let mut node = roots.get_mut(*first)?;
    for idx in rest
    {
        node = node.children.as_mut()?.get_mut(*idx)?;
    }

    return Some(node);
}

fn clamp_move(current: usize, delta: isize, len: usize) -> usize
{
    if len == 0
    {
        return 0;
    }

    let target = current as isize + delta;
    return target.clamp(0, len as isize - 1) as usize;
}
