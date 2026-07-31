use pop_runtime_interface::{
    ChannelId, ChannelLifecycle, ChannelReceive, ChannelSendError, ChannelState,
};

#[test]
fn bounded_channel_preserves_fifo_and_reports_backpressure_without_losing_values() {
    let mut channel = ChannelLifecycle::bounded(ChannelId::new(7), 2);

    assert_eq!(channel.try_send(11), Ok(()));
    assert_eq!(channel.try_send(13), Ok(()));
    assert_eq!(channel.try_send(17), Err(ChannelSendError::Full(17)));
    assert_eq!(channel.try_receive(), ChannelReceive::Item(11));
    assert_eq!(channel.try_send(17), Ok(()));
    assert_eq!(channel.try_receive(), ChannelReceive::Item(13));
    assert_eq!(channel.try_receive(), ChannelReceive::Item(17));
    assert_eq!(channel.try_receive(), ChannelReceive::Empty);
}

#[test]
fn sender_close_drains_buffer_before_receive_observes_closed() {
    let mut channel = ChannelLifecycle::bounded(ChannelId::new(8), 2);
    channel.try_send("first").expect("first value is admitted");
    channel
        .try_send("second")
        .expect("second value is admitted");

    assert!(channel.close());
    assert!(!channel.close());
    assert_eq!(channel.state(), ChannelState::SenderClosed);
    assert_eq!(
        channel.try_send("late"),
        Err(ChannelSendError::Closed("late"))
    );
    assert_eq!(channel.try_receive(), ChannelReceive::Item("first"));
    assert_eq!(channel.try_receive(), ChannelReceive::Item("second"));
    assert_eq!(channel.try_receive(), ChannelReceive::Closed);
}

#[test]
fn endpoint_lifetimes_close_the_corresponding_direction_exactly_once() {
    let mut channel = ChannelLifecycle::bounded(ChannelId::new(9), 3);
    channel.retain_sender().expect("sender clone");
    channel.retain_receiver().expect("receiver clone");
    channel
        .try_send(23)
        .expect("buffered before receiver closes");

    assert!(channel.release_sender());
    assert_eq!(channel.sender_count(), 1);
    assert!(!channel.release_sender());
    assert_eq!(channel.state(), ChannelState::SenderClosed);
    assert_eq!(channel.try_receive(), ChannelReceive::Item(23));
    assert_eq!(channel.try_receive(), ChannelReceive::Closed);

    assert!(channel.release_receiver().is_empty());
    assert!(channel.release_receiver().is_empty());
    assert_eq!(channel.state(), ChannelState::Closed);
}

#[test]
fn last_receiver_release_returns_buffered_values_for_precise_root_cleanup() {
    let mut channel = ChannelLifecycle::bounded(ChannelId::new(10), 3);
    channel.try_send(29).expect("first value");
    channel.try_send(31).expect("second value");

    assert_eq!(channel.release_receiver(), vec![29, 31]);
    assert_eq!(channel.state(), ChannelState::ReceiverClosed);
    assert_eq!(channel.try_send(37), Err(ChannelSendError::Closed(37)));
    assert_eq!(channel.try_receive(), ChannelReceive::Closed);
}

#[test]
fn zero_capacity_is_an_explicit_rendezvous_channel_until_waiters_are_paired() {
    let mut channel = ChannelLifecycle::bounded(ChannelId::new(11), 0);

    assert_eq!(channel.capacity(), 0);
    assert_eq!(channel.try_send(41), Err(ChannelSendError::Full(41)));
    assert_eq!(channel.try_receive(), ChannelReceive::Empty);
}

#[test]
fn logical_capacity_does_not_eagerly_allocate_the_declared_bound() {
    let mut channel = ChannelLifecycle::bounded(ChannelId::new(12), u64::MAX);

    assert_eq!(channel.capacity(), u64::MAX);
    assert_eq!(channel.length(), 0);
    assert_eq!(channel.try_send(43), Ok(()));
    assert_eq!(channel.length(), 1);
}
