#![cfg(test)]
//! The egress proxy against a real daemon: from inside a started proxy, a listed name never resolves
//! to the proxy itself.
//!
//! The oracle is docker's own resolver, asked from inside the container.

use std::process::Command;

use candler_core::isolation::gateway::egress_create_args;
use candler_core::isolation::model::NetworkName;
use candler_core::resolve::steps::egress::settle;
use candler_schemas::primitives::JobId;
use serde_json::json;

fn docker(args: &[&str]) -> String {
    let out = Command::new("docker")
        .args(args)
        .output()
        .expect("docker runs");
    assert!(
        out.status.success(),
        "docker {args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().to_owned()
}

/// Guards: a listed name never resolves to the proxy's own address, even when nothing on
/// the egress network answers to it. Oracle: docker's resolver at 127.0.0.11 asked from inside the
/// proxy, against the address `docker inspect` gives the proxy.
#[test]
#[ignore = "needs Docker; run with --run-ignored only"]
fn egress_proxy_never_resolves_a_listed_host_to_itself() {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets/docker/gateway");
    let image = docker(&["build", "-q", root]);
    let job: JobId = "01939d6c-1234-7abc-8def-0123456789ab"
        .parse()
        .expect("a job");
    let list = settle(Some(&json!(["listed.test:443"])), None)
        .expect("a list")
        .expect("not empty");
    let allow = NetworkName::allow_for(&job, &list);
    let egress = NetworkName::egress_for_job(&job);
    let digest = "sha256:0000000000000000000000000000000000000000000000000000000000000001"
        .parse()
        .expect("a digest");
    let nets = [
        docker(&["network", "create", "--internal", allow.as_str()]),
        docker(&["network", "create", egress.as_str()]),
    ];
    let mut create = vec![String::from("create")];
    create.extend(egress_create_args(&image, &job, &digest, &list, &allow));
    let proxy = docker(&create.iter().map(String::as_str).collect::<Vec<_>>());
    docker(&["network", "connect", egress.as_str(), &proxy]);
    docker(&["start", &proxy]);
    let ip_in = |cid: &str, net: &str| {
        docker(&[
            "inspect",
            "-f",
            &format!("{{{{(index .NetworkSettings.Networks \"{net}\").IPAddress}}}}"),
            cid,
        ])
    };
    let own = ip_in(&proxy, allow.as_str());
    std::thread::sleep(std::time::Duration::from_secs(2));
    // nslookup exits non-zero for a name nobody answers, which is the wanted outcome
    let looked = Command::new("docker")
        .args(["exec", &proxy, "nslookup", "listed.test", "127.0.0.11"])
        .output()
        .expect("docker runs");
    let answer = String::from_utf8_lossy(&looked.stdout).into_owned();
    let found: Vec<&str> = answer
        .lines()
        .filter_map(|l| l.strip_prefix("Address"))
        .filter_map(|l| l.split(':').next_back())
        .map(str::trim)
        .collect();
    let cleanup = || {
        let _ = Command::new("docker").args(["rm", "-f", &proxy]).output();
        for id in &nets {
            let _ = Command::new("docker").args(["network", "rm", id]).output();
        }
        let _ = Command::new("docker").args(["rmi", &image]).output();
    };
    let ok = !found.contains(&own.as_str());
    cleanup();
    assert!(ok, "own={own} answer={answer}");
}
