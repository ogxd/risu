use futures::Future;
use hyper::rt::Executor;

#[derive(Clone)]
pub struct TokioExecutor;

impl<F> Executor<F> for TokioExecutor
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    fn execute(&self, future: F)
    {
        tokio::spawn(future);
    }
}
