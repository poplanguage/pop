//! Backend-neutral local actor incarnation and bounded mailbox lifecycle.

use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ActorId(u64);

impl ActorId {
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ActorIncarnation(u64);

impl ActorIncarnation {
    #[must_use]
    pub const fn new(raw: u64) -> Self {
        Self(raw)
    }

    #[must_use]
    pub const fn raw(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ActorReference {
    actor: ActorId,
    incarnation: ActorIncarnation,
}

impl ActorReference {
    #[must_use]
    pub const fn new(actor: ActorId, incarnation: ActorIncarnation) -> Self {
        Self { actor, incarnation }
    }

    #[must_use]
    pub const fn actor(self) -> ActorId {
        self.actor
    }

    #[must_use]
    pub const fn incarnation(self) -> ActorIncarnation {
        self.incarnation
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorExit {
    Completed,
    Cancelled,
    Panicked,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorState {
    Starting,
    Running,
    Stopping(ActorExit),
    Exited(ActorExit),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActorSendError<T> {
    Full(T),
    Closed(T),
    Stale(T),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActorReceive<T> {
    Message(T),
    Empty,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActorLifecycleError {
    AlreadyActive(ActorReference),
    AlreadyStopping(ActorReference),
    AlreadyExited(ActorReference),
    NotStopping(ActorReference),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActorLifecycle<T> {
    reference: ActorReference,
    capacity: u64,
    mailbox: VecDeque<T>,
    state: ActorState,
}

impl<T> ActorLifecycle<T> {
    #[must_use]
    pub fn starting(actor: ActorId, incarnation: ActorIncarnation, capacity: u64) -> Self {
        Self {
            reference: ActorReference::new(actor, incarnation),
            capacity,
            mailbox: VecDeque::new(),
            state: ActorState::Starting,
        }
    }

    #[must_use]
    pub const fn reference(&self) -> ActorReference {
        self.reference
    }

    #[must_use]
    pub const fn capacity(&self) -> u64 {
        self.capacity
    }

    #[must_use]
    pub fn length(&self) -> u64 {
        u64::try_from(self.mailbox.len()).unwrap_or(u64::MAX)
    }

    #[must_use]
    pub const fn state(&self) -> ActorState {
        self.state
    }

    /// Publishes a successfully initialized actor incarnation.
    ///
    /// # Errors
    ///
    /// Returns the exact invalid lifecycle transition without changing state.
    pub fn activate(&mut self) -> Result<(), ActorLifecycleError> {
        match self.state {
            ActorState::Starting => {
                self.state = ActorState::Running;
                Ok(())
            }
            ActorState::Running => Err(ActorLifecycleError::AlreadyActive(self.reference)),
            ActorState::Stopping(_) => Err(ActorLifecycleError::AlreadyStopping(self.reference)),
            ActorState::Exited(_) => Err(ActorLifecycleError::AlreadyExited(self.reference)),
        }
    }

    /// Admits one already copied message into this exact actor incarnation.
    ///
    /// # Errors
    ///
    /// Returns the exact unconsumed message when the reference is stale, the
    /// actor is unavailable, or the bounded mailbox is full.
    pub fn try_admit(
        &mut self,
        reference: ActorReference,
        copied_message: T,
    ) -> Result<(), ActorSendError<T>> {
        if reference != self.reference {
            return Err(ActorSendError::Stale(copied_message));
        }
        if self.state != ActorState::Running {
            return Err(ActorSendError::Closed(copied_message));
        }
        if u64::try_from(self.mailbox.len()).map_or(true, |length| length >= self.capacity) {
            return Err(ActorSendError::Full(copied_message));
        }
        self.mailbox.push_back(copied_message);
        Ok(())
    }

    pub fn try_receive(&mut self) -> ActorReceive<T> {
        if self.state != ActorState::Running {
            return ActorReceive::Closed;
        }
        self.mailbox
            .pop_front()
            .map_or(ActorReceive::Empty, ActorReceive::Message)
    }

    /// Stops admission and returns queued messages to the owning runtime for
    /// exact managed-root cleanup.
    ///
    /// # Errors
    ///
    /// Returns the exact invalid lifecycle transition without changing state.
    pub fn begin_exit(&mut self, exit: ActorExit) -> Result<Vec<T>, ActorLifecycleError> {
        match self.state {
            ActorState::Starting | ActorState::Running => {
                self.state = ActorState::Stopping(exit);
                Ok(self.mailbox.drain(..).collect())
            }
            ActorState::Stopping(_) => Err(ActorLifecycleError::AlreadyStopping(self.reference)),
            ActorState::Exited(_) => Err(ActorLifecycleError::AlreadyExited(self.reference)),
        }
    }

    /// Publishes terminal exit only after child-task and resource cleanup.
    ///
    /// # Errors
    ///
    /// Returns the exact invalid lifecycle transition without changing state.
    pub fn complete_exit(&mut self) -> Result<ActorExit, ActorLifecycleError> {
        match self.state {
            ActorState::Stopping(exit) => {
                self.state = ActorState::Exited(exit);
                Ok(exit)
            }
            ActorState::Exited(_) => Err(ActorLifecycleError::AlreadyExited(self.reference)),
            ActorState::Starting | ActorState::Running => {
                Err(ActorLifecycleError::NotStopping(self.reference))
            }
        }
    }
}
