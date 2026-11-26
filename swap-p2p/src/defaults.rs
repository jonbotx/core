use std::time::Duration;

pub const DEFAULT_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);
pub const IDLE_CONNECTION_TIMEOUT: Duration = Duration::from_secs(15 * 60); // 15 minutes

pub const BACKOFF_MULTIPLIER: f64 = 1.5;

// Redial
//
// Peers that lose a connection are retried with exponential backoff, and the
// backoff is reset as soon as a connection is established (see
// `BackoffTracker::reset`). So these limits only govern *unsuccessful* peers —
// they do not slow down a healthy connection or a quick reconnect after a drop.
//
// Over Tor, every dial attempt to a `.onion` peer makes Arti re-solve that
// service's hidden-service proof-of-work (and the effort is ramped up on each
// failed attempt, up to Arti's maximum). With the old cap (10s/30s) a handful of
// permanently-unreachable onion makers kept a whole set of CPU cores pegged with
// PoW solving, continuously.
pub const REDIAL_INITIAL_INTERVAL: Duration = Duration::from_secs(10);
pub const REDIAL_MAX_INTERVAL: Duration = Duration::from_secs(300);

// Redial for peers we have *never* managed to connect to.
//
// Makers come and go, so the rendezvous point keeps advertising makers that are
// currently offline. Those peers are the ones that actually cost CPU: they fail
// every dial, ramp their hidden-service PoW effort to the maximum and never
// reset the backoff. Peers that have connected at least once keep the normal
// (short) cap above so a transient drop recovers quickly, while peers that have
// never connected back off much harder (upstream #865 asks for this: dial
// frequently-reachable peers often, and long-unreachable peers rarely).
//
// The backoff is still reset as soon as such a peer *does* connect, and the user
// can force an immediate retry of every peer from the "force a network refresh"
// button in the GUI, so a maker that comes back online is not hidden for long.
pub const REDIAL_UNREACHABLE_INITIAL_INTERVAL: Duration = Duration::from_secs(60);
pub const REDIAL_UNREACHABLE_MAX_INTERVAL: Duration = Duration::from_secs(60 * 30);

// Rendezvous
pub const RENDEZVOUS_REDIAL_MAX_INTERVAL: Duration = Duration::from_secs(60);

// Rendezvous discovery
pub const DISCOVERY_INITIAL_INTERVAL: Duration = Duration::from_secs(1);
pub const DISCOVERY_MAX_INTERVAL: Duration = Duration::from_secs(60 * 3);
pub const DISCOVERY_INTERVAL: Duration = Duration::from_secs(60);

// Rendezvous register
pub const RENDEZVOUS_RETRY_INITIAL_INTERVAL: Duration = Duration::from_secs(1);
pub const RENDEZVOUS_RETRY_MAX_INTERVAL: Duration = Duration::from_secs(60);

// Quote
pub const CACHED_QUOTE_EXPIRY: Duration = Duration::from_secs(180);
pub const QUOTE_INTERVAL: Duration = Duration::from_secs(45);
pub const QUOTE_REDIAL_INTERVAL: Duration = Duration::from_secs(10);
pub const QUOTE_REDIAL_MAX_INTERVAL: Duration = Duration::from_secs(300);
pub const QUOTE_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

// Swap setup
pub const NEGOTIATION_TIMEOUT: Duration = Duration::from_secs(120);
/// How long Alice keeps the setup open while Bob's user decides whether to lock the Bitcoin.
/// Must be longer than the preparation and approval timeouts of Bob combined, plus time to commit.
pub const SWAP_SETUP_DECISION_TIMEOUT: Duration = Duration::from_secs(180);
/// How long Bob waits for Alice to confirm that she persisted the swap.
pub const SWAP_SETUP_PERSISTED_TIMEOUT: Duration = Duration::from_secs(60);
/// How long Alice may take to persist a swap Bob committed to. Committing can take
/// another few seconds on a busy database, so this stays well below
/// [`SWAP_SETUP_PERSISTED_TIMEOUT`] and her confirmation still reaches Bob.
pub const SWAP_SETUP_PERSIST_TIMEOUT: Duration = Duration::from_secs(20);
const _: () =
    assert!(SWAP_SETUP_PERSIST_TIMEOUT.as_secs() * 2 < SWAP_SETUP_PERSISTED_TIMEOUT.as_secs());
pub const SWAP_SETUP_KEEP_ALIVE: Duration = Duration::from_secs(30);
pub const SWAP_SETUP_CHANNEL_TIMEOUT: Duration = Duration::from_secs(60);
