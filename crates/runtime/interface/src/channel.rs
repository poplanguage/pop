//! Backend-neutral bounded channel lifecycle and FIFO admission contract.

use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ChannelId(u64);

impl ChannelId {
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelState {
    Open,
    SenderClosed,
    ReceiverClosed,
    Closed,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChannelSendError<T> {
    Full(T),
    Closed(T),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ChannelReceive<T> {
    Item(T),
    Empty,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ChannelEndpointError {
    Closed(ChannelId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChannelLifecycle<T> {
    id: ChannelId,
    capacity: usize,
    queue: VecDeque<T>,
    sender_count: usize,
    receiver_count: usize,
    sender_closed: bool,
    receiver_closed: bool,
}

impl<T> ChannelLifecycle<T> {
    #[must_use]
    pub fn bounded(id: ChannelId, capacity: usize) -> Self {
        Self {
            id,
            capacity,
            queue: VecDeque::with_capacity(capacity),
            sender_count: 1,
            receiver_count: 1,
            sender_closed: false,
            receiver_closed: false,
        }
    }

    #[must_use]
    pub const fn id(&self) -> ChannelId {
        self.id
    }

    #[must_use]
    pub const fn capacity(&self) -> usize {
        self.capacity
    }

    #[must_use]
    pub fn length(&self) -> usize {
        self.queue.len()
    }

    #[must_use]
    pub const fn sender_count(&self) -> usize {
        self.sender_count
    }

    #[must_use]
    pub const fn receiver_count(&self) -> usize {
        self.receiver_count
    }

    #[must_use]
    pub const fn state(&self) -> ChannelState {
        match (self.sender_closed, self.receiver_closed) {
            (false, false) => ChannelState::Open,
            (true, false) => ChannelState::SenderClosed,
            (false, true) => ChannelState::ReceiverClosed,
            (true, true) => ChannelState::Closed,
        }
    }

    pub fn retain_sender(&mut self) -> Result<(), ChannelEndpointError> {
        if self.sender_closed || self.receiver_closed {
            return Err(ChannelEndpointError::Closed(self.id));
        }
        self.sender_count = self
            .sender_count
            .checked_add(1)
            .expect("channel sender count overflow");
        Ok(())
    }

    pub fn retain_receiver(&mut self) -> Result<(), ChannelEndpointError> {
        if self.receiver_closed {
            return Err(ChannelEndpointError::Closed(self.id));
        }
        self.receiver_count = self
            .receiver_count
            .checked_add(1)
            .expect("channel receiver count overflow");
        Ok(())
    }

    /// Releases one sender and returns whether another sender remains.
    pub fn release_sender(&mut self) -> bool {
        if self.sender_count == 0 {
            return false;
        }
        self.sender_count -= 1;
        if self.sender_count == 0 {
            self.sender_closed = true;
        }
        self.sender_count != 0
    }

    /// Releases one receiver. The final release returns buffered payloads so
    /// the owning runtime can release any corresponding precise GC roots.
    pub fn release_receiver(&mut self) -> Vec<T> {
        if self.receiver_count == 0 {
            return Vec::new();
        }
        self.receiver_count -= 1;
        if self.receiver_count != 0 {
            return Vec::new();
        }
        self.receiver_closed = true;
        self.queue.drain(..).collect()
    }

    /// Closes the sender direction for every endpoint.
    pub fn close(&mut self) -> bool {
        if self.sender_closed {
            return false;
        }
        self.sender_closed = true;
        true
    }

    pub fn try_send(&mut self, value: T) -> Result<(), ChannelSendError<T>> {
        if self.sender_closed || self.receiver_closed {
            return Err(ChannelSendError::Closed(value));
        }
        if self.queue.len() >= self.capacity {
            return Err(ChannelSendError::Full(value));
        }
        self.queue.push_back(value);
        Ok(())
    }

    pub fn try_receive(&mut self) -> ChannelReceive<T> {
        if self.receiver_closed {
            return ChannelReceive::Closed;
        }
        if let Some(value) = self.queue.pop_front() {
            return ChannelReceive::Item(value);
        }
        if self.sender_closed {
            ChannelReceive::Closed
        } else {
            ChannelReceive::Empty
        }
    }
}
