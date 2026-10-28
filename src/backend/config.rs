use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use crate::backend::atom::strip_to_cp;
use crate::backend::metadata::list_config_files;
use crate::domain::package::{FlagOrigin, StagedAction};

// ============================================================================
// READING: what the user has configured (USE, masks, keywords)
// ============================================================================

pub struct UserConfig
{
    make_conf_use: Vec<String>,                // tokens of USE="..." in make.conf
    package_use: Vec<(String, Vec<String>)>,   // (cp or "*/*", ["flag", "-flag", ...])
    package_mask: Vec<String>,                 // list of masked atoms in /etc/portage/package.mask
    package_unmask: Vec<String>,               // list of unmasked atoms in /etc/portage/package.unmask
    package_keywords: Vec<String>,             // list of atoms with ~testing in /etc/portage/package.accept_keywords
}

impl UserConfig
{
    pub fn load(root: &Path) -> Self
    {
        let mut make_conf_use: Vec<String> = Vec::new();
        for file in list_config_files(&root.join("etc/portage/make.conf"))
        {
            if let Ok(content) = fs::read_to_string(&file)
            {
                make_conf_use.extend(parse_make_conf_use(&content));
            }
        }

        let mut package_use: Vec<(String, Vec<String>)> = Vec::new();
        for file in list_config_files(&root.join("etc/portage/package.use"))
        {
            if let Ok(content) = fs::read_to_string(&file)
            {
                package_use.extend(parse_package_use(&content));
            }
        }

        let package_mask = read_simple_atoms_list(&root.join("etc/portage/package.mask"));
        let package_unmask = read_simple_atoms_list(&root.join("etc/portage/package.unmask"));

        // Accept keywords can be in package.accept_keywords or package.keywords
        let mut package_keywords = read_simple_atoms_list(&root.join("etc/portage/package.accept_keywords"));
        if package_keywords.is_empty()
        {
            package_keywords = read_simple_atoms_list(&root.join("etc/portage/package.keywords"));
        }

        let config = UserConfig {
            make_conf_use,
            package_use,
            package_mask,
            package_unmask,
            package_keywords,
        };

        return config;
    }

    pub fn origin_of(&self, cp: &str, flag: &str) -> FlagOrigin
    {
        let mentions = |token: &String| token.trim_start_matches('-') == flag;

        for (atom, flags) in &self.package_use
        {
            if (atom == cp || atom == "*/*") && flags.iter().any(mentions)
            {
                return FlagOrigin::PackageUse;
            }
        }

        if self.make_conf_use.iter().any(mentions) {
            return FlagOrigin::MakeConf;
        }

        return FlagOrigin::ProfileDefault;
    }

    pub fn is_flag_enabled_in_package_use(&self, cp: &str, flag: &str) -> Option<bool>
    {
        for (atom, flags) in &self.package_use {
            if atom == cp || atom == "*/*" {
                for token in flags {
                    if token == flag {
                        return Some(true);
                    }
                    if let Some(bare) = token.strip_prefix('-') {
                        if bare == flag {
                            return Some(false);
                        }
                    }
                }
            }
        }
        return None;
    }

    pub fn is_flag_enabled_in_make_conf(&self, flag: &str) -> Option<bool>
    {
        for token in &self.make_conf_use {
            if token == flag {
                return Some(true);
            }
            if let Some(bare) = token.strip_prefix('-') {
                if bare == flag {
                    return Some(false);
                }
            }
        }
        return None;
    }

    pub fn is_masked(&self, cp: &str) -> bool
    {
        return self.package_mask.contains(&cp.to_string());
    }

    pub fn is_unmasked(&self, cp: &str) -> bool
    {
        return self.package_unmask.contains(&cp.to_string());
    }

    pub fn accepts_testing(&self, cp: &str) -> bool
    {
        return self.package_keywords.contains(&cp.to_string());
    }
}

// Reads clean list of atoms from a path (file or directory of files), stripping comments
fn read_simple_atoms_list(base: &Path) -> Vec<String>
{
    let mut atoms: Vec<String> = Vec::new();

    for file in list_config_files(base)
    {
        if let Ok(content) = fs::read_to_string(&file)
        {
            for line in content.lines()
            {
                let line = line.split('#').next().unwrap_or("").trim();
                if let Some(token) = line.split_whitespace().next()
                {
                    if let Some(cp) = strip_to_cp(token)
                    {
                        if !atoms.contains(&cp)
                        {
                            atoms.push(cp);
                        }
                    }
                }
            }
        }
    }

    return atoms;
}

// Extract tokens from every `USE="..."` assignment (may span several lines).
fn parse_make_conf_use(content: &str) -> Vec<String>
{
    let mut tokens: Vec<String> = Vec::new();
    let mut rest = content;

    while let Some(pos) = rest.find("USE=\"")
    {
        // Must be at the start of a line (ignores "#USE=", "CPU_FLAGS_X86_USE=", ...)
        let line_start = rest[..pos].rfind('\n').map_or(0, |i| i + 1);
        let at_line_start = rest[line_start..pos].trim().is_empty();

        let after = &rest[pos + 5..];
        let end = after.find('"').unwrap_or(after.len());

        if at_line_start
        {
            for token in after[..end].split_whitespace()
            {
                if !token.starts_with('$')
                {
                    tokens.push(token.to_string());
                }
            }
        }

        rest = &after[end.min(after.len())..];
        if rest.starts_with('"')
        {
            rest = &rest[1..];
        }
    }

    return tokens;
}

// Parse package.use lines, including the USE_EXPAND form:
//   "app-editors/neovim LUA_SINGLE_TARGET: luajit" -> ["lua_single_target_luajit"]
fn parse_package_use(content: &str) -> Vec<(String, Vec<String>)>
{
    let mut entries: Vec<(String, Vec<String>)> = Vec::new();

    for line in content.lines()
    {
        let line = line.split('#').next().unwrap_or("").trim();
        let mut tokens = line.split_whitespace();

        let atom = match tokens.next().and_then(strip_to_cp)
        {
            Some(a) => a,
            None => continue,
        };

        let mut flags: Vec<String> = Vec::new();
        let mut expand_prefix = String::new();
        for token in tokens
        {
            if let Some(name) = token.strip_suffix(':')
            {
                expand_prefix = name.to_lowercase();
                continue;
            }

            if expand_prefix.is_empty()
            {
                flags.push(token.to_string());
            }
            else
            {
                let (sign, bare) = match token.strip_prefix('-')
                {
                    Some(b) => ("-", b),
                    None => ("", token),
                };
                flags.push(format!("{}{}_{}", sign, expand_prefix, bare));
            }
        }

        entries.push((atom, flags));
    }

    return entries;
}

// ============================================================================
// WRITING: commit staged changes (package.use, package.mask, package.accept_keywords, package.unmask)
// ============================================================================

const MANAGED_FILE: &str = "zz-portage-manager";

pub fn config_target_file(root: &Path, rel_dir: &str) -> PathBuf
{
    let base = root.join("etc/portage").join(rel_dir);
    if base.is_dir()
    {
        return base.join(MANAGED_FILE);
    }

    return base;
}

pub fn save_all_staged_changes(root: &Path, actions: &[StagedAction]) -> Result<String, io::Error>
{
    if actions.is_empty()
    {
        return Ok("No staged changes to save.".to_string());
    }

    let mut use_actions: Vec<&StagedAction> = Vec::new();
    let mut mask_actions: Vec<(&str, bool)> = Vec::new();
    let mut keyword_actions: Vec<(&str, bool)> = Vec::new();
    let mut unmask_actions: Vec<(&str, bool)> = Vec::new();

    for action in actions
    {
        match action
        {
            StagedAction::ToggleUse { .. } => use_actions.push(action),
            StagedAction::ToggleMask { atom, masked } => mask_actions.push((atom, *masked)),
            StagedAction::ToggleTesting { atom, testing } => keyword_actions.push((atom, *testing)),
            StagedAction::UnmaskPackage { atom } => {
                unmask_actions.push((atom, true));
                // If user had an entry in package.mask, remove it as well
                mask_actions.push((atom, false));
            }
        }
    }

    if !use_actions.is_empty()
    {
        save_use_entries(root, &use_actions)?;
    }
    if !mask_actions.is_empty()
    {
        save_simple_list_entries(root, "package.mask", &mask_actions)?;
    }
    if !keyword_actions.is_empty()
    {
        save_simple_list_entries(root, "package.accept_keywords", &keyword_actions)?;
    }
    if !unmask_actions.is_empty()
    {
        save_simple_list_entries(root, "package.unmask", &unmask_actions)?;
    }

    let msg = format!("Saved {} change(s) across /etc/portage/", actions.len());
    return Ok(msg);
}

fn save_use_entries(root: &Path, actions: &[&StagedAction]) -> Result<(), io::Error>
{
    let target_file = config_target_file(root, "package.use");
    if let Some(parent) = target_file.parent()
    {
        fs::create_dir_all(parent)?;
    }

    let mut lines: Vec<String> = Vec::new();
    if target_file.exists()
    {
        let content = fs::read_to_string(&target_file)?;
        lines = content.lines().map(|s| s.to_string()).collect();
    }
    else
    {
        lines.push("# Managed by portage-manager".to_string());
    }

    for action in actions
    {
        if let StagedAction::ToggleUse { atom, flag, enable } = action
        {
            apply_use_change(&mut lines, atom, flag, *enable);
        }
    }

    lines.retain(|line| line.split_whitespace().count() != 1 || line.trim_start().starts_with('#'));

    let mut file = OpenOptions::new().create(true).write(true).truncate(true).open(&target_file)?;
    for line in &lines
    {
        writeln!(file, "{}", line)?;
    }

    return Ok(());
}

fn apply_use_change(lines: &mut Vec<String>, atom: &str, flag: &str, enable: bool)
{
    let flag_entry = if enable { flag.to_string() } else { format!("-{}", flag) };
    let mut last_match: Option<usize> = None;

    for (idx, line) in lines.iter_mut().enumerate()
    {
        let tokens: Vec<&str> = line.split_whitespace().collect();
        let editable = tokens.first() == Some(&atom) && !tokens.iter().any(|t| t.ends_with(':'));
        if !editable
        {
            continue;
        }

        let kept: Vec<&str> = tokens
            .iter()
            .copied()
            .filter(|t| t.trim_start_matches('-') != flag)
            .collect();

        *line = kept.join(" ");
        last_match = Some(idx);
    }

    match last_match
    {
        Some(idx) => lines[idx] = format!("{} {}", lines[idx], flag_entry),
        None => lines.push(format!("{} {}", atom, flag_entry)),
    }
}

// Write or remove simple single-atom entries (e.g. package.mask, package.accept_keywords, package.unmask)
fn save_simple_list_entries(root: &Path, rel_dir: &str, actions: &[(&str, bool)]) -> Result<(), io::Error>
{
    let target_file = config_target_file(root, rel_dir);
    if let Some(parent) = target_file.parent()
    {
        fs::create_dir_all(parent)?;
    }

    let mut lines: Vec<String> = Vec::new();
    if target_file.exists()
    {
        let content = fs::read_to_string(&target_file)?;
        lines = content.lines().map(|s| s.to_string()).collect();
    }
    else
    {
        lines.push("# Managed by portage-manager".to_string());
    }

    for (atom, should_include) in actions
    {
        // Remove existing line matching atom
        lines.retain(|line| {
            let token = line.split_whitespace().next().unwrap_or("");
            strip_to_cp(token).as_deref() != Some(*atom)
        });

        if *should_include
        {
            lines.push(atom.to_string());
        }
    }

    let mut file = OpenOptions::new().create(true).write(true).truncate(true).open(&target_file)?;
    for line in &lines
    {
        writeln!(file, "{}", line)?;
    }

    return Ok(());
}

#[cfg(test)]
mod tests
{
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn save_to_single_file()
    {
        let tmp = tempdir().unwrap();
        let actions = vec![
            StagedAction::ToggleUse { atom: "app-editors/neovim".into(), flag: "test".into(), enable: true },
            StagedAction::ToggleUse { atom: "app-editors/neovim".into(), flag: "nvimpager".into(), enable: false },
            StagedAction::ToggleUse { atom: "app-editors/neovim".into(), flag: "test".into(), enable: false },
        ];

        save_all_staged_changes(tmp.path(), &actions).unwrap();

        let content = fs::read_to_string(tmp.path().join("etc/portage/package.use")).unwrap();
        assert!(content.contains("app-editors/neovim -nvimpager -test"));
    }

    #[test]
    fn save_masks_and_keywords()
    {
        let tmp = tempdir().unwrap();
        let actions = vec![
            StagedAction::ToggleMask { atom: "app-editors/neovim".into(), masked: true },
            StagedAction::ToggleTesting { atom: "app-editors/neovim".into(), testing: true },
            StagedAction::UnmaskPackage { atom: "games-emulation/fceux".into() },
        ];

        save_all_staged_changes(tmp.path(), &actions).unwrap();

        let mask_content = fs::read_to_string(tmp.path().join("etc/portage/package.mask")).unwrap();
        assert!(mask_content.contains("app-editors/neovim"));

        let kw_content = fs::read_to_string(tmp.path().join("etc/portage/package.accept_keywords")).unwrap();
        assert!(kw_content.contains("app-editors/neovim"));

        let unmask_content = fs::read_to_string(tmp.path().join("etc/portage/package.unmask")).unwrap();
        assert!(unmask_content.contains("games-emulation/fceux"));
    }
}
