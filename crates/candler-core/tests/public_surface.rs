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
