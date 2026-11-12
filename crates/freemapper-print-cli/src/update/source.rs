//! The network of the self-update: an [`UpdateSource`] over its own HTTP agent, which follows
//! redirects by hand so that every hop passes the host rule (ADR 0060, P9). The requests and the
//! stub for the tests follow in the next tasks; this holds the agent.

use std::time::Duration;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// One request of a manifest or signature (each redirect hop is its own), body included.
const GET_TIMEOUT: Duration = Duration::from_secs(60);

/// A source of updates over HTTPS. Its agent follows no redirects: the source follows them itself,
/// after each target passed the host rule of `freemapper-update`.
pub struct HttpUpdateSource {
    agent: ureq::Agent,
}

impl HttpUpdateSource {
    pub fn new() -> Self {
        Self {
            agent: ureq::Agent::config_builder()
                .max_redirects(0)
                // Redirects and error answers arrive as responses, so their status can be named.
                .http_status_as_error(false)
                .timeout_connect(Some(CONNECT_TIMEOUT))
                .timeout_global(Some(GET_TIMEOUT))
                // The bytes as they are: the hashes in the manifest are those of the files.
                .accept_encoding("identity")
                .build()
                .into(),
        }
    }
}
