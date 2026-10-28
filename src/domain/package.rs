// ============================================================================
// 1. USE FLAG DEFINITIONS
// ============================================================================

// Where the current (on-disk) setting of a flag comes from.
#[derive(Debug, Clone, PartialEq)]
pub enum FlagOrigin
{
    ProfileDefault, // not mentioned by the user: profile / IUSE default decided it
    MakeConf,       // mentioned in USE="..." in /etc/portage/make.conf
    PackageUse,     // mentioned in /etc/portage/package.use for this package
}

#[derive(Debug, Clone, PartialEq)]
pub struct UseFlag
{
    pub name: String,
    pub description: String,
    pub enabled: bool,    // state as built (VDB "USE" file), updated after a save
    pub default_on: bool, // declared as "+flag" in IUSE
    pub origin: FlagOrigin,
}

// ============================================================================
// 2. MASK INSPECTION REASONS
// ============================================================================

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum MaskReason
{
    UserMasked,     // in /etc/portage/package.mask
    ProfileMasked,  // in repo profiles/package.mask (needs package.unmask)
    KeywordTesting, // ebuild has ~testing keyword (needs package.accept_keywords)
}

#[derive(Debug, Clone, PartialEq)]
pub struct MaskReport
{
    pub atom: String,
    pub is_user_masked: bool,
    pub is_profile_masked: bool,
    pub is_testing: bool,
}

impl MaskReport
{
    pub fn is_masked(&self) -> bool
    {
        return self.is_user_masked || self.is_profile_masked || self.is_testing;
    }

    pub fn summary(&self) -> Vec<String>
    {
        let mut list: Vec<String> = Vec::new();
        if self.is_user_masked
        {
            list.push("User mask (in /etc/portage/package.mask)".to_string());
        }
        if self.is_profile_masked
        {
            list.push("Gentoo profile hard-mask (needs /etc/portage/package.unmask)".to_string());
        }
        if self.is_testing
        {
            list.push("Testing keyword ~arch (needs /etc/portage/package.accept_keywords)".to_string());
        }
        return list;
    }
}

// ============================================================================
// 3. INSTALLED PACKAGE
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub struct Package
{
    pub category: String,
    pub name: String,
    pub version: String,
    pub slot: String,
    pub description: String,
    pub homepage: String,
    pub repository: String,
    pub is_world: bool,
    pub is_masked: bool,         // is package masked by user in package.mask?
    pub is_unmasked: bool,       // is package explicitly unmasked in package.unmask?
    pub accepts_testing: bool,   // is ~arch accepted in package.accept_keywords?
    pub use_flags: Vec<UseFlag>,
    pub dependencies: Vec<String>, // installed runtime deps as "cat/pkg" atoms
}

impl Package
{
    // "category/name", e.g. "app-editors/neovim"
    pub fn atom(&self) -> String
    {
        return format!("{}/{}", self.category, self.name);
    }

    // "category/name-version", e.g. "app-editors/neovim-0.10.4"
    pub fn cpv(&self) -> String
    {
        return format!("{}/{}-{}", self.category, self.name, self.version);
    }
}

// ============================================================================
// 4. DEPENDENCY TREE NODE
// ============================================================================

#[derive(Debug, Clone)]
pub struct TreeNode
{
    pub atom: String,
    pub is_expanded: bool,
    pub children: Option<Vec<TreeNode>>,
}

impl TreeNode
{
    pub fn new(atom: String) -> Self
    {
        let node = TreeNode {
            atom,
            is_expanded: false,
            children: None,
        };

        return node;
    }
}

// ============================================================================
// 5. STAGED CHANGES BUFFER
// ============================================================================

#[derive(Debug, Clone, PartialEq)]
pub enum StagedAction
{
    ToggleUse  { atom: String, flag: String, enable: bool },
    ToggleMask { atom: String, masked: bool },
    ToggleTesting { atom: String, testing: bool },
    UnmaskPackage { atom: String },
}
