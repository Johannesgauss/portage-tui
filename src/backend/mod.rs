pub mod atom;
pub mod config;
pub mod metadata;
pub mod repo_index;
pub mod runner;
pub mod vdb;

use std::io;
use std::path::{Path, PathBuf};

use crate::domain::package::{MaskReport, Package, StagedAction, UseFlag};
use config::UserConfig;
use metadata::MetadataDb;
use repo_index::{RepoIndex, ResolveResult};
use vdb::VdbReader;

// Single entry point the UI talks to. Everything Portage-specific stays behind
// this struct, so a pkgcraft-based backend can replace it later.
pub struct Portage
{
    root: PathBuf,
    vdb: VdbReader,
    meta: MetadataDb,
    user: UserConfig,
    index: RepoIndex,
}

impl Portage
{
    pub fn open(root: &Path) -> Self
    {
        let meta = MetadataDb::load(root);
        let repo_paths = meta.all_repo_paths();
        let index = RepoIndex::index_repos(&repo_paths);

        let portage = Portage {
            root: root.to_path_buf(),
            vdb: VdbReader::new(root),
            meta,
            user: UserConfig::load(root),
            index,
        };

        return portage;
    }

    pub fn root(&self) -> &Path
    {
        return &self.root;
    }

    pub fn main_repo(&self) -> Option<&PathBuf>
    {
        return self.meta.main_repo();
    }

    pub fn installed_count(&self) -> usize
    {
        return self.vdb.installed_count();
    }

    pub fn world_atoms(&self) -> Result<Vec<String>, io::Error>
    {
        return self.vdb.load_world_atoms();
    }

    // Resolves a package query ("fceux", "games-emulation/fceux") against the repo
    pub fn query_package(&self, query: &str) -> ResolveResult
    {
        return self.index.resolve_query(query);
    }

    // Inspect if a package (installed or about to be installed) is masked
    pub fn check_mask_status(&self, cp: &str) -> MaskReport
    {
        let is_user_masked = self.user.is_masked(cp);
        let is_unmasked = self.user.is_unmasked(cp);
        let accepts_testing = self.user.accepts_testing(cp);

        // 1. Check Gentoo repository profile mask (e.g. profiles/package.mask)
        let is_profile_masked = self.meta.is_profile_masked(cp);

        // 2. Check ebuild KEYWORDS in repo
        let mut is_testing = false;
        let mut resolved = self.index.resolve_query(cp);
        if let ResolveResult::Exact(ref mut pkg) = resolved
        {
            self.index.load_package_keywords(pkg);

            // Check if any version is stable, or if all are ~arch / testing
            let mut saw_keywords = false;
            let mut has_stable_version = false;

            for ver in &pkg.versions
            {
                if !ver.keywords.is_empty()
                {
                    saw_keywords = true;
                    // Stable keywords look like "amd64", "x86". Testing keywords look like "~amd64".
                    let ver_has_stable = ver.keywords.iter().any(|kw| {
                        !kw.starts_with('~') && !kw.starts_with('-') && kw != "*"
                    });
                    if ver_has_stable
                    {
                        has_stable_version = true;
                        break;
                    }
                }
            }

            if saw_keywords && !has_stable_version
            {
                is_testing = true;
            }
        }

        let effective_profile_mask = is_profile_masked && !is_unmasked;
        let effective_testing = is_testing && !accepts_testing;

        let report = MaskReport {
            atom: cp.to_string(),
            is_user_masked,
            is_profile_masked: effective_profile_mask,
            is_testing: effective_testing,
        };

        return report;
    }

    // Full package info: VDB data + flag descriptions + flag origins + masks.
    pub fn load_package(&self, cp: &str, is_world: bool) -> Option<Package>
    {
        let record = self.vdb.read_record(cp)?;
        let repo = if record.repository.is_empty() { "gentoo".to_string() } else { record.repository.clone() };
        let local_descs = self.meta.local_flags(&repo, cp);

        let mut use_flags: Vec<UseFlag> = Vec::new();
        for (name, default_on) in &record.iuse
        {
            let flag = UseFlag {
                name: name.clone(),
                description: self.meta.describe(&local_descs, name),
                enabled: record.active_use.contains(name),
                default_on: *default_on,
                origin: self.user.origin_of(cp, name),
            };
            use_flags.push(flag);
        }
        use_flags.sort_by(|a, b| a.name.cmp(&b.name));

        let package = Package {
            category: record.category,
            name: record.name,
            version: record.version,
            slot: record.slot,
            description: record.description,
            homepage: record.homepage,
            repository: repo,
            is_world,
            is_masked: self.user.is_masked(cp) || self.meta.is_profile_masked(cp),
            is_unmasked: self.user.is_unmasked(cp),
            accepts_testing: self.user.accepts_testing(cp),
            use_flags,
            dependencies: record.dependencies,
        };

        return Some(package);
    }

    // Loads available USE flags for an uninstalled (or repository) package from its ebuild
    pub fn load_available_package_flags(&self, cp: &str) -> Vec<UseFlag>
    {
        let resolved = self.index.resolve_query(cp);
        let pkg = match resolved {
            ResolveResult::Exact(p) => p,
            _ => return Vec::new(),
        };

        let (iuse_list, _) = self.index.load_package_iuse_and_desc(&pkg);
        let local_descs = self.meta.local_flags("gentoo", cp);

        let mut flags: Vec<UseFlag> = Vec::new();

        for (name, default_on) in iuse_list {
            let origin = self.user.origin_of(cp, &name);

            let enabled = match origin {
                crate::domain::package::FlagOrigin::PackageUse => {
                    self.user.is_flag_enabled_in_package_use(cp, &name).unwrap_or(default_on)
                }
                crate::domain::package::FlagOrigin::MakeConf => {
                    self.user.is_flag_enabled_in_make_conf(&name).unwrap_or(default_on)
                }
                crate::domain::package::FlagOrigin::ProfileDefault => default_on,
            };

            let description = self.meta.describe(&local_descs, &name);

            flags.push(UseFlag {
                name,
                description,
                enabled,
                default_on,
                origin,
            });
        }

        flags.sort_by(|a, b| a.name.cmp(&b.name));
        return flags;
    }

    // Write staged changes, then re-read user config so origins & masks are fresh.
    pub fn save_staged_changes(&mut self, actions: &[StagedAction]) -> Result<String, io::Error>
    {
        let msg = config::save_all_staged_changes(&self.root, actions)?;
        self.user = UserConfig::load(&self.root);
        return Ok(msg);
    }

    pub fn refresh_package_settings(&self, package: &mut Package)
    {
        let cp = package.atom();
        package.is_masked = self.user.is_masked(&cp) || self.meta.is_profile_masked(&cp);
        package.is_unmasked = self.user.is_unmasked(&cp);
        package.accepts_testing = self.user.accepts_testing(&cp);

        for flag in package.use_flags.iter_mut()
        {
            flag.origin = self.user.origin_of(&cp, &flag.name);
        }
    }
}
