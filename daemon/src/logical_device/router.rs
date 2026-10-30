use std::{collections::HashMap, hash::Hash, thread::JoinHandle};

use crossbeam_channel::{Receiver, SendError, Sender};
use library::{
    binding::BindingPolicy,
    keys::{KeyCode, modded_key::ModdedKey, modifiers::Modifiers},
};

use crate::client_gateway::LogicalDeviceMessage;

use library::ipc::message::{FromProcess, Request};

/// A message to the logical device from a physical one.
#[derive(Clone, Debug)]
pub enum FromPhysicalDevice {
    /// Request actions from the logical device.
    Request(Vec<Request>),
    /// Close the connection.
    Close,
}

trait Id: Copy + Default + Eq + PartialEq + Hash {
    /// Generates the Id that comes after the current one.
    fn next(&mut self) -> Self;
}

/// Wrapper around a message to include sender id.
struct WrappedMessage<I: Id, M> {
    /// Id of the device sending the message.
    id: I,
    /// Message to send.
    message: M,
}
// TODO: Remove clone if each device only needs one sender.
#[derive(Clone)]
struct Tx<I: Id, M> {
    /// Id of the device that uses the sender.
    id: I,
    /// Sender channel.
    sender: Sender<WrappedMessage<I, M>>,
}
impl<I: Id, M> Tx<I, M> {
    pub fn new(id: I, sender: Sender<WrappedMessage<I, M>>) -> Self {
        Self { id, sender }
    }
    pub fn send(&self, message: M) -> Result<(), SendError<WrappedMessage<I, M>>> {
        self.sender.send(WrappedMessage {
            id: self.id,
            message,
        })
    }
}
/// Creates senders with increasing ids.
struct TxMaker<I: Id, M> {
    /// Id of the next device that will use the sender.
    next_id: I,
    /// Sender channel.
    sender: Sender<WrappedMessage<I, M>>,
}
impl<I: Id, M> TxMaker<I, M> {
    fn new(sender: Sender<WrappedMessage<I, M>>) -> Self {
        Self {
            sender,
            next_id: Default::default(),
        }
    }
    /// Generate a new sender and increment the id for the next sender.
    pub fn generate_tx(&mut self) -> Tx<I, M> {
        let tx = Tx {
            id: self.next_id,
            sender: self.sender.clone(),
        };
        self.next_id = self.next_id.next();
        tx
    }
    /// The id that will be given to the next channel.
    pub fn next_id(&self) -> I {
        self.next_id
    }
}

/// A simulated device's id. Comes from client requests.
/// A device of this type will WILL NOT trigger bindings.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct SimulatedId(u64);
impl Id for SimulatedId {
    fn next(&mut self) -> SimulatedId {
        SimulatedId(self.0 + 1)
    }
}
/// Wraps a message for the logical device with the incoming simulated device's [`SimulatedId`].
pub type WrappedSimulatedMessage = WrappedMessage<SimulatedId, FromProcess>;
/// Sender that ensures messages are wrapped with the simulated device's [`SimulatedId`].
pub type SimulatedTx = Tx<SimulatedId, FromProcess>;

/// A physical device's id. Comes from an actual physical device.
/// A device of this type will WILL trigger bindings.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
pub struct PhysicalId(u64);
impl Id for PhysicalId {
    fn next(&mut self) -> PhysicalId {
        PhysicalId(self.0 + 1)
    }
}

/// Wraps a message for the logical device with the incoming physical device's [`PhysicalId`].
pub type WrappedPhysicalMessage = WrappedMessage<PhysicalId, FromPhysicalDevice>;
/// Sender that ensures messages are wrapped with the physical device's [`PhysicalId`].
pub type PhysicalTx = Tx<PhysicalId, FromPhysicalDevice>;

/// The state of the key when it was first pressed.
#[derive(PartialEq, Eq, Clone)]
struct Latch {
    /// The [`BindingPolicy`] of the first press.
    policy: BindingPolicy,
    /// The [`Modifiers`] of the first press.
    modifiers: Modifiers,
    /// Clients that have a release binding on the corresponding [`ModdedKey`].
    /// This is to ensure clients who just started the script won't
    /// receive an up binding if they were not there for the down binding.
    release_targets: Vec<SimulatedId>,
}
/// Related to the motion of the key, either press or release.
struct Edge {
    /// If true, the key ignores the pause state of the client when the edge triggers.
    active_when_paused: bool,
}
impl Edge {
    /// If true, the edge is considered active.
    ///
    /// # Arguments
    ///
    /// * `paused` - True if the client requested a binding pause.
    fn active(&self, paused: bool) -> bool {
        !paused || self.active_when_paused
    }
}
/// Info related the the subcription of a client to a [`ModdedKey`].
struct Subscription {
    /// Whether to forard or swallow the key on activation.
    policy: BindingPolicy,
    /// Some if press is bound, None otherwise.
    press: Option<Edge>,
    /// Some if release is bound, None otherwise.
    release: Option<Edge>,
}
impl Subscription {
    /// If returns true, the press binding should be executed.
    ///
    /// # Arguments
    ///
    /// * `paused` - True if the client requested a binding pause.
    fn run_press_binding(&self, paused: bool) -> bool {
        self.press.as_ref().is_some_and(|e| e.active(paused))
    }
    /// If returns true, the release binding should be executed.
    ///
    /// # Arguments
    ///
    /// * `paused` - True if the client requested a binding pause.
    fn run_release_binding(&self, paused: bool) -> bool {
        self.release.as_ref().is_some_and(|e| e.active(paused))
    }
    /// Returns true if the key should be swallowed because of the subscription.
    fn swallow(&self, paused: bool) -> bool {
        self.policy == BindingPolicy::Swallow
            && (!paused
                || self.press.as_ref().is_some_and(|e| e.active_when_paused)
                || self.release.as_ref().is_some_and(|e| e.active_when_paused))
    }
}
pub type SimulatedTxMaker = TxMaker<SimulatedId, FromProcess>;
pub type PhysicalTxMaker = TxMaker<SimulatedId, FromProcess>;
/// Contents needed for the logical device runner to track and communicate with the logical device.
pub struct LogicalDeviceHandle {
    /// Handle for the running thread of the logical device.
    pub handle: JoinHandle<()>,
    /// A [`TxMaker`] for simulated devices.
    pub simulated_tx_maker: SimulatedTxMaker,
    /// A [`TxMaker`] for physical devices.
    pub physical_tx_maker: PhysicalTxMaker,
}

/// Acts as a singular device by managing all logical and physical requests.
pub struct LogicalDevice {
    /// Receiver for simulated device requests.
    simulated_rx: Receiver<WrappedSimulatedMessage>,
    /// Receiver for physical device requests.
    physical_rx: Receiver<WrappedPhysicalMessage>,
    /// Sender for the gateway.
    gateway_tx: Sender<LogicalDeviceMessage>,

    /// List of subscribed clients for each [`ModdedKey`].
    client_subscription_map: HashMap<ModdedKey, HashMap<SimulatedId, Subscription>>,
    /// The value for a client is true if it is currently paused.
    /// If the id is not in the map, it means the client associated with the [`SimulatedId`] is dead.
    client_is_paused: HashMap<SimulatedId, bool>,
    /// Stores info about the state of the key when it was pressed.
    key_latches: HashMap<KeyCode, Latch>,
}
impl LogicalDevice {
    pub fn new() -> Self {
        todo!()
    }
    /// Start the logical device and return one [`TxMaker`] for each type of devices alongside the
    /// [`JoinHandle`] for the thread that will run.
    pub fn launch(self) -> LogicalDeviceHandle {
        todo!()
    }

    /// Handle message coming from simulated device.
    fn handle_physical_device_message(&mut self, message: WrappedPhysicalMessage) {
        todo!()
    }
    /// Handle message coming from simulated device.
    fn handle_simulated_device_message(&mut self, message: WrappedSimulatedMessage) {
        todo!()
    }
}
