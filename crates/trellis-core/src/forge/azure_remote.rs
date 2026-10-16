//! Azure DevOps remote URLs and host names: which organization, project and repository a remote
//! names, and the host its credentials are stored under.

use super::{Account, Provider, Target, dec};

/// Recognises dev.azure.com, *.visualstudio.com and SSH remote URLs.
pub fn parse_remote(url: &str) -> Option<Target> {
    let url = url.trim().trim_end_matches('/');
    let url = url.strip_suffix(".git").unwrap_or(url);
    let target = |host: String, org: &str, project: &str, repo: &str| Target {
        provider: Provider::AzureDevOps,
        host,
        owner: dec(org),
        project: dec(project),
        repo: dec(repo),
    };

    // SSH: git@ssh.dev.azure.com:v3/org/project/repo or org@vs-ssh.visualstudio.com:v3/...
    if url.contains("ssh.dev.azure.com") || url.contains("vs-ssh.visualstudio.com") {
        let (_, path) = url.split_once("v3/")?;
        let mut segs = path.split('/');
        let (org, project, repo) = (segs.next()?, segs.next()?, segs.next()?);
        let host = if url.contains("visualstudio.com") { format!("{}.visualstudio.com", dec(org)) } else { "dev.azure.com".into() };
        return Some(target(host, org, project, repo));
    }

    let rest = url.strip_prefix("https://").or_else(|| url.strip_prefix("http://"))?;
    let (authority, path) = rest.split_once('/')?;
    let host = authority.rsplit('@').next()?.to_ascii_lowercase();
    let segs: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let git = segs.iter().position(|s| *s == "_git")?;
    let repo = *segs.get(git + 1)?;
    let before: Vec<&str> = segs[..git].iter().copied().filter(|s| !s.eq_ignore_ascii_case("DefaultCollection")).collect();

    if host == "dev.azure.com" {
        let org = *before.first()?;
        // A repo named after its project can omit the project segment.
        let project = before.get(1).copied().unwrap_or(repo);
        Some(target(host, org, project, repo))
    } else if let Some(org) = host.strip_suffix(".visualstudio.com") {
        let org = org.to_owned();
        let project = before.last().copied().unwrap_or(repo);
        Some(target(host, &org, project, repo))
    } else {
        None
    }
}

/// "dev.azure.com/mycompany", a full URL, or "mycompany.visualstudio.com" → (host, org).
pub fn parse_host_domain(text: &str) -> Option<(String, String)> {
    let t = text.trim();
    let t = t.strip_prefix("https://").or_else(|| t.strip_prefix("http://")).unwrap_or(t);
    let mut parts = t.split('/').filter(|s| !s.is_empty());
    let host = parts.next()?.rsplit('@').next()?.to_ascii_lowercase();
    if host == "dev.azure.com" {
        Some((host, dec(parts.next()?)))
    } else if let Some(org) = host.strip_suffix(".visualstudio.com") {
        (!org.is_empty()).then(|| (host.clone(), org.to_owned()))
    } else {
        None
    }
}

pub fn credential_host(account: &Account) -> String {
    account.host.clone()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_azure_remotes() {
        let p = |u: &str| parse_remote(u).map(|t| (t.host, t.owner, t.project, t.repo));
        let s = |a: &str, b: &str, c: &str, d: &str| Some((a.to_owned(), b.to_owned(), c.to_owned(), d.to_owned()));
        assert_eq!(
            p("https://contoso@dev.azure.com/contoso/Fabrikam%20Web/_git/web-app"),
            s("dev.azure.com", "contoso", "Fabrikam Web", "web-app")
        );
        assert_eq!(p("https://dev.azure.com/contoso/Solo/_git/Solo/"), s("dev.azure.com", "contoso", "Solo", "Solo"));
        assert_eq!(
            p("https://contoso.visualstudio.com/DefaultCollection/Fabrikam/_git/api"),
            s("contoso.visualstudio.com", "contoso", "Fabrikam", "api")
        );
        assert_eq!(p("https://contoso.visualstudio.com/_git/Solo"), s("contoso.visualstudio.com", "contoso", "Solo", "Solo"));
        assert_eq!(p("git@ssh.dev.azure.com:v3/contoso/Fabrikam/api"), s("dev.azure.com", "contoso", "Fabrikam", "api"));
        assert_eq!(
            p("contoso@vs-ssh.visualstudio.com:v3/contoso/Fabrikam/api"),
            s("contoso.visualstudio.com", "contoso", "Fabrikam", "api")
        );
        assert_eq!(p("https://github.com/owner/repo.git"), None);
        assert_eq!(parse_host_domain("https://user@dev.azure.com/my%20co/x"), Some(("dev.azure.com".into(), "my co".into())));
        assert_eq!(parse_host_domain("dev.azure.com"), None);
    }
}
