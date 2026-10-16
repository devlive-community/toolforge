use super::*;
use std::future::{Future, pending};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::task::{Context, Poll, Waker};

fn ready<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    match future
        .as_mut()
        .poll(&mut Context::from_waker(Waker::noop()))
    {
        Poll::Ready(result) => result,
        Poll::Pending => panic!("expected an immediately ready test operation"),
    }
}

fn available() -> Pending<&'static str> {
    let state = Pending::default();
    state.begin_check().unwrap().replace(Some("1.2.3"));
    state
}

#[test]
fn failed_installations_retry_the_same_candidate_until_success() {
    let state = available();
    for _ in 0..2 {
        let result = ready(state.install(|version| async move {
            assert_eq!(version, "1.2.3");
            Err(AppError::new("update.install_failed"))
        }));
        assert_eq!(result.unwrap_err().code, "update.install_failed");
    }
    ready(state.install(|version| async move {
        assert_eq!(version, "1.2.3");
        Ok(())
    }))
    .unwrap();
    assert_eq!(
        ready(state.install(|_| async { Ok(()) })).unwrap_err().code,
        "update.none"
    );
}

#[test]
fn no_candidate_does_not_leave_the_updater_busy() {
    let state = Pending::<&str>::default();
    assert_eq!(
        ready(state.install(|_| async { panic!("no installer should run") }))
            .unwrap_err()
            .code,
        "update.none"
    );
    state.begin_check().unwrap().replace(Some("new"));
    ready(state.install(|version| async move {
        assert_eq!(version, "new");
        Ok(())
    }))
    .unwrap();
}

#[test]
fn active_installation_rejects_checks_and_duplicate_installations() {
    let state = available();
    let mut first = Box::pin(state.install(|_| pending::<AppResult<()>>()));
    assert!(matches!(
        first.as_mut().poll(&mut Context::from_waker(Waker::noop())),
        Poll::Pending
    ));
    assert_eq!(state.begin_check().err().unwrap().code, "update.busy");
    assert_eq!(
        ready(state.install(|_| async { panic!("duplicate installer") }))
            .unwrap_err()
            .code,
        "update.busy"
    );
    // Cancellation after the operation has actually started must preserve it.
    drop(first);
    ready(state.install(|version| async move {
        assert_eq!(version, "1.2.3");
        Ok(())
    }))
    .unwrap();
}

#[test]
fn active_check_rejects_checks_and_installations_without_losing_candidate() {
    let state = available();
    let checking = state.begin_check().unwrap();
    assert_eq!(state.begin_check().err().unwrap().code, "update.busy");
    assert_eq!(
        ready(state.install(|_| async { panic!("installation during check") }))
            .unwrap_err()
            .code,
        "update.busy"
    );
    // A failed check never publishes a replacement.
    drop(checking);
    ready(state.install(|version| async move {
        assert_eq!(version, "1.2.3");
        Ok(())
    }))
    .unwrap();
}

#[test]
fn successful_checks_replace_or_clear_the_previous_candidate() {
    let state = available();
    state.begin_check().unwrap().replace(Some("2.0.0"));
    assert_eq!(
        ready(state.install(|version| async move {
            assert_eq!(version, "2.0.0");
            Err(AppError::new("update.install_failed"))
        }))
        .unwrap_err()
        .code,
        "update.install_failed"
    );
    state.begin_check().unwrap().replace(None);
    assert_eq!(
        ready(state.install(|_| async { Ok(()) })).unwrap_err().code,
        "update.none"
    );
}

#[test]
fn installer_panic_releases_the_lease_and_preserves_retry() {
    let state = available();
    let result = catch_unwind(AssertUnwindSafe(|| {
        ready(state.install(|_| async { panic!("simulated installer panic") }))
    }));
    assert!(result.is_err());
    ready(state.install(|version| async move {
        assert_eq!(version, "1.2.3");
        Ok(())
    }))
    .unwrap();
}
