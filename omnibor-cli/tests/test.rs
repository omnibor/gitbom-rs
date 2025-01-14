use insta::Settings;
use insta_cmd::{assert_cmd_snapshot, get_cargo_bin};
use std::process::Command;

macro_rules! settings {
    ($block:expr) => {
        let mut settings = Settings::clone_current();
        settings.add_filter(r#"omnibor(?:\.exe)?"#, "omnibor");
        settings.bind(|| $block);
    };
}

#[test]
fn no_args() {
    settings!({
        assert_cmd_snapshot!(Command::new(get_cargo_bin("omnibor")));
    });
}

#[test]
fn artifact_no_args() {
    settings!({
        assert_cmd_snapshot!(Command::new(get_cargo_bin("omnibor")).arg("artifact"));
    });
}

#[test]
fn manifest_no_args() {
    settings!({
        assert_cmd_snapshot!(Command::new(get_cargo_bin("omnibor")).arg("manifest"));
    });
}

#[test]
fn debug_no_args() {
    settings!({
        assert_cmd_snapshot!(Command::new(get_cargo_bin("omnibor")).arg("debug"));
    });
}

#[test]
fn artifact_id_plain() {
    settings!({
        assert_cmd_snapshot!(Command::new(get_cargo_bin("omnibor")).args([
            "artifact",
            "id",
            "--format",
            "plain",
            "--path",
            "tests/data/main.c"
        ]))
    });
}

#[test]
fn artifact_id_short() {
    settings!({
        assert_cmd_snapshot!(Command::new(get_cargo_bin("omnibor")).args([
            "artifact",
            "id",
            "--format",
            "short",
            "--path",
            "tests/data/main.c"
        ]))
    });
}

#[test]
fn artifact_id_json() {
    settings!({
        assert_cmd_snapshot!(Command::new(get_cargo_bin("omnibor")).args([
            "artifact",
            "id",
            "--format",
            "json",
            "--path",
            "tests/data/main.c"
        ]))
    });
}
