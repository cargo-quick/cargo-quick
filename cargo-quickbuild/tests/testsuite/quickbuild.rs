//! Tests for the `cargo quick build` command.

use std::path::{Path, PathBuf};

use cargo_test_macro::cargo_test;
use cargo_test_support::registry::{self, Package};
use cargo_test_support::{project, Project};

fn make_simple_proj() -> Project {
    let quickbuild_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    std::os::unix::fs::symlink(
        std::env::var("CARGO").unwrap(),
        quickbuild_dir.join("../target/debug/cargo"),
    )
    .unwrap_or_else(|e| match e.kind() {
        std::io::ErrorKind::AlreadyExists => (),
        _ => panic!("{}", e),
    });

    Package::new("c", "1.0.0").publish();
    Package::new("b", "1.0.0").dep("c", "1.0").publish();
    Package::new("a", "1.0.0").dep("b", "1.0").publish();
    Package::new("bdep", "1.0.0").dep("b", "1.0").publish();
    Package::new("devdep", "1.0.0").dep("b", "1.0.0").publish();

    project()
        .file(
            "Cargo.toml",
            r#"
            [package]
            name = "foo"
            version = "0.1.0"

            [dependencies]
            a = "1.0"
            c = "1.0"

            [build-dependencies]
            bdep = "1.0"

            [dev-dependencies]
            devdep = "1.0"
            "#,
        )
        .file("src/lib.rs", "")
        .file("build.rs", "fn main() {}")
        .build()
}

#[cargo_test]
fn simple() {
    let registry = registry::init();

    // A simple test with a few different dependencies.
    let p = make_simple_proj();

    dbg!(p.cargo("tree").with_stdout(
        "\
foo v0.1.0 ([..]/foo)
├── a v1.0.0
│   └── b v1.0.0
│       └── c v1.0.0
└── c v1.0.0
[build-dependencies]
└── bdep v1.0.0
    └── b v1.0.0 (*)
[dev-dependencies]
└── devdep v1.0.0
    └── b v1.0.0 (*)
",
    ))
    .run();

    p.cargo("tree -p bdep")
        .with_stdout(
            "\
bdep v1.0.0
└── b v1.0.0
    └── c v1.0.0
",
        )
        .run();
}
