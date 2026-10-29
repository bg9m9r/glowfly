//! The Release workflow (.github/workflows/release.yml) and release-please's config, read line by
//! line: a release is created as a draft with its tag pushed at once, every job that uploads a
//! file runs before the one job that publishes it, and the `release` environment's approval
//! stays on the prepare job alone. With release immutability on, a published release's files
//! can't change, so nothing may upload after publishing (docs/RELEASING.md).

use std::path::Path;

fn workspace_file(rel: &str) -> String {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel);
    // A Windows checkout may have CRLF line endings.
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {}: {e}", path.display())).replace("\r\n", "\n")
}

/// The workflow's jobs in order, as (name, the job's lines).
fn jobs(workflow: &str) -> Vec<(String, String)> {
    let body = workflow.split_once("\njobs:\n").map(|(_, b)| b).expect("a jobs: key");
    let mut out: Vec<(String, String)> = Vec::new();
    for line in body.lines() {
        let job = line.strip_prefix("  ").filter(|l| !l.starts_with(' ') && !l.starts_with('#'));
        match job.and_then(|l| l.strip_suffix(':')) {
            Some(name) => out.push((name.to_owned(), String::new())),
            None => {
                if let Some((_, text)) = out.last_mut() {
                    text.push_str(line);
                    text.push('\n');
                }
            }
        }
    }
    out
}

/// The job's `needs:` list.
fn needs(job: &str) -> Vec<String> {
    let line = job.lines().find_map(|l| l.trim().strip_prefix("needs: ")).unwrap_or("");
    line.trim_matches(|c| c == '[' || c == ']').split(',').map(|n| n.trim().to_owned()).filter(|n| !n.is_empty()).collect()
}

#[test]
fn release_please_creates_a_draft_and_pushes_its_tag() {
    let config = workspace_file("release-please-config.json");
    let lines: Vec<&str> = config.lines().map(str::trim).collect();
    assert!(lines.contains(&"\"draft\": true,"), "release-please creates the release as a draft");
    // Without the tag, the next release pull request can't find this release.
    assert!(lines.contains(&"\"force-tag-creation\": true,"), "the draft's tag is pushed at once");
    assert!(!config.contains("\"prerelease\""), "a 0.x version would make every release a pre-release");
}

#[test]
fn every_upload_comes_before_the_one_publish() {
    let workflow = workspace_file(".github/workflows/release.yml");
    let jobs = jobs(&workflow);
    let names: Vec<&str> = jobs.iter().map(|(n, _)| n.as_str()).collect();
    assert_eq!(names, ["release-please", "prepare", "build", "flatpak", "checksums", "manifests", "publish"]);
    let (_, publish) = jobs.last().unwrap();
    assert_eq!(needs(publish), ["prepare", "build", "flatpak", "checksums", "manifests"]);
    for job in ["build", "flatpak", "checksums", "manifests"] {
        assert!(publish.contains(&format!("needs.{job}.result == 'success'")), "publish waits for {job} to succeed");
    }
    assert!(publish.contains("needs.prepare.outputs.draft == 'true'"), "a release published before the run is left as it is");
    assert!(publish.contains("bash scripts/release-publish.sh \"$TAG\" \"$RELEASE_ID\""));
    for (name, job) in &jobs {
        if job.contains("gh release upload") {
            assert!(needs(publish).contains(name), "{name} uploads, so publish must need it");
        }
        // Only the scripts publish or create releases, so nothing goes out before its files.
        for call in ["gh release create", "--draft=false", "draft=false"] {
            assert!(name == "publish" || !job.contains(call), "{name} has {call}");
        }
    }
}

#[test]
fn the_approval_stays_on_prepare_which_finds_the_draft() {
    let workflow = workspace_file(".github/workflows/release.yml");
    let jobs = jobs(&workflow);
    let gated: Vec<&str> = jobs.iter().filter(|(_, j)| j.contains("\n    environment:")).map(|(n, _)| n.as_str()).collect();
    assert_eq!(gated, ["prepare"], "one approval, before anything is built or published");
    let (_, prepare) = jobs.iter().find(|(n, _)| n == "prepare").unwrap();
    assert!(prepare.contains("      name: release\n"));
    let draft = prepare.find("bash scripts/release-draft.sh \"$TAG\"").expect("prepare finds or creates the draft");
    let header = prepare.find("bash scripts/release-notes-header.sh \"$TAG\"").expect("prepare puts the header on");
    assert!(draft < header, "the header goes on the draft");
    for out in ["tag:", "release-id:", "draft:"] {
        assert!(prepare.contains(&format!("      {out} ${{{{ steps.")), "prepare outputs {out}");
    }
    for (name, job) in &jobs {
        if name != "release-please" && name != "prepare" {
            assert!(needs(job).first().map(String::as_str) == Some("prepare"), "{name} needs prepare");
        }
    }
}

#[test]
fn the_publish_script_marks_pre_releases_and_checks_the_files() {
    let script = workspace_file("scripts/release-publish.sh");
    assert!(script.contains(
        "if [[ \"$tag\" =~ ^v[0-9]+\\.[0-9]+\\.[0-9]+$ ]]; then\n  prerelease=false latest=true\nelse\n  prerelease=true latest=false\n"
    ));
    assert!(script.contains("-F draft=false -F prerelease=\"$prerelease\""));
    assert!(script.contains("-f make_latest=\"$latest\""));
    assert!(script.contains("check \"$tag\" \"$work/sums/SHA256SUMS\" \"$work/assets\"\n"), "the files are checked before publishing");
    // The manifests use the same test for a pre-release.
    let manifests = workspace_file("scripts/fill-manifests.sh");
    assert!(manifests.contains("if ! [[ \"$tag\" =~ ^v[0-9]+\\.[0-9]+\\.[0-9]+$ ]]; then"));
}
