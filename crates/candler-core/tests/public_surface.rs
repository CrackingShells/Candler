//! The public items other crates consume, used from outside the crate: usage only, with no
//! behaviour asserted. It is the contract the API review diffs.
#![cfg(test)]

// --- resolve: config, layers, preflight ---
mod resolve_surface {
    use std::sync::Arc;

    use candler_core::resolve::config::{
        Config, ConfigEnv, ConfigError, ProjectConfig, UserConfig,
    };
    use candler_core::resolve::layers::{
        FileValues, Layered, Resolution, ResolveInput, is_setting,
    };
    use candler_core::resolve::preflight::{
        CheckResult, PreflightError, PreflightStep, ResolveContext, ResolveError, Resolver,
        StepError, StepReport,
    };

    fn send_sync<T: Send + Sync + ?Sized>() {}

    /// Names every resolve item; `Box<dyn PreflightStep>` is `Send + Sync`.
    #[test]
    fn resolve_items_are_nameable() {
        send_sync::<Box<dyn PreflightStep>>(); // TRAIT_OBJECT: the resolver holds steps of different types
        send_sync::<Resolver>();
        send_sync::<ResolveContext>();
        let _ = (
            ConfigEnv::new,
            ConfigEnv::from_process,
            ProjectConfig::discover,
            UserConfig::load,
            Config::new,
            Config::load,
            Layered::new,
            FileValues::new,
            is_setting,
            Resolver::new,
            StepReport::new,
            CheckResult::new,
        );
        let _ = ResolveInput::default();
        let _ = (
            None::<ConfigError>,
            None::<Resolution>,
            None::<ResolveError>,
            None::<PreflightError>,
            None::<StepError>,
            None::<Arc<ResolveContext>>,
        );
    }
}

// ---- schema_ctors ----
mod schema_ctors {
    use candler_schemas::execution::{EventPayload, Integrity, InvalidReason, StrayProcess};
    use candler_schemas::process::ProcessStart;

    /// Builds from outside the crate the wire types that `#[non_exhaustive]` keeps out of reach
    /// of a struct or variant literal.
    #[test]
    fn non_exhaustive_wire_types_are_constructible() {
        let _ = (
            Integrity::Valid,
            Integrity::invalid(InvalidReason::BudgetStop),
            Integrity::invalid(InvalidReason::Killed),
            Integrity::invalid(InvalidReason::Interrupted),
        );
        let _ = (
            ProcessStart::linux,
            ProcessStart::darwin,
            StrayProcess::new,
            EventPayload::run_stray,
            EventPayload::process_exited,
            EventPayload::process_stop_step,
        );
    }
}

// ---- run_store ----

mod store_surface {
    use candler_core::store::layout::StoreLayout;
    use candler_core::store::outcome::{OutcomeFileRef, OutcomeWriter};
    use candler_core::store::run::{Existing, RunEntry, RunWriter};
    use candler_core::store::{Store, StoreError};

    /// Names every store item and the readers of runs, outcomes and the index.
    #[test]
    fn store_items_are_nameable() {
        let _ = (
            Store::init,
            Store::open,
            Store::layout,
            Store::put_blob,
            Store::put_blob_from_path,
            Store::blob_path,
            Store::next_attempt,
            Store::seal_interrupted,
            Store::runs,
            Store::read_manifest,
            Store::read_tree,
            Store::read_trace,
            Store::raw_path,
            Store::outcome_files,
            Store::read_outcomes,
            Store::rebuild_index,
            Store::indexed_runs,
            Store::indexed_outcomes,
            StoreLayout::new,
            RunWriter::create,
            OutcomeWriter::create,
            OutcomeFileRef::new,
        );
        let _ = (
            Existing::Refuse,
            Existing::ReplaceUnfinished,
            None::<RunEntry>,
            None::<StoreError>,
        );
    }
}

// ---- legacy_mapping ----
mod legacy_mapping {
    use candler_core::import::ImportError;
    use candler_core::import::grading::map_grading;
    use candler_core::import::manifest::{MappedRun, map_runs};
    use candler_core::import::read::{LegacyFolder, SkipReason};
    use candler_core::import::rules::{
        HarnessRule, ImportRules, MarkerCode, MarkerRule, SnapshotRule, TreatmentRule,
    };

    /// Names from outside the crate the import rules and their parts, the reader of a legacy
    /// folder, the two mappings and their error and skip types.
    #[test]
    fn the_import_surface_is_nameable() {
        let _ = (
            ImportRules::builder,
            HarnessRule::new,
            TreatmentRule::new,
            SnapshotRule::new,
            MarkerRule::new,
            LegacyFolder::read,
            map_runs,
            map_grading,
        );
        let _ = (
            None::<ImportRules>,
            None::<MarkerCode>,
            None::<LegacyFolder>,
            None::<MappedRun>,
            None::<ImportError>,
            None::<SkipReason>,
        );
    }
}

// ---- supervisor ----
mod supervisor {
    use candler_core::frames::{FrameReader, FrameWriter, FramesError, RawFileInfo, frame_bytes};
    use candler_core::supervise::http::is_loopback;
    use candler_core::supervise::platform::{Liveness, liveness, parse_proc_stat, process_start};
    use candler_core::supervise::reap::{ReapRefusal, ReapReport};
    use candler_core::supervise::record::{record_path, write_record};
    use candler_core::supervise::spawn::{
        Child, DEFAULT_GRACE, FrameObserver, OutputSpec, SpawnRequest, SpawnRequestBuilder,
        StdinSpec,
    };
    use candler_core::supervise::stop::{
        Captured, Ended, Exit, StepRecord, StopHandle, StraySweeper,
    };
    use candler_core::supervise::{EventSink, SuperviseError, Supervisor};
    use candler_harness::spec::Framing;

    fn send_sync<T: Send + Sync + ?Sized>() {}

    /// Names every frames and supervise item the execution milestone consumes; the sink, the
    /// sweeper and the observer are trait objects.
    #[test]
    fn frames_and_supervise_items_are_nameable() {
        send_sync::<Supervisor>();
        send_sync::<StopHandle>();
        send_sync::<dyn EventSink>(); // TRAIT_OBJECT: the job runner chooses where events go
        send_sync::<dyn StraySweeper>(); // TRAIT_OBJECT: the execution milestone provides the sweep
        let _ = None::<Box<dyn FrameObserver>>; // TRAIT_OBJECT: a live reader observes frames
        let _ = (
            FrameWriter::create,
            FrameWriter::write,
            FrameWriter::flush_tail,
            FrameWriter::frames,
            FrameWriter::finish,
            frame_bytes,
            FrameReader::open,
            FrameReader::header,
            RawFileInfo::new,
        );
        let _ = (
            Supervisor::new,
            Supervisor::with_sweeper,
            Supervisor::spawn,
            Supervisor::run_captured,
            Supervisor::reap,
            SpawnRequest::builder,
            SpawnRequestBuilder::argv,
            SpawnRequestBuilder::env,
            SpawnRequestBuilder::marker,
            SpawnRequestBuilder::run,
            SpawnRequestBuilder::cwd,
            SpawnRequestBuilder::stdin,
            SpawnRequestBuilder::stdout,
            SpawnRequestBuilder::stderr,
            SpawnRequestBuilder::deadline,
            SpawnRequestBuilder::stop,
            SpawnRequestBuilder::record_dir,
            SpawnRequestBuilder::build,
            OutputSpec::framed,
            OutputSpec::with_observer,
        );
        let _ = (
            Child::record,
            Child::take_stdin,
            Child::take_stdout,
            Child::take_stderr,
            Child::stop_handle,
            Child::wait,
            Child::stop,
            StopHandle::request,
            StepRecord::new,
            Exit::new,
            Captured::new,
            ReapReport::new,
            ReapRefusal::guard,
            FrameWriter::discard,
        );
        let _ = (
            is_loopback,
            liveness,
            process_start,
            parse_proc_stat,
            record_path,
            write_record,
            DEFAULT_GRACE,
        );
        let _ = (
            None::<Framing>,
            None::<FramesError>,
            None::<SuperviseError>,
            None::<StdinSpec>,
            None::<Ended>,
            None::<Liveness>,
        );
    }
}

// ---- legacy_import ----

/// Guards: the legacy import's public surface — the `import` module with its entry point, the
/// report and each of its entry types and constructors, the reader factory, the asset, stream and
/// trace functions, the two error variants of the import and `trace::normalise` with its error are
/// nameable from outside the crate, at the paths the leaf gives. Oracle: the names written here.
#[test]
fn the_legacy_import_surface_is_public() {
    use candler_core::import::ImportError;
    use candler_core::import::assemble::import;
    use candler_core::import::assets::{
        ImportedAssets, TreeDiff, import_assets, plan_assets, put_sources,
    };
    use candler_core::import::report::{ImportReport, InvalidRun, SkippedEntry};
    use candler_core::import::streams::{
        ReaderFactory, TraceFailure, UsageDiff, frame_streams, usage_diffs,
    };
    use candler_core::trace::{NormaliseError, normalise};

    let _ = (
        import,
        normalise,
        import_assets,
        plan_assets,
        put_sources,
        frame_streams,
        usage_diffs,
    );
    let _ = (
        ImportReport::new,
        InvalidRun::new,
        SkippedEntry::new,
        TraceFailure::new,
        UsageDiff::new,
        TreeDiff::new,
    );
    let _ = (
        None::<ImportReport>,
        None::<InvalidRun>,
        None::<SkippedEntry>,
        None::<TraceFailure>,
        None::<UsageDiff>,
        None::<TreeDiff>,
        None::<ImportedAssets>,
        None::<NormaliseError>,
        None::<Box<ReaderFactory>>,
    );
    // The two variants name their members.
    let store = ImportError::Store {
        run: String::new(),
        detail: String::new(),
    };
    let conflict = ImportError::Conflict {
        run: String::new(),
        file: String::new(),
        detail: String::new(),
    };
    assert!(matches!(store, ImportError::Store { .. }));
    assert!(matches!(conflict, ImportError::Conflict { .. }));
}

// ---- gate_sweep ----

mod writer_steps {
    use candler_core::store::outcome::OutcomeWriter;
    use candler_core::store::run::RunWriter;

    /// Names the steps of a run writer and of an outcome writer, which the runner and the
    /// evaluation engine call in order after `create`.
    #[test]
    fn writer_steps_are_nameable() {
        let _ = (
            RunWriter::manifest,
            RunWriter::tree,
            RunWriter::raw,
            RunWriter::trace,
            RunWriter::done,
        );
        let _ = (OutcomeWriter::append, OutcomeWriter::commit);
    }
}

// ---- m2b scaffold ----
#[cfg(unix)]
mod m2b_surface {
    use std::sync::Arc;

    use candler_core::experiment::paths;
    use candler_core::isolation::container::DockerIsolation;
    use candler_core::isolation::container::guard::{
        ContainerGuard, Destroyed, GuardError, GuardRefusal,
    };
    use candler_core::isolation::container::mounts::{MountMode, MountSpec};
    use candler_core::isolation::container::stop::ContainerStopHook;
    use candler_core::isolation::container::sweeper::{ContainerSweeper, LabelSweeper, SweepError};
    use candler_core::isolation::docker::DockerError;
    use candler_core::isolation::driver::{DestroyDriver, DockerDriver};
    use candler_core::isolation::gateway::{
        ChildRecorder, Gateway, GatewayError, GatewaySignal, GatewayStart, GatewayStatus,
        SecretError, SecretSource, SecretValue,
    };
    use candler_core::isolation::image::{
        BuildSetting, Extension, ImageError, ImageService, LockDigest, LockRequest, LockSource,
    };
    use candler_core::isolation::layout;
    use candler_core::isolation::model::{
        BuildRequest, ContainerOutcome, ContainerPlan, ContainerRef, ContainerSweep, GatewayHandle,
        ImageId, ImageInfo, ImageRef, Inspect, InspectState, LabelSel, Labels, LayerDigest, Limits,
        NetworkInfo, NetworkName, Refusal, SweepScope, UserSpec,
    };
    use candler_core::isolation::testing::{
        AllowlistDriver, FakeJobView, RecordingDestroyer, RecordingDocker,
    };
    use candler_core::isolation::{EndFacts, IsolatedRequest, Isolation, IsolationError};
    use candler_core::jobs::dir::{JobDir, JobError};
    use candler_core::jobs::lost::{RecoverError, RecoverReport, recover};
    use candler_core::jobs::model::{JobBody, JobCause, JobControl, JobOutcome, JobSpec};
    use candler_core::jobs::runtime::{JobEntry, JobServices, RuntimeError, supervise_entry};
    use candler_core::jobs::slots::{Holder, Slot, SlotError, SlotInfo, Slots};
    use candler_core::jobs::view::{ChildEntry, JobView, JobViewError};
    use candler_core::manifest::compare::{ArmView, NUISANCE_PATHS, Verdict, VerdictError};
    use candler_core::resolve::steps::{M2bServices, register_m2b_steps};
    use candler_core::runner::evidence::catalog::{Catalog, CatalogError, CatalogProvider};
    use candler_core::runner::evidence::{Evidence, Inputs};
    use candler_core::runner::exec::exec_run;
    use candler_core::runner::plan::{
        ExperimentLock, Force, PlanCtx, PlanError, PlanMode, PlanOutcome, RunPlan, plan_runs,
    };
    use candler_core::runner::{JobGateway, JobMeter, RunControl, RunOutcome, Runtime, Usage};
    use candler_core::supervise::hook::{HookError, StopHook, StopHookCtx};

    fn send_sync<T: Send + Sync + ?Sized>() {}

    /// Names every M2b trait object the leaves hold and every stub entry point.
    #[test]
    fn m2b_items_are_nameable() {
        send_sync::<dyn StopHook>(); // TRAIT_OBJECT: a run attaches the hook that destroys its container
        send_sync::<dyn DockerDriver>(); // TRAIT_OBJECT: docker, or a recording fake
        send_sync::<dyn DestroyDriver>(); // TRAIT_OBJECT: only the guard holds one
        send_sync::<dyn JobView>(); // TRAIT_OBJECT: the job directory, or a fake
        send_sync::<dyn ContainerSweeper>(); // TRAIT_OBJECT: the label sweeper, or a fake
        send_sync::<dyn ImageService>(); // TRAIT_OBJECT: docker, or a fake
        send_sync::<dyn Gateway>(); // TRAIT_OBJECT: the gateway container, or a fake
        send_sync::<dyn SecretSource>(); // TRAIT_OBJECT: the environment, or a fake
        send_sync::<dyn CatalogProvider>(); // TRAIT_OBJECT: lists commands in the image, or a fake
        send_sync::<dyn EndFacts>(); // TRAIT_OBJECT: the run's stop hook, read after the wait
        send_sync::<dyn Isolation>(); // TRAIT_OBJECT: docker isolation, or a fake
        send_sync::<ContainerGuard>();
        send_sync::<Runtime>();
        send_sync::<M2bServices>();
        let _ = (
            ContainerGuard::new,
            ContainerStopHook::new,
            DockerIsolation::new,
            StopHookCtx::new,
            SecretValue::new,
            Slots::open,
            JobDir::new,
            JobMeter::new,
            FakeJobView::new,
            AllowlistDriver::<RecordingDestroyer>::new,
            RecordingDocker::new,
            RecordingDestroyer::new,
            Labels::new,
            NetworkName::for_job,
            NetworkName::egress_for_job,
            ImageInfo::new,
            ContainerRef::new,
            Inspect::new,
            InspectState::new,
        );
        let _ = (
            register_m2b_steps,
            recover,
            supervise_entry,
            plan_runs,
            exec_run,
            SweepScope::wire,
            GatewayHandle::run_base_url,
            layout::image_lock_path,
        );
        let _ = (
            layout::EXPIRY_MARGIN,
            layout::CONTAINER_WORKDIR,
            paths::MODEL_ID,
        );
        let _ = (
            NUISANCE_PATHS,
            Force::No,
            MountMode::ReadOnly,
            UserSpec::Engine,
        );
        let _ = (
            None::<Arc<dyn StopHook>>, // TRAIT_OBJECT: the spawn request holds the hook
            None::<JobBody>,
            None::<JobControl>,
            None::<JobCause>,
            None::<JobOutcome>,
            None::<JobSpec>,
            None::<JobEntry>,
            None::<JobServices>,
            None::<RunOutcome>,
            None::<RunControl>,
            None::<RunPlan>,
            None::<PlanCtx>,
            None::<PlanOutcome>,
            None::<PlanError>,
            None::<Usage>,
            None::<IsolatedRequest>,
            None::<JobGateway>,
            None::<ExperimentLock>,
            None::<PlanMode>,
            None::<GatewaySignal>,
            None::<Evidence>,
            None::<Inputs>,
            None::<Catalog>,
            None::<CatalogError>,
        );
        let _ = (
            None::<Destroyed>,
            None::<GuardError>,
            None::<GuardRefusal>,
            None::<MountSpec>,
            None::<LabelSweeper>,
            None::<SweepError>,
            None::<DockerError>,
            None::<GatewayError>,
            None::<GatewayStart<'static>>,
            None::<&dyn ChildRecorder>,
            None::<GatewayStatus>,
            None::<GatewayHandle>,
            None::<SecretError>,
            None::<ImageError>,
            None::<BuildSetting>,
            None::<Extension>,
            None::<LockRequest>,
            None::<LockSource>,
            None::<LockDigest>,
            None::<BuildRequest>,
            None::<ContainerOutcome>,
            None::<ContainerPlan>,
            None::<ContainerSweep>,
            None::<ImageId>,
            None::<ImageRef>,
            None::<LabelSel>,
            None::<LayerDigest>,
            None::<Limits>,
            None::<NetworkInfo>,
            None::<Refusal>,
            None::<IsolationError>,
            None::<HookError>,
        );
        let _ = (
            None::<JobError>,
            None::<RecoverReport>,
            None::<RecoverError>,
            None::<RuntimeError>,
            None::<Holder>,
            None::<Slot>,
            None::<SlotError>,
            None::<SlotInfo>,
            None::<ChildEntry>,
            None::<JobViewError>,
            None::<ArmView>,
            None::<Verdict>,
            None::<VerdictError>,
        );
    }
}

// ---- m2a-evaluation-parity: one block per leaf, each edited only by its leaf ----

/// The `evaluation::registry` items, filled by the `registry` leaf.
#[cfg(unix)]
mod registry_surface {
    use std::collections::BTreeMap;
    use std::path::{Path, PathBuf};
    use std::sync::Arc;

    use candler_core::evaluation::registry::{
        CheckedEntry, CheckedSuite, EnvironmentKey, EnvironmentStep, NeedError, Plugin, Registry,
        RegistryError, default_env_root, register_evaluation_steps,
    };
    use candler_core::resolve::config::Config;
    use candler_core::resolve::preflight::{PreflightStep, Resolver};
    use candler_core::supervise::Supervisor;

    fn send_sync<T: Send + Sync + ?Sized>() {}

    /// Names every registry item and the signatures the engine and the binary call.
    #[test]
    fn registry_items_are_nameable() {
        send_sync::<EnvironmentStep>();
        send_sync::<Registry>();
        let _: fn(&[PathBuf]) -> Result<Registry, RegistryError> = Registry::load;
        let _ = (
            Registry::check_suite,
            Registry::plugins,
            Registry::plugin,
            Plugin::new,
        );
        let _: fn(&Config) -> Result<PathBuf, RegistryError> = default_env_root;
        let _: fn(Resolver, &Registry, &Path, &Arc<Supervisor>) -> _ = register_evaluation_steps;
        let _ = (
            EnvironmentStep::for_plugin,
            EnvironmentStep::with_path,
            EnvironmentKey::of_files,
            EnvironmentKey::digest,
            EnvironmentKey::dir,
            CheckedSuite::entries,
        );
        let _ = <EnvironmentStep as PreflightStep>::name;
        let _ = (
            None::<CheckedEntry>,
            None::<NeedError>,
            None::<BTreeMap<String, Plugin>>,
        );
    }
}

/// The `evaluation::context` items, filled by the `run_context` leaf.
mod run_context_surface {
    use candler_core::evaluation::context::{
        Assembled, AssembledArtifact, ContextError, FileRule, Workspace, assemble, file_rule_keeps,
        materialise,
    };

    /// Names the context items: assembling a run's context from needs, materialising it into a
    /// workspace, the file rule and its error, so that the engine can call them in order.
    #[test]
    fn context_items_are_nameable() {
        let _ = (
            assemble,
            materialise,
            file_rule_keeps,
            FileRule::new,
            FileRule::keeps,
            Workspace::prepare,
            Workspace::root,
            Workspace::outputs_dir,
            Assembled::run,
            Assembled::store_root,
            Assembled::artifacts,
            Assembled::metadata,
        );
        let _ = (None::<AssembledArtifact>, None::<ContextError>);
    }
}

/// The `evaluation::client` items, filled by the `protocol_client` leaf.
#[cfg(unix)]
mod protocol_client_surface {
    use std::sync::Arc;

    use candler_core::evaluation::client::{
        Client, ClientError, ClientOutcome, Clock, DEFAULT_CANCEL_GRACE, DEFAULT_DESCRIBE_DEADLINE,
        EvaluatorCommand, ItemOutcome, SystemClock, Unreported, UnreportedReason,
    };

    fn send_sync<T: Send + Sync + ?Sized>() {}

    /// Names every client item; `Arc<dyn Clock>` is `Send + Sync`.
    #[test]
    fn client_items_are_nameable() {
        send_sync::<Arc<dyn Clock>>(); // TRAIT_OBJECT: tests drive the deadline on their own clock
        send_sync::<EvaluatorCommand>();
        send_sync::<ClientOutcome>();
        let _ = (
            Client::new,
            Client::with_clock,
            Client::with_graces,
            Client::with_describe_deadline,
            Client::describe,
            Client::evaluate,
            EvaluatorCommand::new,
            EvaluatorCommand::env,
            EvaluatorCommand::cwd,
            EvaluatorCommand::run,
            UnreportedReason::code,
            ItemOutcome::expectation_id,
            DEFAULT_CANCEL_GRACE,
            DEFAULT_DESCRIBE_DEADLINE,
        );
        let _ = (SystemClock, None::<ClientError>, None::<Unreported>);
    }
}

/// The `evaluation::selection` items, filled by the `selection_rates` leaf.
mod selection_rates_surface {
    use std::collections::BTreeMap;

    use candler_core::evaluation::selection::diff::{
        Change, ChangeClass, DiffReport, Equivalence, EquivalenceError, EquivalenceRecord,
        Uncompared, diff,
    };
    use candler_core::evaluation::selection::fields::FourFields;
    use candler_core::evaluation::selection::rates::{CellKey, Rates, rates, rates_toml};
    use candler_core::evaluation::selection::select::{I4Key, I4Violation, Selection, select};

    /// Names every selection, rates and diff item.
    #[test]
    fn selection_items_are_nameable() {
        let _ = (
            select::<Vec<_>>,
            diff,
            rates,
            rates_toml,
            Selection::current,
            Selection::get,
            Selection::violations,
            Selection::is_empty,
            Selection::for_evaluator,
            I4Key::of,
            CellKey::new,
            CellKey::digest,
            Rates::gate,
            Rates::quality,
            Equivalence::load,
            Equivalence::from_bytes,
            Equivalence::sha256,
            Equivalence::row_count,
            Equivalence::rule_names,
            FourFields::of,
            FourFields::normalised,
            FourFields::value_equals,
            FourFields::is_error,
        );
        let _ = (
            None::<I4Violation>,
            None::<DiffReport>,
            None::<Change>,
            None::<ChangeClass>,
            None::<Uncompared>,
            None::<EquivalenceRecord>,
            None::<EquivalenceError>,
            None::<BTreeMap<CellKey, Rates>>,
        );
    }
}

/// The `evaluation::engine` items, filled by the `engine` leaf.
#[cfg(unix)]
mod engine_surface {
    use std::time::Duration;

    use candler_core::evaluation::engine::{
        Batch, EvaluateError, EvaluateReport, EvaluateRequest, EvaluatorSlots, PlanError,
        REQUEST_DEADLINE_MARGIN, RunPlan, RunSelection, applies, evaluate, evaluate_reporting,
        pending_runs, plan, regrade,
    };
    use candler_core::evaluation::registry::{CheckedSuite, discover_manifests};
    use candler_core::jobs::slots::default_evaluator_slots;
    use candler_schemas::evaluation::SuiteEntry;
    use candler_schemas::execution::RunManifest;
    use candler_schemas::primitives::RequestId;

    /// Names every engine item and the signatures the binary calls in order: plan, then evaluate.
    #[test]
    fn engine_items_are_nameable() {
        let planned: fn(&RunManifest, &CheckedSuite) -> Result<RunPlan, PlanError> = plan;
        let applied: fn(&RunManifest, &SuiteEntry) -> bool = applies;
        let _ = (
            planned,
            applied,
            evaluate,
            evaluate_reporting,
            regrade,
            pending_runs,
        );
        let _ = (
            None::<Batch>,
            None::<EvaluateError>,
            None::<EvaluateReport>,
            None::<EvaluateRequest>,
            None::<EvaluatorSlots>,
            None::<RunSelection>,
        );
        let document: fn(&EvaluateReport) -> serde_json::Value = EvaluateReport::to_document;
        let slots: fn(usize) -> u32 = default_evaluator_slots;
        let discovered: fn(&[std::path::PathBuf]) -> Result<Vec<std::path::PathBuf>, _> =
            discover_manifests;
        let _ = (document, slots, discovered);
        assert_eq!(REQUEST_DEADLINE_MARGIN, Duration::from_secs(30));
        // The report names one `(request id, deadline)` per request sent.
        let report = EvaluateReport::default();
        let deadlines: &Vec<(RequestId, Duration)> = &report.request_deadlines;
        assert_eq!(deadlines.as_slice(), []);
    }
}

/// The items of the evaluate job body, filled by the `evaluate_body` leaf.
#[cfg(unix)]
mod evaluate_job_surface {
    use candler_core::jobs::model::{EvaluateSpec, JobControl, JobSpecFile};
    use candler_core::jobs::runtime::RuntimeError;
    use candler_core::jobs::runtime::evaluate::{
        CancelBridge, EvaluateEntry, EvaluateJobOutcome, REPORT_FILE, cancel_bridge,
        supervise_evaluate,
    };
    use candler_core::jobs::slots::evaluator_slots_dir;
    use futures::future::BoxFuture;
    use tokio::sync::watch;

    /// Names the evaluate job's entry, outcome, spec and the functions the binary calls.
    #[test]
    fn evaluate_job_items_are_nameable() {
        let supervise: fn(
            EvaluateEntry,
        ) -> BoxFuture<'static, Result<EvaluateJobOutcome, RuntimeError>> = supervise_evaluate;
        let bridge: fn(&JobControl) -> (watch::Receiver<bool>, CancelBridge) = cancel_bridge;
        let slots: fn(&std::path::Path) -> std::path::PathBuf = evaluator_slots_dir;
        let _ = (supervise, bridge, slots);
        let _ = (
            None::<EvaluateEntry>,
            None::<EvaluateJobOutcome>,
            None::<EvaluateSpec>,
            None::<JobSpecFile>,
        );
        assert_eq!(REPORT_FILE, "report.json");
    }
}
