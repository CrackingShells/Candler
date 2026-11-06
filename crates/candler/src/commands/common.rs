//! What every command shares: the store root (`--store`, else the configuration), the `Resolver`
//! with each milestone's preflight steps registered one per line, and the event sink the
//! supervisor reports to. The surfaces leaf fills it, and later commands call it.
