use super::*;

#[test]
fn deepen_cmds() {
    assert_eq!(deepen_cmd("origin", Deepen::Full), ["fetch", "--unshallow", "--", "origin"]);
    assert_eq!(deepen_cmd("up", Deepen::By(25)), ["fetch", "--deepen=25", "--", "up"]);
    assert_eq!(Deepen::Full.job_label(), "Fetch full history");
    assert_eq!(Deepen::By(1).job_label(), "Fetch 1 more commit");
    assert_eq!(Deepen::By(5).job_label(), "Fetch 5 more commits");
}
