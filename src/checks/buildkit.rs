//! Size of BuildKit's cache: Docker's built-in BuildKit under
//! `<root>/var/lib/docker/buildkit`, and an external `buildkitd` under
//! `<root>/var/lib/buildkit`. No `docker builder prune` or other command
//! that changes anything.

use crate::{Finding, Root, Status};

use super::util::{dir_bytes, is_permission};

const CHECK: &str = "buildkit";
const HINT: &str = "BuildKit's cache is not counted by `docker system df` for an external \
buildkitd. Run `docker builder prune` or clear the buildkitd cache dir yourself if you need the \
space back. This tool only reports and does not change anything.";

pub(crate) fn check(root: &Root) -> Vec<Finding> {
    let docker_buildkit = dir_bytes(&root.join("/var/lib/docker/buildkit"));
    let external_buildkit = dir_bytes(&root.join("/var/lib/buildkit"));

    if matches!(&docker_buildkit, Err(err) if is_permission(err)) {
        return vec![needs_root("/var/lib/docker/buildkit")];
    }
    if matches!(&external_buildkit, Err(err) if is_permission(err)) {
        return vec![needs_root("/var/lib/buildkit")];
    }

    let docker_buildkit = match docker_buildkit {
        Ok(bytes) => bytes,
        Err(err) => {
            return vec![error_finding(format!(
                "cannot read /var/lib/docker/buildkit: {err}"
            ))]
        }
    };
    let external_buildkit = match external_buildkit {
        Ok(bytes) => bytes,
        Err(err) => {
            return vec![error_finding(format!(
                "cannot read /var/lib/buildkit: {err}"
            ))]
        }
    };

    if docker_buildkit.is_none() && external_buildkit.is_none() {
        return vec![not_present()];
    }

    let bytes = docker_buildkit.unwrap_or(0) + external_buildkit.unwrap_or(0);
    vec![Finding::new(
        CHECK,
        Status::Ok,
        Some(bytes),
        None,
        format!("/var/lib/docker/buildkit + /var/lib/buildkit use {bytes} bytes"),
        HINT,
    )]
}

fn not_present() -> Finding {
    Finding::new(
        CHECK,
        Status::NotPresent,
        None,
        None,
        "/var/lib/docker/buildkit and /var/lib/buildkit are not present",
        "No BuildKit cache directory was found.",
    )
}

fn needs_root(path: &str) -> Finding {
    Finding::new(
        CHECK,
        Status::NeedsRoot,
        None,
        None,
        format!("cannot read {path}"),
        format!("run with sudo to see the BuildKit cache (cannot read {path})"),
    )
}

fn error_finding(evidence: String) -> Finding {
    Finding::new(
        CHECK,
        Status::Error,
        None,
        None,
        evidence,
        "The check could not finish reading the BuildKit cache.",
    )
}
