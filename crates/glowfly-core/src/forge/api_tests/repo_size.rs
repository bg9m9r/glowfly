use super::*;

#[test]
fn github_reports_kib() {
    let m = Mock::start(|r| match r.route().as_str() {
        "GET api.github.com/repos/o/r" => Reply::json(r#"{"size":2048,"name":"r"}"#),
        _ => Reply::not_found(),
    });
    let size = repo_size(&remote("https://github.com/o/r"), &token(Provider::GitHub)).unwrap();
    assert_eq!(size, Some(2 * 1024 * 1024));
    assert_eq!(find(&m, "GET api.github.com/repos/o/r").header("Authorization"), Some("Bearer tok"));
}

#[test]
fn gitlab_reports_its_statistics_when_allowed() {
    let _m = Mock::start(|r| match r.route().as_str() {
        "GET gitlab.com/api/v4/projects/o%2Fr?statistics=true" => Reply::json(r#"{"statistics":{"repository_size":12345}}"#),
        "GET gitlab.com/api/v4/projects/o%2Fq?statistics=true" => Reply::json(r#"{"id":1}"#),
        _ => Reply::not_found(),
    });
    let auth = token(Provider::GitLab);
    assert_eq!(repo_size(&remote("https://gitlab.com/o/r"), &auth).unwrap(), Some(12345));
    assert_eq!(repo_size(&remote("https://gitlab.com/o/q"), &auth).unwrap(), None, "no statistics below Reporter");
}

#[test]
fn bitbucket_cloud_and_azure_report_bytes() {
    let _m = Mock::start(|r| {
        let route = r.route();
        if route == "GET api.bitbucket.org/2.0/repositories/team/app" {
            Reply::json(r#"{"size":777}"#)
        } else if route.starts_with("GET dev.azure.com/contoso/Fabrikam%20Web/_apis/git/repositories/web%20app?api-version=") {
            Reply::json(r#"{"size":999,"name":"web app"}"#)
        } else {
            Reply::not_found()
        }
    });
    assert_eq!(repo_size(&remote("https://bitbucket.org/team/app"), &token(Provider::Bitbucket)).unwrap(), Some(777));
    let azure = remote("https://dev.azure.com/contoso/Fabrikam%20Web/_git/web%20app");
    assert_eq!(repo_size(&azure, &token(Provider::AzureDevOps)).unwrap(), Some(999));
}

#[test]
fn data_center_has_no_size_and_a_404_fails() {
    let m = Mock::start(|_| Reply::not_found());
    let dc = remote("https://bitbucket.acme.io/scm/PROJ/api.git");
    assert_eq!(dc.provider, Provider::BitbucketDataCenter);
    assert_eq!(repo_size(&dc, &token(Provider::BitbucketDataCenter)).unwrap(), None);
    assert!(routes(&m).is_empty(), "no request");
    assert!(repo_size(&remote("https://github.com/o/gone"), &token(Provider::GitHub)).unwrap_err().is_not_found());
}
