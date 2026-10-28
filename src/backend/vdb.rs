use std::collections::HashMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::backend::atom::{split_pv, strip_to_cp};

// Where an installed package lives in /var/db/pkg.
pub struct InstalledEntry
{
    pub dir: PathBuf,
    pub version: String,
}

// Raw files of one VDB entry, before descriptions/origins are attached.
pub struct VdbRecord
{
    pub category: String,
    pub name: String,
    pub version: String,
    pub slot: String,
    pub description: String,
    pub homepage: String,
    pub repository: String,
    pub iuse: Vec<(String, bool)>, // (flag, default_on)
    pub active_use: Vec<String>,
    pub dependencies: Vec<String>,
}

pub struct VdbReader
{
    root: PathBuf,
    installed: HashMap<String, InstalledEntry>, // "cat/pkg" -> entry
}

impl VdbReader
{
    // Index every installed package once (two readdir levels: fast, ~ms).
    pub fn new(root: &Path) -> Self
    {
        let mut installed: HashMap<String, InstalledEntry> = HashMap::new();
        let vdb = root.join("var/db/pkg");

        if let Ok(categories) = fs::read_dir(&vdb)
        {
            for cat_entry in categories.flatten()
            {
                let category = cat_entry.file_name().to_string_lossy().to_string();
                let mut pkgs: Vec<_> = match fs::read_dir(cat_entry.path())
                {
                    Ok(p) => p.flatten().collect(),
                    Err(_) => continue,
                };
                pkgs.sort_by_key(|e| e.file_name());

                for pkg_entry in pkgs
                {
                    let dir_name = pkg_entry.file_name().to_string_lossy().to_string();
                    if dir_name.starts_with('-') // "-MERGING-..." leftovers
                    {
                        continue;
                    }

                    if let Some((name, version)) = split_pv(&dir_name)
                    {
                        let entry = InstalledEntry { dir: pkg_entry.path(), version };
                        installed.insert(format!("{}/{}", category, name), entry);
                    }
                }
            }
        }

        let reader = VdbReader {
            root: root.to_path_buf(),
            installed,
        };

        return reader;
    }

    pub fn installed_count(&self) -> usize
    {
        return self.installed.len();
    }

    // /var/lib/portage/world, reduced to "cat/pkg" (slots/repos stripped)
    pub fn load_world_atoms(&self) -> Result<Vec<String>, io::Error>
    {
        let world_file = self.root.join("var/lib/portage/world");
        let mut atoms: Vec<String> = Vec::new();

        if !world_file.exists()
        {
            return Ok(atoms);
        }

        let content = fs::read_to_string(&world_file)?;
        for line in content.lines()
        {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#')
            {
                continue;
            }

            if let Some(cp) = strip_to_cp(trimmed)
            {
                if !atoms.contains(&cp)
                {
                    atoms.push(cp);
                }
            }
        }

        return Ok(atoms);
    }

    pub fn read_record(&self, cp: &str) -> Option<VdbRecord>
    {
        let entry = self.installed.get(cp)?;
        let (category, name) = cp.split_once('/')?;
        let dir = &entry.dir;

        let iuse: Vec<(String, bool)> = read_file(dir, "IUSE")
            .split_whitespace()
            .map(|t| (t.trim_start_matches(['+', '-']).to_string(), t.starts_with('+')))
            .collect();

        let active_use: Vec<String> = read_file(dir, "USE")
            .split_whitespace()
            .map(|t| t.to_string())
            .collect();

        let deps_raw = format!("{} {}", read_file(dir, "RDEPEND"), read_file(dir, "PDEPEND"));
        let dependencies = self.parse_dependencies(&deps_raw, cp);

        let record = VdbRecord {
            category: category.to_string(),
            name: name.to_string(),
            version: entry.version.clone(),
            slot: read_file(dir, "SLOT"),
            description: read_file(dir, "DESCRIPTION"),
            homepage: read_file(dir, "HOMEPAGE"),
            repository: read_file(dir, "repository"),
            iuse,
            active_use,
            dependencies,
        };

        return Some(record);
    }

    // Turn a dependency string into the list of INSTALLED "cat/pkg" atoms.
    // Skips blockers (!foo), operators (||, parens), USE conditionals (foo?),
    // and alternatives in || ( ) groups that are not installed.
    fn parse_dependencies(&self, raw: &str, self_cp: &str) -> Vec<String>
    {
        let mut deps: Vec<String> = Vec::new();

        for token in raw.split_whitespace()
        {
            if token.starts_with('!') || token.ends_with('?') || matches!(token, "||" | "(" | ")" | "^^")
            {
                continue;
            }

            if let Some(cp) = strip_to_cp(token)
            {
                if cp != self_cp && self.installed.contains_key(&cp) && !deps.contains(&cp)
                {
                    deps.push(cp);
                }
            }
        }

        deps.sort();
        return deps;
    }
}

// Read a one-value VDB file, trimmed; empty string if missing.
fn read_file(dir: &Path, filename: &str) -> String
{
    match fs::read_to_string(dir.join(filename))
    {
        Ok(content) => return content.trim().to_string(),
        Err(_) => return String::new(),
    }
}
