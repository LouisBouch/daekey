pub use evdev::KeyCode;
pub mod binder;
pub mod keys;
pub mod api;
pub mod display_monitor;
#[doc(hidden)]
pub mod message;
pub mod modifiers;


// instead of just hoding the binding in a hashmap, hold them in the trie.
//
// Order of things:
// user creates bindings (hashmap of keyevents with closures)
// user calls launch with the bindings
// process launches privileged process and creates stdin and stdout
// sends to stdin the serialized bindings
// new process loops listen on stdin
// when it reeives the bindings, it creates 2 new threads. It also starts listening for commands
// from the core process. It then sends the required commands to the threads:
// 1. waits for commands from the thread listening to stdin or from commands from the other input thread.
// 2. blocks on input and sends keybind code through stdout (keybind code is an integer which
//    represents a keyevent. the brain only holds integer and closures). If key has no bindings,
//    send the result directly to the uinput process through crossbeam.
//
// create some struct.
// Have this struct expose a function that takes in a keybind and a closure.
// Closure takes as a field another special struct.
// This special struct can be called to send keys to uinput directly.
// Under the hood, this special struct wraps a channel and it streamlines the usage of the channel.
// An now, instead of just calling the closure directly, the app needs to call it with the special struct every time.
// This special struct can easily be clone to allow multiple channels.
//
// Make user file a simple .rs file to import?
