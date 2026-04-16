use agents_core::{Agent, AgentsError, Runner};

#[tokio::test]
async fn run_sync_rejects_active_runtime() {
    let agent = Agent::builder("assistant").build();

    let error = Runner::new()
        .run_sync(&agent, "hello")
        .expect_err("run_sync should reject active runtimes");

    assert!(matches!(error, AgentsError::User(_)));
    assert!(
        error.to_string().contains("event loop is already running"),
        "unexpected error: {error}"
    );
}
