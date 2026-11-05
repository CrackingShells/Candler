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
    use candler_core::frames::{
        FrameMode, FrameReader, FrameWriter, FramesError, RawFileInfo, frame_bytes,
    };
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
            None::<FrameMode>,
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
