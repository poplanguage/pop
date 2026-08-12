use pop_runtime_interface::{
    ActorExit, ActorId, ActorIncarnation, ActorLifecycle, ActorLifecycleError, ActorReceive,
    ActorSendError, ActorState,
};

#[test]
fn active_actor_mailbox_preserves_fifo_and_exact_backpressure() {
    let mut actor = ActorLifecycle::starting(ActorId::new(7), ActorIncarnation::new(1), 2);
    let reference = actor.reference();
    actor.activate().expect("starting actor activates once");

    assert_eq!(actor.try_admit(reference, 11), Ok(()));
    assert_eq!(actor.try_admit(reference, 13), Ok(()));
    assert_eq!(
        actor.try_admit(reference, 17),
        Err(ActorSendError::Full(17))
    );
    assert_eq!(actor.try_receive(), ActorReceive::Message(11));
    assert_eq!(actor.try_admit(reference, 17), Ok(()));
    assert_eq!(actor.try_receive(), ActorReceive::Message(13));
    assert_eq!(actor.try_receive(), ActorReceive::Message(17));
    assert_eq!(actor.try_receive(), ActorReceive::Empty);
}

#[test]
fn old_actor_reference_is_stale_after_restart() {
    let first = ActorLifecycle::<u64>::starting(ActorId::new(8), ActorIncarnation::new(3), 1);
    let stale = first.reference();
    let mut restarted = ActorLifecycle::starting(ActorId::new(8), ActorIncarnation::new(4), 1);
    let current = restarted.reference();
    restarted.activate().expect("replacement activates");

    assert_eq!(
        restarted.try_admit(stale, 19),
        Err(ActorSendError::Stale(19))
    );
    assert_eq!(restarted.try_admit(current, 23), Ok(()));
    assert_eq!(restarted.try_receive(), ActorReceive::Message(23));
}

#[test]
fn actor_exit_closes_admission_and_returns_queued_messages_for_cleanup() {
    let mut actor = ActorLifecycle::starting(ActorId::new(9), ActorIncarnation::new(1), 3);
    let reference = actor.reference();
    actor.activate().expect("actor activates");
    actor.try_admit(reference, 29).expect("first admission");
    actor.try_admit(reference, 31).expect("second admission");

    assert_eq!(actor.begin_exit(ActorExit::Panicked), Ok(vec![29, 31]));
    assert_eq!(actor.state(), ActorState::Stopping(ActorExit::Panicked));
    assert_eq!(
        actor.try_admit(reference, 37),
        Err(ActorSendError::Closed(37))
    );
    assert_eq!(actor.try_receive(), ActorReceive::Closed);
    assert_eq!(
        actor.begin_exit(ActorExit::Cancelled),
        Err(ActorLifecycleError::AlreadyStopping(reference))
    );

    assert_eq!(actor.complete_exit(), Ok(ActorExit::Panicked));
    assert_eq!(actor.state(), ActorState::Exited(ActorExit::Panicked));
    assert_eq!(
        actor.complete_exit(),
        Err(ActorLifecycleError::AlreadyExited(reference))
    );
}

#[test]
fn actor_start_and_exit_transitions_are_single_use() {
    let mut actor = ActorLifecycle::<u64>::starting(ActorId::new(10), ActorIncarnation::new(2), 0);
    let reference = actor.reference();

    assert_eq!(actor.state(), ActorState::Starting);
    assert_eq!(actor.activate(), Ok(()));
    assert_eq!(
        actor.activate(),
        Err(ActorLifecycleError::AlreadyActive(reference))
    );
    assert_eq!(
        actor.try_admit(reference, 41),
        Err(ActorSendError::Full(41))
    );
    assert_eq!(actor.begin_exit(ActorExit::Completed), Ok(Vec::new()));
    assert_eq!(actor.complete_exit(), Ok(ActorExit::Completed));
}
