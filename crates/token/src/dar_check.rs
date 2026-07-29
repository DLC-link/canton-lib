use canton_api_client::apis::configuration::Configuration;
use canton_api_client::apis::default_api;
use log::{error, info, warn};
use semver::Version;
use std::collections::{HashMap, HashSet};
use std::io::Read;
use std::path::Path;

#[derive(Debug, Clone)]
pub struct PackageInfo {
    pub name: String,
    pub version: String,
    pub package_id: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DarCheckStatus {
    Pass,
    Fail,
}

#[derive(Debug, Clone)]
pub struct DarCheckResult {
    pub status: DarCheckStatus,
    pub missing: Vec<PackageInfo>,
    pub found: Vec<PackageInfo>,
    pub total_expected: usize,
}

#[derive(Debug, Clone)]
pub struct Params {
    pub ledger_host: String,
    pub access_token: String,
    pub dar_dirs: Vec<String>,
}

/// Check whether all required DAR packages are uploaded to the participant.
///
/// Scans `.dar` files in `params.dar_dirs` to determine expected packages,
/// then fetches the list of package IDs from the Ledger API and compares them.
pub async fn check(params: Params) -> Result<DarCheckResult, String> {
    let expected = scan_dar_dirs(&params.dar_dirs)?;

    let total_expected = expected.len();
    info!("Found {} expected packages from DAR files", total_expected);

    // Fetch package IDs from the participant via JSON Ledger API
    let mut config = Configuration::new();
    config.base_path = params.ledger_host.clone();
    config.bearer_access_token = Some(params.access_token.clone());

    let response = default_api::get_v2_packages(&config)
        .await
        .map_err(|e| format!("Failed to fetch packages from participant: {}", e))?;

    let participant_packages: HashSet<String> = response.package_ids.into_iter().collect();

    info!(
        "Fetched {} packages from participant",
        participant_packages.len()
    );

    let result = compare_packages(&expected, &participant_packages);

    // Log summary
    match &result.status {
        DarCheckStatus::Pass => {
            info!(
                "DAR check passed: all {} expected packages found on participant",
                result.total_expected
            );
        }
        DarCheckStatus::Fail => {
            error!(
                "DAR check failed: {} of {} packages missing",
                result.missing.len(),
                result.total_expected
            );
            for info in &result.missing {
                error!("  Missing: {} v{}", info.name, info.version);
            }
        }
    }

    Ok(result)
}

/// Scan DAR directories and extract package info from each DAR file.
/// For each package family, only the latest version is returned.
pub fn scan_dar_dirs(dar_dirs: &[String]) -> Result<Vec<PackageInfo>, String> {
    let mut all_dars: Vec<DarEntry> = Vec::new();

    for dir in dar_dirs {
        let dir_path = Path::new(dir);
        if !dir_path.exists() {
            return Err(format!("DAR directory not found: {}", dir));
        }

        let entries = std::fs::read_dir(dir_path)
            .map_err(|e| format!("Failed to read directory '{}': {}", dir, e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read dir entry: {}", e))?;
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("dar") {
                continue;
            }

            match extract_dar_info(&path) {
                Ok(info) => all_dars.push(info),
                Err(e) => warn!("Skipping {}: {}", path.display(), e),
            }
        }
    }

    // Group by package name, keep only latest version
    let mut latest_by_name: HashMap<String, DarEntry> = HashMap::new();
    for dar in all_dars {
        let existing = latest_by_name.get(&dar.name);
        if existing.is_none() || existing.unwrap().version < dar.version {
            latest_by_name.insert(dar.name.clone(), dar);
        }
    }

    let mut result: Vec<PackageInfo> = latest_by_name
        .into_values()
        .map(|d| PackageInfo {
            name: d.name,
            version: d.version.to_string(),
            package_id: d.package_id,
        })
        .collect();

    result.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(result)
}

/// Compare expected packages against the actual package IDs on the participant.
pub fn compare_packages(
    expected: &[PackageInfo],
    participant_packages: &HashSet<String>,
) -> DarCheckResult {
    let mut missing = Vec::new();
    let mut found = Vec::new();

    for info in expected {
        if participant_packages.contains(&info.package_id) {
            found.push(info.clone());
        } else {
            missing.push(info.clone());
        }
    }

    missing.sort_by(|a, b| a.name.cmp(&b.name));
    found.sort_by(|a, b| a.name.cmp(&b.name));

    let status = if missing.is_empty() {
        DarCheckStatus::Pass
    } else {
        DarCheckStatus::Fail
    };

    DarCheckResult {
        status,
        missing,
        found,
        total_expected: expected.len(),
    }
}

/// Internal struct for DAR parsing (before version selection)
struct DarEntry {
    name: String,
    version: Version,
    package_id: String,
}

/// Extract package name, version, and main package ID from a DAR file.
///
/// DAR files are ZIP archives containing META-INF/MANIFEST.MF.
/// The Name field gives us the DAR name (e.g., "cbtc-1.1.1").
/// The Main-Dalf field path contains the package ID as a 64-char hex hash.
fn extract_dar_info(path: &Path) -> Result<DarEntry, String> {
    let file =
        std::fs::File::open(path).map_err(|e| format!("Failed to open {}: {}", path.display(), e))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| format!("Failed to read ZIP: {}", e))?;
    let mut manifest_file = archive
        .by_name("META-INF/MANIFEST.MF")
        .map_err(|e| format!("No META-INF/MANIFEST.MF: {}", e))?;

    let mut manifest_content = String::new();
    manifest_file
        .read_to_string(&mut manifest_content)
        .map_err(|e| format!("Failed to read MANIFEST.MF: {}", e))?;

    // JAR manifests use line wrapping: continuation lines start with a single space.
    let unwrapped = manifest_content.replace("\r\n ", "").replace("\n ", "");

    let name = unwrapped
        .lines()
        .find(|l| l.starts_with("Name: "))
        .map(|l| l.strip_prefix("Name: ").unwrap().trim().to_string())
        .ok_or("No Name field in MANIFEST.MF")?;

    let main_dalf = unwrapped
        .lines()
        .find(|l| l.starts_with("Main-Dalf: "))
        .map(|l| l.strip_prefix("Main-Dalf: ").unwrap().trim().to_string())
        .ok_or("No Main-Dalf field in MANIFEST.MF")?;

    // Main-Dalf format: {name}-{package_id}/{name}-{package_id}.dalf
    let dir_part = main_dalf
        .split('/')
        .next()
        .ok_or("Invalid Main-Dalf format")?;

    let package_id = dir_part
        .strip_prefix(&format!("{}-", name))
        .ok_or_else(|| format!("Main-Dalf dir '{}' doesn't start with '{}-'", dir_part, name))?
        .to_string();

    if package_id.len() != 64 || !package_id.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!(
            "Expected 64-char hex package ID, got '{}' (len={})",
            package_id,
            package_id.len()
        ));
    }

    let version = parse_version_from_name(&name)?;
    // Safe for x.y.z semver: Version::to_string() round-trips to the original text
    let pkg_name = name[..name.len() - version.to_string().len() - 1].to_string();

    Ok(DarEntry {
        name: pkg_name,
        version,
        package_id,
    })
}

/// Parse the semver version from a DAR name like "utility-commercials-v0-0.2.2".
/// Tries progressively larger suffixes of the dash-split name parts as a semver string.
fn parse_version_from_name(name: &str) -> Result<Version, String> {
    let parts: Vec<&str> = name.split('-').collect();
    for i in (0..parts.len()).rev() {
        let candidate = parts[i..].join("-");
        if let Ok(v) = Version::parse(&candidate) {
            return Ok(v);
        }
    }
    Err(format!("Could not parse semver from DAR name: {}", name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_packages_present() {
        let expected = vec![
            PackageInfo { name: "cbtc".into(), version: "1.1.1".into(), package_id: "aaa".into() },
            PackageInfo { name: "splice-util".into(), version: "0.1.4".into(), package_id: "bbb".into() },
        ];
        let participant: HashSet<String> = ["aaa", "bbb", "ccc"].iter().map(|s| s.to_string()).collect();

        let result = compare_packages(&expected, &participant);

        assert_eq!(result.status, DarCheckStatus::Pass);
        assert_eq!(result.total_expected, 2);
        assert_eq!(result.found.len(), 2);
        assert!(result.missing.is_empty());
    }

    #[test]
    fn some_packages_missing() {
        let expected = vec![
            PackageInfo { name: "cbtc".into(), version: "1.1.1".into(), package_id: "aaa".into() },
            PackageInfo { name: "splice-util".into(), version: "0.1.4".into(), package_id: "bbb".into() },
            PackageInfo { name: "utility-registry-v0".into(), version: "0.4.0".into(), package_id: "ccc".into() },
        ];
        let participant: HashSet<String> = ["aaa"].iter().map(|s| s.to_string()).collect();

        let result = compare_packages(&expected, &participant);

        assert_eq!(result.status, DarCheckStatus::Fail);
        assert_eq!(result.total_expected, 3);
        assert_eq!(result.found.len(), 1);
        assert_eq!(result.missing.len(), 2);
        assert_eq!(result.missing[0].name, "splice-util");
        assert_eq!(result.missing[1].name, "utility-registry-v0");
    }

    #[test]
    fn empty_expected() {
        let expected: Vec<PackageInfo> = vec![];
        let participant: HashSet<String> = ["aaa"].iter().map(|s| s.to_string()).collect();

        let result = compare_packages(&expected, &participant);

        assert_eq!(result.status, DarCheckStatus::Pass);
        assert_eq!(result.total_expected, 0);
    }

    #[test]
    fn empty_participant() {
        let expected = vec![
            PackageInfo { name: "cbtc".into(), version: "1.1.1".into(), package_id: "aaa".into() },
        ];
        let participant: HashSet<String> = HashSet::new();

        let result = compare_packages(&expected, &participant);

        assert_eq!(result.status, DarCheckStatus::Fail);
        assert_eq!(result.missing.len(), 1);
        assert_eq!(result.missing[0].name, "cbtc");
    }

    #[test]
    fn scan_dar_dirs_reads_real_dars() {
        let result = scan_dar_dirs(&["cbtc-dars/dars/cbtc".to_string()]);
        assert!(result.is_ok());
        let packages = result.unwrap();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "cbtc");
        assert_eq!(packages[0].package_id.len(), 64);
    }

    #[test]
    fn scan_dar_dirs_missing_dir() {
        let result = scan_dar_dirs(&["nonexistent/path".to_string()]);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("not found"));
    }

    #[test]
    fn parse_version_from_dar_names() {
        assert_eq!(parse_version_from_name("cbtc-1.1.1").unwrap(), Version::parse("1.1.1").unwrap());
        assert_eq!(parse_version_from_name("utility-commercials-v0-0.4.1").unwrap(), Version::parse("0.4.1").unwrap());
        assert_eq!(parse_version_from_name("splice-amulet-0.1.17").unwrap(), Version::parse("0.1.17").unwrap());
        assert!(parse_version_from_name("no-version-here").is_err());
    }
}
