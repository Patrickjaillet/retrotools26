pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const NAME: &str = env!("CARGO_PKG_NAME");
pub const GIT_HASH: &str = env!("RETROTOOLS_GIT_HASH");
pub const BUILD_DATE: &str = env!("RETROTOOLS_BUILD_DATE");

pub struct VersionInfo {
    pub version: &'static str,
    pub git_hash: &'static str,
    pub build_date: &'static str,
}

pub fn current() -> VersionInfo {
    VersionInfo {
        version: VERSION,
        git_hash: GIT_HASH,
        build_date: BUILD_DATE,
    }
}

impl std::fmt::Display for VersionInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "v{} ({}, {})", self.version, self.git_hash, self.build_date)
    }
}
