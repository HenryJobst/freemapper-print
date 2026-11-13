//! Guard for `examples/`: every example project loads, its inputs exist, and the map credit is there.
//!
//! The examples are built on maps by Remo Madella (ADR 0064). A folder that loses its map, its course file or
//! its `ATTRIBUTION.txt` should fail here, not on a reader's machine.

use std::path::{Path, PathBuf};

use freemapper_print::project::{self, FILE_NAME};

fn examples_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples")
}

/// Every folder under `dir` that holds a project file, at most one level below an example folder.
fn project_dirs(dir: &Path, found: &mut Vec<PathBuf>) {
    if dir.join(FILE_NAME).is_file() {
        found.push(dir.to_path_buf());
    }
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        // `out/` holds PDFs and `logos/` images; only `iof/` has a project of its own.
        if path.is_dir() && path.file_name().is_some_and(|n| n == "iof") {
            project_dirs(&path, found);
        }
    }
}

#[test]
fn every_example_loads_with_its_inputs_and_credit() {
    let mut projects = Vec::new();
    let mut examples = Vec::new();
    for entry in std::fs::read_dir(examples_dir()).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            examples.push(path.clone());
            project_dirs(&path, &mut projects);
        }
    }
    projects.sort();
    examples.sort();
    // The seven examples of ADR 0064, and the one extra project in `rummelsburg-competition-courses/iof/`.
    assert_eq!(examples.len(), 7, "examples: {examples:?}");
    assert_eq!(projects.len(), 8, "projects: {projects:?}");

    for example in &examples {
        let attribution = std::fs::read_to_string(example.join("ATTRIBUTION.txt"))
            .unwrap_or_else(|e| panic!("{}: ATTRIBUTION.txt: {e}", example.display()));
        assert!(
            attribution.contains("Remo Madella"),
            "{} names no map author",
            example.join("ATTRIBUTION.txt").display()
        );
    }

    for dir in &projects {
        let loaded = project::load(&dir.join(FILE_NAME))
            .unwrap_or_else(|e| panic!("{}: {e}", dir.display()));
        let map = loaded.map_path();
        assert!(
            map.is_file(),
            "{}: map {} is missing",
            dir.display(),
            map.display()
        );
        if let Some(courses) = &loaded.project.sources.courses {
            let courses = loaded.resolve(courses);
            assert!(
                courses.is_file(),
                "{}: course file {} is missing",
                dir.display(),
                courses.display()
            );
        }
        let layout_elements = loaded.project.layouts.iter().flat_map(|l| &l.elements);
        for element in loaded.project.shared.iter().chain(layout_elements) {
            if let Some(file) = &element.file {
                let file = loaded.resolve(file);
                assert!(
                    file.is_file(),
                    "{}: logo {} is missing",
                    dir.display(),
                    file.display()
                );
            }
        }
    }
}
