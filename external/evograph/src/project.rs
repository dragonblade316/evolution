use std::collections::{BTreeMap, HashSet};
use std::fs;
use std::path::{Path, PathBuf};

use cargo_metadata::{Metadata, MetadataCommand, Package, TargetKind};
use thiserror::Error;
use toml_edit::{DocumentMut, Item};

#[derive(Debug, Error)]
pub enum ProjectError {
    #[error("no Cargo.toml was found in or above {0}")]
    ManifestNotFound(PathBuf),
    #[error("failed to read {path}: {source}")]
    Read {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("invalid manifest {path}: {message}")]
    Manifest { path: PathBuf, message: String },
}

#[derive(Debug, Clone)]
pub struct SourcePackage {
    pub package_name: String,
    pub crate_name: String,
    pub manifest_path: PathBuf,
    pub source_root: PathBuf,
    pub workspace_only: bool,
}

#[derive(Debug, Clone)]
pub struct ProjectInfo {
    pub opened_path: PathBuf,
    pub app_manifest: PathBuf,
    pub workspace_manifest: PathBuf,
    pub package_name: String,
    pub config_path: PathBuf,
    pub source_packages: Vec<SourcePackage>,
    pub direct_dependencies: HashSet<String>,
    pub workspace_dependencies: BTreeMap<String, PathBuf>,
    pub metadata_warning: Option<String>,
}

impl ProjectInfo {
    pub fn discover(path: impl AsRef<Path>) -> Result<Self, ProjectError> {
        let opened_path = path.as_ref().canonicalize().unwrap_or_else(|_| path.as_ref().into());
        let app_manifest = find_manifest(&opened_path)?;
        match MetadataCommand::new().manifest_path(&app_manifest).exec() {
            Ok(metadata) => Self::from_metadata(opened_path, app_manifest, metadata),
            Err(error) => Self::from_manifests(opened_path, app_manifest, Some(error.to_string())),
        }
    }

    fn from_metadata(
        opened_path: PathBuf,
        requested_manifest: PathBuf,
        metadata: Metadata,
    ) -> Result<Self, ProjectError> {
        let workspace_manifest = metadata.workspace_root.join("Cargo.toml").into_std_path_buf();
        let app_package = choose_app_package(&metadata, &requested_manifest).ok_or_else(|| {
            ProjectError::Manifest {
                path: requested_manifest.clone(),
                message: "Cargo metadata did not contain an application package".into(),
            }
        })?;
        let app_manifest = app_package.manifest_path.clone().into_std_path_buf();
        let app_dir = app_manifest.parent().unwrap_or(Path::new(".")).to_path_buf();
        let direct_dependencies: HashSet<String> = app_package
            .dependencies
            .iter()
            .filter(|dependency| dependency.kind == cargo_metadata::DependencyKind::Normal)
            .map(|dependency| {
                dependency
                    .rename
                    .clone()
                    .unwrap_or_else(|| dependency.name.replace('-', "_"))
            })
            .collect();

        let workspace_dependencies = workspace_dependency_paths(&workspace_manifest)?;
        let workspace_names: HashSet<_> = workspace_dependencies.keys().cloned().collect();
        let mut source_packages = Vec::new();
        for package in &metadata.packages {
            let manifest = package.manifest_path.clone().into_std_path_buf();
            let Some(root) = manifest.parent().map(Path::to_path_buf) else {
                continue;
            };
            let library_target = package
                .targets
                .iter()
                .find(|target| target.kind.iter().any(|kind| *kind == TargetKind::Lib));
            let mut crate_name = library_target
                .map(|target| target.name.replace('-', "_"))
                .unwrap_or_else(|| package.name.replace('-', "_"));
            let source_root = library_target
                .and_then(|target| target.src_path.parent())
                .map(|path| path.to_path_buf().into_std_path_buf())
                .unwrap_or_else(|| root.join("src"));
            let package_name = package.name.to_string();
            let is_app = package.id == app_package.id;
            let app_dependency = app_package
                .dependencies
                .iter()
                .find(|dependency| dependency.name == package_name);
            let directly_available = direct_dependencies.contains(&crate_name) || app_dependency.is_some();
            if let Some(rename) = app_dependency.and_then(|dependency| dependency.rename.as_ref()) {
                crate_name = rename.replace('-', "_");
            }
            let workspace_only =
                !is_app && !directly_available && workspace_names.contains(&package_name);
            if is_app || directly_available || workspace_only {
                source_packages.push(SourcePackage {
                    package_name: package.name.to_string(),
                    crate_name,
                    manifest_path: manifest,
                    source_root,
                    workspace_only,
                });
            }
        }
        add_missing_workspace_sources(&mut source_packages, &workspace_dependencies);

        Ok(Self {
            opened_path,
            app_manifest,
            workspace_manifest,
            package_name: app_package.name.to_string(),
            config_path: discover_config_path(&app_dir),
            source_packages,
            direct_dependencies,
            workspace_dependencies,
            metadata_warning: None,
        })
    }

    fn from_manifests(
        opened_path: PathBuf,
        app_manifest: PathBuf,
        metadata_warning: Option<String>,
    ) -> Result<Self, ProjectError> {
        let app_text = read(&app_manifest)?;
        let app_doc = parse_manifest(&app_manifest, &app_text)?;
        let package_name = app_doc["package"]["name"]
            .as_str()
            .unwrap_or("copper_app")
            .to_string();
        let workspace_manifest = find_workspace_manifest(&app_manifest);
        let workspace_dependencies = workspace_dependency_paths(&workspace_manifest)?;
        let direct_dependencies = dependency_keys(&app_doc);
        let app_dir = app_manifest.parent().unwrap_or(Path::new(".")).to_path_buf();
        let mut source_packages = vec![SourcePackage {
            package_name: package_name.clone(),
            crate_name: package_name.replace('-', "_"),
            manifest_path: app_manifest.clone(),
            source_root: app_dir.join("src"),
            workspace_only: false,
        }];

        for (dependency, path) in &workspace_dependencies {
            if direct_dependencies.contains(dependency) || path.exists() {
                source_packages.push(SourcePackage {
                    package_name: dependency.clone(),
                    crate_name: dependency.replace('-', "_"),
                    manifest_path: path.join("Cargo.toml"),
                    source_root: path.join("src"),
                    workspace_only: !direct_dependencies.contains(dependency),
                });
            }
        }

        // Direct path dependencies that are not inherited from the workspace.
        if let Some(table) = app_doc.get("dependencies").and_then(Item::as_table_like) {
            for (name, item) in table.iter() {
                let Some(relative) = item.get("path").and_then(Item::as_str) else {
                    continue;
                };
                let root = app_dir.join(relative);
                source_packages.push(SourcePackage {
                    package_name: name.to_string(),
                    crate_name: name.replace('-', "_"),
                    manifest_path: root.join("Cargo.toml"),
                    source_root: root.join("src"),
                    workspace_only: false,
                });
            }
        }

        Ok(Self {
            opened_path,
            app_manifest,
            workspace_manifest,
            package_name,
            config_path: discover_config_path(&app_dir),
            source_packages,
            direct_dependencies,
            workspace_dependencies,
            metadata_warning,
        })
    }

    pub fn add_workspace_dependencies(&self, dependencies: &HashSet<String>) -> Result<(), String> {
        if dependencies.is_empty() {
            return Ok(());
        }
        let source = fs::read_to_string(&self.app_manifest).map_err(|error| error.to_string())?;
        let mut document = source
            .parse::<DocumentMut>()
            .map_err(|error| error.to_string())?;
        if !document.contains_key("dependencies") {
            document["dependencies"] = Item::Table(Default::default());
        }
        for dependency in dependencies {
            if document["dependencies"].get(dependency).is_none() {
                document["dependencies"][dependency]["workspace"] = toml_edit::value(true);
            }
        }
        atomic_write(&self.app_manifest, document.to_string().as_bytes())
            .map_err(|error| error.to_string())
    }
}

fn find_manifest(path: &Path) -> Result<PathBuf, ProjectError> {
    let start = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };
    let direct = start.join("Cargo.toml");
    if direct.is_file() {
        return Ok(direct);
    }
    for ancestor in start.ancestors().skip(1) {
        let manifest = ancestor.join("Cargo.toml");
        if manifest.is_file() {
            return Ok(manifest);
        }
    }
    Err(ProjectError::ManifestNotFound(path.to_path_buf()))
}

fn choose_app_package<'a>(metadata: &'a Metadata, requested: &Path) -> Option<&'a Package> {
    if let Some(package) = metadata
        .packages
        .iter()
        .find(|package| package.manifest_path.as_std_path() == requested)
    {
        return Some(package);
    }
    metadata
        .workspace_packages()
        .into_iter()
        .find(|package| {
            let root = package.manifest_path.parent().unwrap();
            root.join("copperconfig.ron").is_file()
        })
        .or_else(|| metadata.root_package())
}

fn discover_config_path(app_dir: &Path) -> PathBuf {
    let default = app_dir.join("copperconfig.ron");
    if default.exists() {
        return default;
    }
    for entry in walkdir::WalkDir::new(app_dir.join("src"))
        .max_depth(4)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| entry.path().extension().is_some_and(|ext| ext == "rs"))
    {
        let Ok(source) = fs::read_to_string(entry.path()) else {
            continue;
        };
        if let Some(path) = extract_config_attribute(&source) {
            return app_dir.join(path);
        }
    }
    default
}

fn extract_config_attribute(source: &str) -> Option<&str> {
    let marker = "config";
    let mut remaining = source;
    while let Some(index) = remaining.find(marker) {
        remaining = &remaining[index + marker.len()..];
        let trimmed = remaining.trim_start();
        let Some(after_equal) = trimmed.strip_prefix('=') else {
            continue;
        };
        let after_equal = after_equal.trim_start();
        let Some(after_quote) = after_equal.strip_prefix('"') else {
            continue;
        };
        if let Some(end) = after_quote.find('"') {
            return Some(&after_quote[..end]);
        }
    }
    None
}

fn find_workspace_manifest(app_manifest: &Path) -> PathBuf {
    let mut selected = app_manifest.to_path_buf();
    for ancestor in app_manifest.parent().into_iter().flat_map(Path::ancestors) {
        let candidate = ancestor.join("Cargo.toml");
        if let Ok(source) = fs::read_to_string(&candidate)
            && source.contains("[workspace]")
        {
            selected = candidate;
        }
    }
    selected
}

fn workspace_dependency_paths(manifest: &Path) -> Result<BTreeMap<String, PathBuf>, ProjectError> {
    let source = read(manifest)?;
    let document = parse_manifest(manifest, &source)?;
    let mut dependencies = BTreeMap::new();
    let root = manifest.parent().unwrap_or(Path::new("."));
    if let Some(table) = document
        .get("workspace")
        .and_then(|workspace| workspace.get("dependencies"))
        .and_then(Item::as_table_like)
    {
        for (name, item) in table.iter() {
            if let Some(path) = item.get("path").and_then(Item::as_str) {
                dependencies.insert(name.to_string(), root.join(path));
            }
        }
    }
    Ok(dependencies)
}

fn add_missing_workspace_sources(
    packages: &mut Vec<SourcePackage>,
    workspace_dependencies: &BTreeMap<String, PathBuf>,
) {
    for (name, root) in workspace_dependencies {
        if packages.iter().any(|package| package.package_name == *name) {
            continue;
        }
        packages.push(SourcePackage {
            package_name: name.clone(),
            crate_name: name.replace('-', "_"),
            manifest_path: root.join("Cargo.toml"),
            source_root: root.join("src"),
            workspace_only: true,
        });
    }
}

fn dependency_keys(document: &DocumentMut) -> HashSet<String> {
    document
        .get("dependencies")
        .and_then(Item::as_table_like)
        .map(|table| table.iter().map(|(name, _)| name.to_string()).collect())
        .unwrap_or_default()
}

fn read(path: &Path) -> Result<String, ProjectError> {
    fs::read_to_string(path).map_err(|source| ProjectError::Read {
        path: path.to_path_buf(),
        source,
    })
}

fn parse_manifest(path: &Path, source: &str) -> Result<DocumentMut, ProjectError> {
    source.parse().map_err(|error: toml_edit::TomlError| ProjectError::Manifest {
        path: path.to_path_buf(),
        message: error.to_string(),
    })
}

pub fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let parent = path.parent().unwrap_or(Path::new("."));
    fs::create_dir_all(parent)?;
    let mut temporary = tempfile::NamedTempFile::new_in(parent)?;
    use std::io::Write;
    temporary.write_all(bytes)?;
    temporary.as_file().sync_all()?;
    temporary.persist(path).map_err(|error| error.error)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_runtime_config_path() {
        let source = r#"#[copper_runtime(config = "config/robot.ron", sim_mode = true)]"#;
        assert_eq!(extract_config_attribute(source), Some("config/robot.ron"));
    }

    #[test]
    fn adds_missing_workspace_dependency_without_replacing_manifest() {
        let temp = tempfile::tempdir().unwrap();
        let manifest = temp.path().join("Cargo.toml");
        fs::write(
            &manifest,
            "# keep me\n[package]\nname = \"app\"\nversion = \"0.1.0\"\n\n[dependencies]\nserde = \"1\"\n",
        )
        .unwrap();
        let project = ProjectInfo {
            opened_path: temp.path().to_path_buf(),
            app_manifest: manifest.clone(),
            workspace_manifest: manifest.clone(),
            package_name: "app".into(),
            config_path: temp.path().join("copperconfig.ron"),
            source_packages: Vec::new(),
            direct_dependencies: HashSet::new(),
            workspace_dependencies: BTreeMap::new(),
            metadata_warning: None,
        };
        project
            .add_workspace_dependencies(&HashSet::from(["robot-tasks".to_string()]))
            .unwrap();
        let source = fs::read_to_string(manifest).unwrap();
        assert!(source.contains("# keep me"));
        let document = source.parse::<DocumentMut>().unwrap();
        assert_eq!(
            document["dependencies"]["robot-tasks"]["workspace"].as_bool(),
            Some(true)
        );
    }
}
