use std::{
    collections::{HashMap, HashSet},
    hash::Hash,
    thread::JoinHandle,
};

use common_lib::{
    bindings::{ActionId, BindingPolicy, Bindings},
    input::{KeyCode, Modifiers},
};
use crossbeam_channel::{Receiver, SendError, Sender};

use crate::{client_gateway::FromLogicalDevice, device_interface::uinput::UInput};

use common_lib::ipc::message::{FromClient, Request};

/// A message to the logical device from a physical one.
#[derive(Clone, Debug)]
pub enum FromPhysicalDevice {
    /// Request actions from the logical device.
    Request(Vec<Request>),
    /// Close the connection.
    Close,
    /// Resync a device by setting the currently held keys for the device.
    Sync(HashSet<KeyCode>),
}

pub trait Id: Copy + Default + Eq + PartialEq + Hash {
    /// Generates the Id that comes after the current one.
    fn next(&mut self) -> Self;
}

/// Wrapper around a message to include sender id.
pub struct WrappedMessage<I: Id, M> {
    /// Id of the device sending the message.
    id: I,
    /// Message to send.
    message: M,
}
// TODO: Remove clone if each device only needs one sender.
#[derive(Clone)]
pub struct Tx<I: Id, M> {
    /// Id of the device that uses the sender.
    id: I,
    /// Sender channel.
    sender: Sender<WrappedMessage<I, M>>,
}
impl<I: Id, M> Tx<I, M> {
    fn new(id: I, sender: Sender<WrappedMessage<I, M>>) -> Self {
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
pub struct TxMaker<I: Id, M> {
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
pub type WrappedSimulatedMessage = WrappedMessage<SimulatedId, FromClient>;
/// Sender that ensures messages are wrapped with the simulated device's [`SimulatedId`].
pub type SimulatedTx = Tx<SimulatedId, FromClient>;
/// Creates simulated senders to the logical device with increasing ids.
pub type SimulatedTxMaker = TxMaker<SimulatedId, FromClient>;

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
/// Creates physical senders to the logical device with increasing ids.
pub type PhysicalTxMaker = TxMaker<PhysicalId, FromPhysicalDevice>;

/// What to execute when a key is released.
struct OnRelease {
    /// Whether to forward the key to the compositor or swallow it.
    policy: BindingPolicy,
    /// What [`ActionId`] need to be sent to what client with associated [`SimulatedId`].
    actions: HashMap<SimulatedId, Vec<ActionId>>,
}

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
    /// Sender to the gateway.
    gateway_tx: Sender<FromLogicalDevice>,

    /// List of subscribed clients for each [`ModdedKey`].
    // TODO: Maybe turn it into a single [`Bindings`] map that returns a list of [`ActionId`] and
    // associated [`SimulatedId`] on match.
    client_bindings: HashMap<SimulatedId, Bindings>,
    /// Set of clients that are currently paused.
    paused_clients: HashSet<SimulatedId>,
    /// Stores info about what to do when key is released.
    release_map: HashMap<KeyCode, OnRelease>,

    /// Used to create virtual devices and send inputs to the compositor.
    uinput: UInput,
}
impl LogicalDevice {
    pub fn new(gateway_tx: Sender<FromLogicalDevice>) -> Self {
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
