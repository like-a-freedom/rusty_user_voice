use std::future::Future;

use crate::{ClientError, Continuation, RateLimit, User};

/// Successful API response envelope: parsed body plus optional rate-limit
/// state and `Link: rel="next"` continuation extracted from headers.
#[derive(Clone, Debug)]
pub struct ApiResponse<T> {
    pub data: T,
    pub rate_limit: Option<RateLimit>,
    pub continuation: Option<Continuation>,
}

/// Native-async UserVoice API seam. Implementations must be
/// `Clone + Send + Sync + 'static`; `ReqwestUserVoiceClient` is the production
/// impl, `FakeUserVoiceApi` (in the `rusty_user_voice` test suite) is the
/// in-memory test impl.
///
/// Methods return `impl Future` rather than using `async_trait` so the trait
/// stays zero-overhead (`async-fn-in-trait`, stable since Rust 1.75).
pub trait UserVoiceApi: Clone + Send + Sync + 'static {
    /// Returns the authenticated principal. Acts as the access preflight.
    ///
    /// # Errors
    ///
    /// Returns [`ClientError::Authentication`] on 401/403,
    /// [`ClientError::NotFound`] on 404, or any transport/decode variant.
    fn current_user(&self) -> impl Future<Output = Result<ApiResponse<User>, ClientError>> + Send;
}
