// USE flag descriptions, read directly from the ebuild repositories.
//
//   <repo>/profiles/use.desc         global flags        "ssl - Add support for SSL/TLS"
//   <repo>/profiles/desc/<X>.desc    USE_EXPAND flags    lua_single_target.desc: "luajit - ..."
//                                                        -> flag "lua_single_target_luajit"
//   <repo>/<cat>/<pkg>/metadata.xml  package-local flags <use><flag name="nvimpager">...</flag></use>

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use crate::backend::atom::strip_to_cp;

pub struct MetadataDb
{
    repos: HashMap<String, PathBuf>, // repo name -> location (already prefixed with root)
    main_repo: Option<PathBuf>,
    global: HashMap<String, String>, // flag -> description (use.desc + USE_EXPAND)
    profile_masked_atoms: Vec<String>, // atoms from <repo>/profiles/package.mask
}

impl MetadataDb
{
    pub fn load(root: &Path) -> Self
    {
        let mut repos = parse_repos_conf(root);

        // Fallback to the standard location if repos.conf told us nothing.
        if !repos.contains_key("gentoo")
        {
            let default_path = root.join("var/db/repos/gentoo");
            if default_path.is_dir()
            {
                repos.insert("gentoo".to_string(), default_path);
            }
        }

        let main_repo = repos.get("gentoo").cloned();

        // Main repo first so its descriptions win; overlays only fill gaps.
        let mut ordered: Vec<PathBuf> = Vec::new();
        if let Some(main) = &main_repo
        {
            ordered.push(main.clone());
        }
        for path in repos.values()
        {
            if Some(path) != main_repo.as_ref()
            {
                ordered.push(path.clone());
            }
        }

        let mut global: HashMap<String, String> = HashMap::new();
        let mut profile_masked_atoms: Vec<String> = Vec::new();

        for repo in &ordered
        {
            load_desc_file(&repo.join("profiles/use.desc"), "", &mut global);

            if let Ok(entries) = fs::read_dir(repo.join("profiles/desc"))
            {
                for entry in entries.flatten()
                {
                    let path = entry.path();
                    if path.extension().map_or(false, |e| e == "desc")
                    {
                        let prefix = path.file_stem().unwrap_or_default().to_string_lossy().to_lowercase();
                        load_desc_file(&path, &prefix, &mut global);
                    }
                }
            }

            // Read repo's profiles/package.mask (or profiles/base/package.mask)
            load_profile_mask(&repo.join("profiles/package.mask"), &mut profile_masked_atoms);
            load_profile_mask(&repo.join("profiles/base/package.mask"), &mut profile_masked_atoms);
        }

        let db = MetadataDb {
            repos,
            main_repo,
            global,
            profile_masked_atoms,
        };

        return db;
    }

    pub fn main_repo(&self) -> Option<&PathBuf>
    {
        return self.main_repo.as_ref();
    }

    pub fn all_repo_paths(&self) -> Vec<PathBuf>
    {
        let mut paths: Vec<PathBuf> = Vec::new();
        if let Some(main) = &self.main_repo
        {
            paths.push(main.clone());
        }
        for p in self.repos.values()
        {
            if !paths.contains(p)
            {
                paths.push(p.clone());
            }
        }
        return paths;
    }

    pub fn is_profile_masked(&self, cp: &str) -> bool
    {
        return self.profile_masked_atoms.contains(&cp.to_string());
    }

    // Check if an available package in repo has only ~testing keywords or is masked
    #[allow(dead_code)]
    pub fn inspect_package_mask(&self, cp: &str) -> (bool, bool)
    {
        let is_hard_masked = self.is_profile_masked(cp);
        let mut is_testing = false;

        let (category, name) = match cp.split_once('/')
        {
            Some((c, n)) => (c, n),
            None => return (is_hard_masked, false),
        };

        // Scan available ebuilds in repo to check KEYWORDS
        if let Some(repo_path) = &self.main_repo
        {
            let pkg_dir = repo_path.join(category).join(name);
            if let Ok(entries) = fs::read_dir(&pkg_dir)
            {
                let mut saw_ebuild = false;
                let mut all_testing = true;

                for entry in entries.flatten()
                {
                    let p = entry.path();
                    if p.extension().map_or(false, |ext| ext == "ebuild")
                    {
                        saw_ebuild = true;
                        if let Ok(content) = fs::read_to_string(&p)
                        {
                            for line in content.lines()
                            {
                                if line.starts_with("KEYWORDS=")
                                {
                                    let val = line.trim_start_matches("KEYWORDS=").trim_matches('"').trim_matches('\'');
                                    let has_stable = val.split_whitespace().any(|kw| !kw.starts_with('~') && !kw.starts_with('-') && kw != "*");
                                    if has_stable
                                    {
                                        all_testing = false;
                                    }
                                }
                            }
                        }
                    }
                }

                if saw_ebuild && all_testing
                {
                    is_testing = true;
                }
            }
        }

        return (is_hard_masked, is_testing);
    }

    // Package-local flag descriptions from <repo>/<cat>/<pkg>/metadata.xml
    pub fn local_flags(&self, repo: &str, cp: &str) -> HashMap<String, String>
    {
        let mut out: HashMap<String, String> = HashMap::new();

        let repo_path = match self.repos.get(repo).or(self.main_repo.as_ref())
        {
            Some(p) => p,
            None => return out,
        };

        let text = match fs::read_to_string(repo_path.join(cp).join("metadata.xml"))
        {
            Ok(t) => t,
            Err(_) => return out,
        };

        // metadata.xml has a <!DOCTYPE>, so DTDs must be allowed.
        let options = roxmltree::ParsingOptions { allow_dtd: true, ..Default::default() };
        let doc = match roxmltree::Document::parse_with_options(&text, options)
        {
            Ok(d) => d,
            Err(_) => return out,
        };

        for use_node in doc.descendants().filter(|n| n.has_tag_name("use"))
        {
            let lang = use_node.attribute("lang").unwrap_or("en");
            if lang != "en"
            {
                continue;
            }

            for flag in use_node.children().filter(|n| n.has_tag_name("flag"))
            {
                if let Some(name) = flag.attribute("name")
                {
                    // Descriptions can contain inline tags like <pkg>..</pkg>: gather all text.
                    let raw: String = flag.descendants().filter_map(|n| n.text()).collect();
                    let clean = raw.split_whitespace().collect::<Vec<&str>>().join(" ");
                    out.insert(name.to_string(), clean);
                }
            }
        }

        return out;
    }

    // Best description for a flag: package-local first, then global/USE_EXPAND.
    pub fn describe(&self, local: &HashMap<String, String>, flag: &str) -> String
    {
        if let Some(desc) = local.get(flag)
        {
            return desc.clone();
        }
        if let Some(desc) = self.global.get(flag)
        {
            return desc.clone();
        }

        return String::from("(no description found)");
    }
}

// Parse "flag - description" lines. `prefix` is set for USE_EXPAND files.
fn load_desc_file(path: &Path, prefix: &str, out: &mut HashMap<String, String>)
{
    let content = match fs::read_to_string(path)
    {
        Ok(c) => c,
        Err(_) => return,
    };

    for line in content.lines()
    {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#')
        {
            continue;
        }

        if let Some((flag, desc)) = line.split_once(" - ")
        {
            let key = if prefix.is_empty()
            {
                flag.trim().to_string()
            }
            else
            {
                format!("{}_{}", prefix, flag.trim())
            };

            out.entry(key).or_insert_with(|| desc.trim().to_string());
        }
    }
}

// Read lines from profiles/package.mask
fn load_profile_mask(path: &Path, out: &mut Vec<String>)
{
    let content = match fs::read_to_string(path)
    {
        Ok(c) => c,
        Err(_) => return,
    };

    for line in content.lines()
    {
        let line = line.split('#').next().unwrap_or("").trim();
        if let Some(token) = line.split_whitespace().next()
        {
            if let Some(cp) = strip_to_cp(token)
            {
                if !out.contains(&cp)
                {
                    out.push(cp);
                }
            }
        }
    }
}

// Read repo locations from /etc/portage/repos.conf (file or directory)
// and Portage's shipped default config.
fn parse_repos_conf(root: &Path) -> HashMap<String, PathBuf>
{
    let mut repos: HashMap<String, PathBuf> = HashMap::new();

    let candidates = [
        root.join("usr/share/portage/config/repos.conf"),
        root.join("etc/portage/repos.conf"),
    ];

    for candidate in candidates.iter()
    {
        for file in list_config_files(candidate)
        {
            let content = match fs::read_to_string(&file)
            {
                Ok(c) => c,
                Err(_) => continue,
            };

            let mut section = String::new();
            for line in content.lines()
            {
                let line = line.trim();
                if line.starts_with('[') && line.ends_with(']')
                {
                    section = line[1..line.len() - 1].trim().to_string();
                }
                else if let Some((key, value)) = line.split_once('=')
                {
                    if key.trim() == "location" && section != "DEFAULT" && !section.is_empty()
                    {
                        let location = root.join(value.trim().trim_start_matches('/'));
                        repos.insert(section.clone(), location);
                    }
                }
            }
        }
    }

    return repos;
}

// A Portage config path can be a single file or a directory of files
// (read in lexical order, skipping hidden and backup files).
pub fn list_config_files(path: &Path) -> Vec<PathBuf>
{
    let mut files: Vec<PathBuf> = Vec::new();

    if path.is_file()
    {
        files.push(path.to_path_buf());
    }
    else if let Ok(entries) = fs::read_dir(path)
    {
        for entry in entries.flatten()
        {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.starts_with('.') || name.ends_with('~')
            {
                continue;
            }

            let p = entry.path();
            if p.is_dir()
            {
                files.extend(list_config_files(&p));
            }
            else
            {
                files.push(p);
            }
        }
        files.sort();
    }

    return files;
}
