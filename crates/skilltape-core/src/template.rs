use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use thiserror::Error;

const TEMPLATE_FILES: [&str; 6] = [
    "skilltape.yaml",
    "workflow.yaml",
    "permissions.json",
    "skilltape.lock",
    "SKILL.md",
    "README.md",
];

#[derive(Debug, Error)]
pub enum TemplateError {
    #[error(
        "invalid skill name: names must be non-empty, unpadded, and contain no path separators"
    )]
    InvalidName,
    #[error("template target already exists: {path}")]
    TargetExists { path: PathBuf },
    #[error("failed to create template path {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

pub fn create_skill_template(root: &Path, name: &str) -> Result<(), TemplateError> {
    validate_name(name)?;

    fs::create_dir_all(root).map_err(|source| TemplateError::Io {
        path: root.to_path_buf(),
        source,
    })?;

    for file in TEMPLATE_FILES {
        let path = root.join(file);
        if fs::symlink_metadata(&path).is_ok() {
            return Err(TemplateError::TargetExists { path });
        }
    }

    let quoted_name = serde_json::to_string(name).expect("serializing a string cannot fail");
    let files = [
        ("skilltape.yaml", manifest(&quoted_name)),
        ("workflow.yaml", WORKFLOW.to_owned()),
        ("permissions.json", PERMISSIONS.to_owned()),
        ("skilltape.lock", LOCKFILE.to_owned()),
        (
            "SKILL.md",
            format!("# {name}\n\nDescribe how agents should use this skill.\n"),
        ),
        (
            "README.md",
            format!("# {name}\n\nA SkillTape skill package.\n"),
        ),
    ];

    for (file, contents) in files {
        write_new_file(root.join(file), contents.as_bytes())?;
    }

    Ok(())
}

fn validate_name(name: &str) -> Result<(), TemplateError> {
    if name.is_empty() || name.trim() != name || name.contains('/') || name.contains('\\') {
        return Err(TemplateError::InvalidName);
    }

    Ok(())
}

fn write_new_file(path: PathBuf, contents: &[u8]) -> Result<(), TemplateError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .map_err(|source| match source.kind() {
            std::io::ErrorKind::AlreadyExists => TemplateError::TargetExists { path: path.clone() },
            _ => TemplateError::Io {
                path: path.clone(),
                source,
            },
        })?;

    file.write_all(contents)
        .map_err(|source| TemplateError::Io { path, source })
}

fn manifest(quoted_name: &str) -> String {
    format!(
        "schema: skilltape.dev/skill/v1\nname: {quoted_name}\nversion: 0.1.0\ndescription: A minimal SkillTape package.\nengine:\n  min_version: 0.1.0\nentrypoint:\n  workflow: workflow.yaml\n  permissions: permissions.json\n  lockfile: skilltape.lock\ninputs: []\noutputs: []\ntargets:\n  - generic-agent-skill\n"
    )
}

const WORKFLOW: &str = "schema: skilltape.dev/workflow/v1\nsteps: []\n";

const PERMISSIONS: &str = r#"{
  "schema": "skilltape.dev/permissions/v1",
  "filesystem": {
    "read": [],
    "write": []
  },
  "process": {
    "executables": [],
    "max_processes": 1,
    "default_timeout_ms": 120000
  },
  "network": {
    "enabled": false,
    "allow_hosts": []
  },
  "secrets": {
    "read_environment": false
  }
}
"#;

const LOCKFILE: &str = r#"{
  "schema": "skilltape.dev/lock/v1",
  "engine": {
    "version": "0.1.0"
  },
  "tools": [],
  "scripts": []
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    struct TestDir(TempDir);

    impl TestDir {
        fn new(label: &str) -> Self {
            let prefix = format!("skilltape-template-{label}-");
            Self(
                tempfile::Builder::new()
                    .prefix(&prefix)
                    .tempdir()
                    .expect("test directory"),
            )
        }

        fn join(&self, path: impl AsRef<Path>) -> PathBuf {
            self.0.path().join(path)
        }
    }

    #[test]
    fn rejects_invalid_names_before_creating_the_target() {
        for name in ["", " padded", "padded ", "nested/name", "nested\\name"] {
            let temp = TestDir::new("invalid-name");
            let root = temp.join("skill");

            assert!(matches!(
                create_skill_template(&root, name),
                Err(TemplateError::InvalidName)
            ));
            assert!(!root.exists());
        }
    }

    #[test]
    fn creates_six_deterministic_lintable_files() {
        let first = TestDir::new("deterministic-first");
        let second = TestDir::new("deterministic-second");
        let first_root = first.join("example-skill");
        let second_root = second.join("example-skill");

        create_skill_template(&first_root, "example-skill").expect("first template");
        create_skill_template(&second_root, "example-skill").expect("second template");

        let mut generated = fs::read_dir(&first_root)
            .expect("read template")
            .map(|entry| entry.expect("entry").file_name())
            .collect::<Vec<_>>();
        generated.sort();
        let mut expected = TEMPLATE_FILES
            .iter()
            .map(std::ffi::OsString::from)
            .collect::<Vec<_>>();
        expected.sort();
        assert_eq!(generated, expected);

        for file in TEMPLATE_FILES {
            assert_eq!(
                fs::read(first_root.join(file)).expect("first file"),
                fs::read(second_root.join(file)).expect("second file")
            );
        }

        let loaded = crate::SkillPackage::load(&first_root).expect("load generated template");
        let report = loaded.lint(false);
        assert!(
            report.errors.is_empty(),
            "unexpected errors: {:?}",
            report.errors
        );
        assert!(
            report.warnings.is_empty(),
            "unexpected warnings: {:?}",
            report.warnings
        );
    }

    #[test]
    fn never_overwrites_an_existing_required_file() {
        let temp = TestDir::new("existing");
        let root = temp.join("existing");
        fs::create_dir_all(&root).expect("root");
        fs::write(root.join("README.md"), "keep me").expect("existing file");

        assert!(matches!(
            create_skill_template(&root, "existing"),
            Err(TemplateError::TargetExists { .. })
        ));
        assert_eq!(
            fs::read_to_string(root.join("README.md")).expect("existing file"),
            "keep me"
        );
        assert_eq!(fs::read_dir(&root).expect("read root").count(), 1);
    }
}
