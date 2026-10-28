// Repo indexer and atom resolver for available packages in Gentoo ebuild repos.
// Allows querying packages (like "fceux" or "games-emulation/fceux") before installing.

use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;

use crate::backend::atom::split_pv;

#[derive(Debug, Clone)]
pub struct RepoPackage
{
    pub category: String,
    pub name: String,
    pub versions: Vec<RepoVersion>,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub struct RepoVersion
{
    pub version: String,
    pub slot: String,
    pub keywords: Vec<String>,
    pub ebuild_path: PathBuf,
}

pub struct RepoIndex
{
    // "cat/pkg" -> RepoPackage
    by_cp: HashMap<String, RepoPackage>,
    // "pkg" -> list of "cat/pkg"
    by_name: HashMap<String, Vec<String>>,
}

impl RepoIndex
{
    // Indexes all categories and packages across all configured ebuild repositories.
    // Extremely fast (directory name reads only, ~15-30ms for 30,000 packages).
    pub fn index_repos(repo_paths: &[PathBuf]) -> Self
    {
        let mut by_cp: HashMap<String, RepoPackage> = HashMap::new();
        let mut by_name: HashMap<String, Vec<String>> = HashMap::new();

        for repo in repo_paths
        {
            if !repo.is_dir()
            {
                continue;
            }

            let cat_entries = match fs::read_dir(repo)
            {
                Ok(entries) => entries,
                Err(_) => continue,
            };

            for cat_entry in cat_entries.flatten()
            {
                let cat_name = cat_entry.file_name().to_string_lossy().to_string();

                // Skip non-category directories in ebuild repos
                if cat_name.starts_with('.') || matches!(cat_name.as_str(), "profiles" | "eclass" | "metadata" | "licenses")
                {
                    continue;
                }

                let cat_path = cat_entry.path();
                if !cat_path.is_dir()
                {
                    continue;
                }

                if let Ok(pkg_entries) = fs::read_dir(&cat_path)
                {
                    for pkg_entry in pkg_entries.flatten()
                    {
                        let pkg_name = pkg_entry.file_name().to_string_lossy().to_string();
                        if pkg_name.starts_with('.')
                        {
                            continue;
                        }

                        let cp = format!("{}/{}", cat_name, pkg_name);

                        // Only index if it has ebuilds
                        let pkg_path = pkg_entry.path();
                        if pkg_path.is_dir()
                        {
                            let mut versions: Vec<RepoVersion> = Vec::new();

                            if let Ok(ebuild_files) = fs::read_dir(&pkg_path)
                            {
                                for eb in ebuild_files.flatten()
                                {
                                    let fname = eb.file_name().to_string_lossy().to_string();
                                    if fname.ends_with(".ebuild")
                                    {
                                        let bare = &fname[..fname.len() - 7];
                                        if let Some((pname, ver)) = split_pv(bare)
                                        {
                                            if pname == pkg_name
                                            {
                                                versions.push(RepoVersion {
                                                    version: ver,
                                                    slot: "0".to_string(), // default unless parsed
                                                    keywords: Vec::new(),
                                                    ebuild_path: eb.path(),
                                                });
                                            }
                                        }
                                    }
                                }
                            }

                            if !versions.is_empty()
                            {
                                by_name.entry(pkg_name.clone()).or_default().push(cp.clone());
                                by_cp.insert(
                                    cp,
                                    RepoPackage {
                                        category: cat_name.clone(),
                                        name: pkg_name,
                                        versions,
                                    },
                                );
                            }
                        }
                    }
                }
            }
        }

        return RepoIndex { by_cp, by_name };
    }

    // Resolves user query like "fceux" or "games-emulation/fceux"
    // Returns exact "cat/pkg" if found, or multiple matches if ambiguous.
    pub fn resolve_query(&self, query: &str) -> ResolveResult
    {
        let query_clean = query.trim().trim_start_matches(|c| c == '=' || c == '<' || c == '>' || c == '~');

        // 1. Direct cat/pkg match
        if query_clean.contains('/')
        {
            if let Some(pkg) = self.by_cp.get(query_clean)
            {
                return ResolveResult::Exact(pkg.clone());
            }
            return ResolveResult::NotFound;
        }

        // 2. Short name match (e.g. "fceux")
        if let Some(candidates) = self.by_name.get(query_clean)
        {
            if candidates.len() == 1
            {
                if let Some(pkg) = self.by_cp.get(&candidates[0])
                {
                    return ResolveResult::Exact(pkg.clone());
                }
            }
            else
            {
                return ResolveResult::Ambiguous(candidates.clone());
            }
        }

        // 3. Substring search if no direct match
        let query_lower = query_clean.to_lowercase();
        let matches: Vec<String> = self
            .by_cp
            .keys()
            .filter(|cp| cp.to_lowercase().contains(&query_lower))
            .cloned()
            .collect();

        if matches.len() == 1
        {
            if let Some(pkg) = self.by_cp.get(&matches[0])
            {
                return ResolveResult::Exact(pkg.clone());
            }
        }
        else if !matches.is_empty()
        {
            return ResolveResult::Ambiguous(matches);
        }

        return ResolveResult::NotFound;
    }

    // Loads the actual KEYWORDS from ebuilds for this package
    pub fn load_package_keywords(&self, pkg: &mut RepoPackage)
    {
        for ver in &mut pkg.versions
        {
            if let Ok(content) = fs::read_to_string(&ver.ebuild_path)
            {
                for line in content.lines()
                {
                    let trimmed = line.trim();
                    if trimmed.starts_with("KEYWORDS=")
                    {
                        let val = trimmed.trim_start_matches("KEYWORDS=").trim_matches('"').trim_matches('\'');
                        ver.keywords = val.split_whitespace().map(|s| s.to_string()).collect();
                    }
                    else if trimmed.starts_with("SLOT=")
                    {
                        let val = trimmed.trim_start_matches("SLOT=").trim_matches('"').trim_matches('\'');
                        ver.slot = val.to_string();
                    }
                }
            }
        }
    }

    // Loads declared IUSE flags and DESCRIPTION from the package's latest ebuild
    pub fn load_package_iuse_and_desc(&self, pkg: &RepoPackage) -> (Vec<(String, bool)>, String)
    {
        // Try the latest version first (versions are sorted ascending or in order)
        for ver in pkg.versions.iter().rev() {
            if let Ok(content) = fs::read_to_string(&ver.ebuild_path) {
                let mut iuse_tokens: Vec<(String, bool)> = Vec::new();
                let mut description = String::new();

                // 1. Extract DESCRIPTION
                if let Some(pos) = content.find("DESCRIPTION=") {
                    let after = &content[pos + 12..];
                    let quote_char = after.chars().next().unwrap_or('"');
                    if quote_char == '"' || quote_char == '\'' {
                        let text = &after[1..];
                        if let Some(end) = text.find(quote_char) {
                            description = text[..end].to_string();
                        }
                    }
                }

                // 2. Extract IUSE (handles single-line and multi-line assignments)
                for quote_pattern in &["IUSE=\"", "IUSE='"] {
                    let mut rest = &content[..];
                    let quote_char = if quote_pattern.ends_with('\'') { '\'' } else { '"' };

                    while let Some(pos) = rest.find(quote_pattern) {
                        let after = &rest[pos + 6..];
                        if let Some(end) = after.find(quote_char) {
                            let block = &after[..end];
                            for token in block.split_whitespace() {
                                if token.starts_with('$') || token.is_empty() {
                                    continue;
                                }
                                if let Some(flag) = token.strip_prefix('+') {
                                    if !flag.is_empty() {
                                        iuse_tokens.push((flag.to_string(), true));
                                    }
                                } else if let Some(flag) = token.strip_prefix('-') {
                                    if !flag.is_empty() {
                                        iuse_tokens.push((flag.to_string(), false));
                                    }
                                } else {
                                    iuse_tokens.push((token.to_string(), false));
                                }
                            }
                        }
                        rest = after;
                    }
                }

                // Deduplicate tokens
                let mut seen = std::collections::HashSet::new();
                let mut unique: Vec<(String, bool)> = Vec::new();
                for (flag, def) in iuse_tokens {
                    if seen.insert(flag.clone()) {
                        unique.push((flag, def));
                    }
                }

                return (unique, description);
            }
        }

        return (Vec::new(), String::new());
    }
}

pub enum ResolveResult
{
    Exact(RepoPackage),
    Ambiguous(Vec<String>),
    NotFound,
}

#[cfg(test)]
mod tests
{
    use super::*;
    use std::io::Write;
    use tempfile::tempdir;

    #[test]
    fn test_iuse_and_desc_parsing()
    {
        let dir = tempdir().unwrap();
        let ebuild_path = dir.path().join("test-1.0.0.ebuild");
        let mut file = fs::File::create(&ebuild_path).unwrap();
        writeln!(file, r#"DESCRIPTION="A test package description""#).unwrap();
        writeln!(file, r#"IUSE="alsa +opengl -debug test""#).unwrap();

        let pkg = RepoPackage {
            category: "app-misc".to_string(),
            name: "test".to_string(),
            versions: vec![RepoVersion {
                version: "1.0.0".to_string(),
                slot: "0".to_string(),
                keywords: vec!["~amd64".to_string()],
                ebuild_path,
            }],
        };

        let index = RepoIndex {
            by_cp: HashMap::new(),
            by_name: HashMap::new(),
        };

        let (iuse, desc) = index.load_package_iuse_and_desc(&pkg);
        assert_eq!(desc, "A test package description");
        assert_eq!(iuse.len(), 4);
        assert_eq!(iuse[0], ("alsa".to_string(), false));
        assert_eq!(iuse[1], ("opengl".to_string(), true));
        assert_eq!(iuse[2], ("debug".to_string(), false));
        assert_eq!(iuse[3], ("test".to_string(), false));
    }
}
