#![cfg(test)]
//! Guards for the egress proxy's names and its rendered servers.
//!
//! The expected configs are written by hand from the rule: a port owned by one host forwards
//! everything to it, and an SNI, if sent, must equal the host; a port shared by several hosts
//! needs a matching SNI; anything else has an empty upstream and is closed.

use candler_core::isolation::gateway::render_egress;
use candler_core::isolation::model::NetworkName;
use candler_core::resolve::steps::egress::{HostPort, settle};
use candler_schemas::primitives::JobId;
use serde_json::json;

fn job(mid: &str) -> JobId {
    format!("01939d6c-{mid}-7abc-8def-0123456789ab")
        .parse()
        .expect("a job id")
}

fn list(entries: &[&str]) -> Vec<HostPort> {
    settle(Some(&json!(entries)), None)
        .expect("a list")
        .expect("not empty")
}

/// Guards: the allow network is the allowlist's own, per job — agents with equal allowlists meet
/// on one network, two allowlists never share one, no allow network is the egress network, and
/// `NetworkName::belongs_to` takes every network of the job and none of another job's.
/// Oracle: the name rule `candler-<job12>-allow-<digest8>` and the sha256 of `a.test:443\n`,
/// computed with `shasum -a 256` by hand: 754fac9b.
#[test]
fn plan_allow_network_names_follow_the_allowlist() {
    let j = job("1234");
    let a = NetworkName::allow_for(&j, &list(&["a.test:443"]));
    assert_eq!(a.as_str(), "candler-01939d6c1234-allow-754fac9b");
    assert_eq!(a, NetworkName::allow_for(&j, &list(&["a.test:443"])));
    assert_ne!(
        a,
        NetworkName::allow_for(&j, &list(&["a.test:443", "b.test:443"]))
    );
    assert_ne!(a, NetworkName::egress_for_job(&j));
    assert!(a.belongs_to(&j));
    assert!(!a.belongs_to(&job("9999")));
    // every network of the job is its own, and none of another job's is
    let other = job("9999");
    for own in [NetworkName::for_job(&j), NetworkName::egress_for_job(&j)] {
        assert!(own.belongs_to(&j), "{own:?}");
    }
    for theirs in [
        NetworkName::for_job(&other),
        NetworkName::egress_for_job(&other),
        NetworkName::allow_for(&other, &list(&["a.test:443"])),
    ] {
        assert!(!theirs.belongs_to(&j), "{theirs:?}");
    }
}

/// Guards: D2 — one host on a port gets everything, plain or with its own SNI.
/// Oracle: the rule, written by hand.
#[test]
fn plan_render_one_host_per_port() {
    let expected = "  map $ssl_preread_server_name $egress_up_443 {\n    hostnames;\n    default \"\";\n    \"\" a.test:443;\n    a.test a.test:443;\n  }\n  server {\n    listen 443;\n    ssl_preread on;\n    proxy_pass $egress_up_443;\n    proxy_connect_timeout 10s;\n    js_filter egress.guard;\n  }\n";
    assert_eq!(render_egress(&list(&["a.test:443"])), expected);
}

/// Guards: D2 — hosts sharing a port need a matching SNI; a plain connection is closed.
/// Oracle: the rule, written by hand.
#[test]
fn plan_render_shared_port_needs_sni() {
    let out = render_egress(&list(&["a.test:443", "b.test:443", "c.test:8443"]));
    assert!(out.contains(
        "$egress_up_443 {\n    hostnames;\n    default \"\";\n    a.test a.test:443;\n    b.test b.test:443;\n  }"
    ));
    assert!(!out.contains("\"\" a.test:443"));
    assert!(out.contains("\"\" c.test:8443;"));
    assert!(out.contains("listen 8443;"));
}

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use candler_core::isolation::docker::{ClientEnv, DockerCli, DockerError};
use candler_core::isolation::driver::DockerDriver;
use candler_core::isolation::gateway::{
    EgressProxy, EgressStart, Gateway, GatewayError, GatewaySignal, GatewayStart, GatewayStatus,
    SecretError, SecretSource, SecretValue,
};
use candler_core::isolation::model::{ContainerRef, GatewayHandle, Labels, SweepScope};
use candler_core::jobs::model::JobSpec;
use candler_core::jobs::orchestrate::session::{
    JobHost, JobSession, OrchestrateError, allowlists_of,
};
use candler_core::runner::Runtime;
use candler_core::runner::evidence::Inputs;
use candler_core::runner::exec::CellRun;
use candler_core::supervise::Supervisor;
use candler_schemas::primitives::RunKey;
use futures::future::BoxFuture;

use crate::support::job_orchestrate::{FakeGateway, Rig, spec};
use crate::support::run_plan::Facts;

const DIGEST: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000001";

/// A cell whose values are `egress` (when given) and `network` (when given).
fn cell(egress: Option<&[&str]>, network: Option<&str>) -> Arc<CellRun> {
    let mut values = Vec::new();
    if let Some(list) = egress {
        values.push(json!({"path": "isolation.egress", "value": list}));
    }
    if let Some(net) = network {
        values.push(json!({"path": "isolation.network", "value": net}));
    }
    let comparable = serde_json::from_value(json!({
        "values": values,
        "harness": {"name": "claude-code", "version": "2.1.311"},
        "treatment": {"name": "htn"},
        "snapshots": [],
        "inputs": [],
        "environments": [],
    }))
    .expect("a comparable");
    Arc::new(CellRun::new(
        comparable,
        DIGEST.parse().expect("a digest"),
        Vec::new(),
        Vec::new(),
        Vec::new(),
        None,
        Inputs::default(),
    ))
}

fn cells_of(
    specs: &[Option<&[&str]>],
) -> BTreeMap<candler_core::runner::plan::CellId, Arc<CellRun>> {
    specs
        .iter()
        .enumerate()
        .map(|(i, egress)| {
            // a distinct factor value keeps the digests of otherwise equal cells apart
            let digest = format!("sha256:{i:064x}").parse().expect("a digest");
            (digest, cell(*egress, None))
        })
        .collect()
}

fn names(lists: &[Vec<HostPort>]) -> Vec<Vec<String>> {
    lists
        .iter()
        .map(|l| l.iter().map(ToString::to_string).collect())
        .collect()
}

/// Guards: one proxy per distinct allowlist — equal allowlists share one, two allowlists give
/// two, and a cell without an allowlist gives none. Oracle: the leaf's plan rule, by hand.
#[test]
fn plan_one_proxy_per_distinct_allowlist() {
    let a: &[&str] = &["a.test:443"];
    let ab: &[&str] = &["b.test:443", "a.test:443"];
    assert_eq!(
        allowlists_of(&cells_of(&[None, None])),
        Vec::<Vec<HostPort>>::new()
    );
    assert_eq!(
        names(&allowlists_of(&cells_of(&[Some(a), Some(a), None]))),
        vec![vec!["a.test:443"]]
    );
    let two = names(&allowlists_of(&cells_of(&[
        Some(a),
        Some(ab),
        None,
        Some(a),
    ])));
    assert_eq!(
        two,
        vec![vec!["a.test:443"], vec!["a.test:443", "b.test:443"]]
    );
}

/// Guards: an allowlisted agent joins the internal network and its own allow network, never the
/// egress network; a cell without an allowlist joins none; two allowlists never share a network.
/// Oracle: the leaf's plan rule and the existing `bridge` meaning, by hand.
#[test]
fn plan_agent_networks() {
    let j = job("1234");
    let gw = GatewayHandle::new(
        ContainerRef::new(
            "0123456789ab".parse().expect("an id"),
            Labels::new(),
            "2026-11-06T10:00:00Z".parse().expect("a time"),
        ),
        "http://gateway:8080".parse().expect("a url"),
        NetworkName::for_job(&j),
        NetworkName::egress_for_job(&j),
        String::from("dummy"),
    )
    .expect("a handle");
    let a = list(&["a.test:443"]);
    let b = list(&["b.test:443"]);
    let (net, second) = gw.agent_networks(&j, Some(&a), None).expect("a plan");
    assert_eq!(net, NetworkName::for_job(&j));
    assert_eq!(second, Some(NetworkName::allow_for(&j, &a)));
    assert_ne!(second, Some(NetworkName::egress_for_job(&j)));
    let (_, other) = gw
        .agent_networks(&j, Some(&b), Some("internal"))
        .expect("a plan");
    assert_ne!(other, second);
    assert_eq!(gw.agent_networks(&j, None, None).expect("a plan").1, None);
    assert_eq!(
        gw.agent_networks(&j, None, Some("bridge"))
            .expect("a plan")
            .1,
        Some(NetworkName::egress_for_job(&j))
    );
    assert!(gw.agent_networks(&j, None, Some("host")).is_err());
}

/// The gateway fake that starts like [`FakeGateway`] and records each proxy it is asked for.
struct Recording {
    started: Mutex<Vec<Vec<String>>>,
}

impl Gateway for Recording {
    fn start<'a>(
        &'a self,
        sup: &'a Supervisor,
        docker: &'a dyn DockerDriver, // TRAIT_OBJECT: the driver is a fake in tests
        request: GatewayStart<'a>,
    ) -> BoxFuture<'a, Result<GatewayHandle, GatewayError>> {
        FakeGateway.start(sup, docker, request)
    }

    fn start_egress<'a>(
        &'a self,
        _sup: &'a Supervisor,
        _docker: &'a dyn DockerDriver, // TRAIT_OBJECT: the driver is a fake in tests
        request: EgressStart<'a>,
    ) -> BoxFuture<'a, Result<EgressProxy, GatewayError>> {
        let mut started = self.started.lock().expect("the log");
        started.push(request.allowlist.iter().map(ToString::to_string).collect());
        let n = started.len();
        let container = ContainerRef::new(
            format!("ba987654321{n}").parse().expect("an id"),
            Labels::new(),
            "2026-11-06T10:00:00Z".parse().expect("a time"),
        );
        let proxy = EgressProxy {
            container,
            network: NetworkName::allow_for(request.job, request.allowlist),
            ip: String::from("172.30.0.2"),
            allowlist: request.allowlist.to_vec(), // BORROW: the proxy owns its allowlist
        };
        Box::pin(async move { Ok(proxy) })
    }

    fn probe<'a>(&'a self, handle: &'a GatewayHandle) -> BoxFuture<'a, GatewayStatus> {
        FakeGateway.probe(handle)
    }

    fn run_signals<'a>(
        &'a self,
        handle: &'a GatewayHandle,
        run: &'a RunKey,
        since: std::time::SystemTime,
    ) -> BoxFuture<'a, Result<Vec<GatewaySignal>, GatewayError>> {
        FakeGateway.run_signals(handle, run, since)
    }
}

struct Host {
    gateway: Arc<Recording>,
    client: std::path::PathBuf,
}

impl JobHost for Host {
    fn docker_cli(
        &self,
        supervisor: Arc<Supervisor>,
        record_dir: std::path::PathBuf,
    ) -> Result<DockerCli, DockerError> {
        Ok(DockerCli::with_binary(
            supervisor,
            self.client.clone(),
            ClientEnv::from_process(),
            record_dir,
        ))
    }

    fn gateway<'a>(
        &'a self,
        _rt: &'a Runtime,
        _spec: &'a JobSpec,
        // TRAIT_OBJECT: the gateway is a recording fake
    ) -> BoxFuture<'a, Result<Arc<dyn Gateway>, OrchestrateError>> {
        // TRAIT_OBJECT: the session takes the gateway as a trait object
        let gateway: Arc<dyn Gateway> = Arc::<Recording>::clone(&self.gateway); // BORROW: the host keeps its handle
        Box::pin(async move { Ok(gateway) })
    }
}

struct Token;

impl SecretSource for Token {
    fn resolve(&self, _name: &str) -> Result<SecretValue, SecretError> {
        Ok(SecretValue::new(String::from("token")))
    }
}

/// Guards: a job starts one proxy per distinct allowlist of its cells, once, and ends them with
/// the session; the session's teardown asks the guard about each allow network. Oracle: the leaf's
/// plan rule; the fake client's call log for the networks.
#[tokio::test]
async fn plan_session_starts_and_ends_one_proxy_per_allowlist() {
    let rig = Rig::new(0);
    let mut rt = rig.runtime();
    let a: &[&str] = &["a.test:443"];
    let ab: &[&str] = &["a.test:443", "b.test:443"];
    rt.cells = cells_of(&[Some(a), Some(a), Some(ab), None]);
    let host = Host {
        gateway: Arc::new(Recording {
            started: Mutex::new(Vec::new()),
        }),
        client: rig.docker.clone(),
    };
    let job_spec = spec(
        &["m"],
        &Facts {
            secrets: Vec::new(),
            ..Facts::base()
        },
        60,
        1,
    );
    let session = JobSession::open(
        &mut rt,
        &job_spec,
        &DIGEST.parse().expect("a digest"),
        &host,
        &Token,
    )
    .await
    .expect("a session");
    let started = host.gateway.started.lock().expect("the log").clone();
    assert_eq!(
        started,
        vec![vec!["a.test:443"], vec!["a.test:443", "b.test:443"]]
    );
    let _ = session.close(&SweepScope::CurrentJob).await;
    let log = std::fs::read_to_string(&rig.log).unwrap_or_default();
    for list in [list(a), list(ab)] {
        let name = NetworkName::allow_for(&rt.job, &list);
        assert!(
            log.contains(name.as_str()),
            "the guard was asked about {}",
            name.as_str()
        );
    }
}

/// Guards: the proxy's create argv has no network alias equal to a listed name (docker answers a
/// container's own alias with its own address), and an allowlisted agent's argv carries the
/// sorted `--add-host` set pointing at the proxy address; a cell without an allowlist has none.
/// Oracle: the probe of a busybox container with `--network-alias pypi.org`, where
/// `nslookup pypi.org 127.0.0.11` answered its own address.
#[test]
fn plan_proxy_has_no_listed_alias_and_agents_get_add_host() {
    use candler_core::isolation::container::run::create_argv;
    use candler_core::isolation::gateway::egress_create_args;
    use candler_harness::spec::CommandSpec;

    let j = job("1234");
    let l = list(&["b.test:443", "a.test:443"]);
    let allow = NetworkName::allow_for(&j, &l);
    let digest = DIGEST.parse().expect("a digest");
    let args = egress_create_args("sha256:abc", &j, &digest, &l, &allow);
    assert!(!args.iter().any(|a| a == "--network-alias"), "{args:?}");
    let mut plan = agent_plan(&j);
    plan.egress = Some(allow);
    plan.add_hosts = vec![
        (String::from("a.test"), String::from("172.30.0.2")),
        (String::from("b.test"), String::from("172.30.0.2")),
    ];
    let cmd = CommandSpec::builder()
        .argv(vec![String::from("agent")])
        .build()
        .expect("a command");
    let created = create_argv(&plan, &cmd).expect("an argv");
    let hosts: Vec<&str> = created
        .windows(2)
        .filter(|w| w[0] == "--add-host")
        .map(|w| w[1].as_str())
        .collect();
    assert_eq!(hosts, ["a.test:172.30.0.2", "b.test:172.30.0.2"]);
    plan.add_hosts.clear();
    assert!(
        !create_argv(&plan, &cmd)
            .expect("an argv")
            .iter()
            .any(|a| a == "--add-host")
    );
}

fn agent_plan(j: &JobId) -> candler_core::isolation::model::ContainerPlan {
    use candler_core::isolation::model::{ContainerPlan, ImageId, ImageRef, Limits, UserSpec};
    let dir = std::path::PathBuf::from("/tmp");
    ContainerPlan {
        image: ImageRef {
            id: ImageId(String::from("sha256:abc")),
            harness_version: None,
        },
        labels: Labels::new()
            .with("candler.job", j.as_str())
            .with("candler.run", "r")
            .with("candler.role", "agent"),
        mounts: Vec::new(),
        env: Vec::new(),
        gateway: None,
        state: Vec::new(),
        network: NetworkName::for_job(j),
        egress: None,
        add_hosts: Vec::new(),
        limits: Limits {
            pids_limit: 64,
            memory: String::from("1g"),
            cpus: 1.0,
        },
        record_dir: dir.clone(),
        user: UserSpec::Engine,
        workdir: String::from("/work"),
        home: String::from("/home/agent"),
        expiry: Duration::from_secs(10),
        project_root: dir.clone(),
        store_root: dir,
        interactive: false,
    }
}
