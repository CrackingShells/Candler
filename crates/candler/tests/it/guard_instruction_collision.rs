//! The workspace instruction text and a declared input that mount a file at one path are refused
//! when `candler run` plans the runs, before a container exists.
#![cfg(test)]
#![cfg(unix)]

use crate::support::cli_run;

use std::fs;

use cli_run::world;

/// Guards: an experiment that sets `instructions.workspace` and declares an input whose file lands
/// at the harness's workspace instruction file is refused by `candler run` with
/// `experiment.instructions_collide`, naming the input, with no container created; the file name
/// may differ from the harness's in its case, and an input at another path is no collision.
/// Oracle: the input kind `rules` and the file names `CLAUDE.md` and `claude.md` written by hand;
/// the fake docker's record of the containers it created.
/// Bite: leave the check out of planning, or compare the names exactly.
#[test]
fn input_at_the_instruction_file_is_refused_by_run_before_any_container() {
    // (the input's file name, whether the run is refused)
    let rows = [
        ("CLAUDE.md", true),
        ("claude.md", true),
        ("NOTES.md", false),
    ];
    for (name, refused) in rows {
        let world = world();
        let base = world
            .experiment
            .parent()
            .expect("a directory")
            .join("rules");
        fs::create_dir_all(&base).expect("the input directory");
        fs::write(base.join(name), "rules\n").expect("the input file");
        let mut text = fs::read_to_string(&world.experiment).expect("the experiment");
        text = text.replacen(
            "[fixed]\n",
            "[fixed]\n\"instructions.workspace\" = \"be brief\"\n",
            1,
        );
        text.push_str("\n[[inputs]]\nkind = \"rules\"\npath = \"rules\"\n");
        fs::write(&world.experiment, text).expect("the experiment writes");

        let out = world.launch();
        let stderr = String::from_utf8_lossy(&out.stderr).into_owned();
        let collided = stderr.contains("experiment.instructions_collide");
        assert_eq!(collided, refused, "{name}: stderr: {stderr}");
        if refused {
            assert_eq!(out.status.code(), Some(2), "{name}: stderr: {stderr}");
            assert!(
                stderr.contains("`rules`"),
                "{name}: the input is named: {stderr}"
            );
            assert_eq!(
                world.handed_out(),
                Vec::<String>::new(),
                "{name}: no container"
            );
        }
    }
}
