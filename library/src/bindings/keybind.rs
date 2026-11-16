//! Strucs associated with keybinds.

use crate::{
    binding::BindingPolicy,
    bindings::action::{self, Action},
    keys::{KeyCode, modifiers::Modifiers},
};
use std::collections::{BTreeMap, HashMap, HashSet};

pub(crate) struct KeyBindings(pub(crate) HashMap<KeyCode, ModifierMap>);

#[derive(Default)]
/// Holds bindings associated to different modifiers.
pub struct ModifierMap {
    /// Bindings for modifiers tthat must match exactly.
    exact: HashMap<Modifiers, Edges>,
    /// Bindings for modifiers that must match exactly with at least the modifiers.
    at_least: AtLeastBindings<Edges>,
}
#[derive(Debug, Eq, PartialEq, Copy, Clone)]
struct AtLeastKey(Modifiers);
impl PartialOrd for AtLeastKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for AtLeastKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Sort in reverse order. First the [`Modifiers`] with more total modifires and then
        // arbirtarily using the bitmap value.
        other
            .0
            .modifier_count()
            .cmp(&self.0.modifier_count())
            .then_with(|| self.0.bits().cmp(&other.0.bits()))
    }
}
pub struct AtLeastBindings<T> {
    bindings: BTreeMap<AtLeastKey, T>,
}
impl<T> Default for AtLeastBindings<T> {
    fn default() -> Self {
        Self {
            bindings: Default::default(),
        }
    }
}
impl<T> AtLeastBindings<T> {
    pub fn new() -> Self {
        Default::default()
    }
    /// Add new modifier with its bindings.
    /// If it did not have this key present, `None` is returned.
    /// If it did have this key present, the value is updated, and the old one is returned.
    pub fn insert(&mut self, modifiers: Modifiers, value: T) -> Option<T> {
        self.bindings.insert(AtLeastKey(modifiers), value)
    }
    /// Get all values whose [`Modifiers`] are contained within the argument's [`Modifiers`].
    pub fn get_all(&self, modifiers: Modifiers) -> Vec<&T> {
        let mut list = Vec::new();
        for (&AtLeastKey(modi), edges) in self.bindings.iter() {
            if modifiers.contains(modi) {
                list.push(edges);
            }
        }
        list
    }
    /// Get all values that match the most amount of modifiers possible.
    pub fn get_most_specific(&self, modifiers: Modifiers) -> Vec<&T> {
        let mut most_matching = 0;
        let mut edges_list = Vec::new();
        for (&AtLeastKey(modi), edges) in self.bindings.iter() {
            if modifiers.contains(modi) {
                let count = modi.modifier_count();
                // Sorted in descending order, so just break when smaller count appears.
                if count >= most_matching {
                    most_matching = count;
                    edges_list.push(edges);
                } else {
                    break;
                }
            }
        }
        edges_list
    }
    /// Obtain a map of [`Modifiers`] where the key represents modifiers that, when pressed,
    /// result in colliding [`Modifiers`]. A collision here represents ambiguity in the bindign
    /// resolution.
    ///
    /// Essentially, each key is a binding that should be implemented to avoid ambiguity.
    ///
    /// Note that this function returns collisions that aren't currently bound but that should be to
    /// prevent other collisions. For examplle, if Shift, Ctrl and Alt are bound, it will return
    /// that Shift+Ctrl+Alt collides with Shift+Ctrl, Ctrl+Alt and Shift+Alt even though they are
    /// not bound yet. This is because Shift+Ctrl, Ctrl+Alt and Shift+Alt, SHOULD be bound to
    /// avoid the collision between Shift, Ctrl and Alt. It returns these anticipatory collisions to
    /// prevent you from having to run the algorithm multiple times.
    ///
    // # Examples
    // ```rust
    // use library::bindings::builder::{AtLeastBindings};
    // // Dummy value for the example.
    // let v = ();
    // let bindings = AtLeastBindings::new();
    // let shift = Modifiers::LEFT_SHIFT;
    // let ctrl = Modifiers::LEFT_CTRL;
    // let alt = Modifiers::LEFT_ALT;
    //
    // bindings.insert(shift, v);
    // bindings.insert(ctrl, v);
    // let collisions = bindings.collisions();
    // let shift_ctrl_collision = collisions.get(&(shift | ctrl)).unwrap();
    // assert!(shift_ctrl_collision.contains(&shift) && shift_ctrl_collision.contains(&ctrl));
    //
    // bindings.insert(alt, v);
    // bindings.insert(shift | ctrl, v);
    // let collisions = bindings.collisions();
    // let alt_shift_ctrl_collision = collisions.get(&(shift | alt | ctrl));
    // let alt_ctrl_collision = collisions.get(&(alt | ctrl)).unwrap();
    // // Empty because Shift+Ctrl matches and is more specific than the other bindings.
    // assert!(alt_ctrl_collision.is_none());
    // assert!(alt_ctrl_collision.contains(&alt) && alt_ctrl_collision.contains(&ctrl));
    // ```
    pub fn collisions(&self) -> HashMap<Modifiers, HashSet<Modifiers>> {
        let mut collisions = HashMap::new();
        #[derive(Debug, PartialEq, Eq)]
        enum BindState {
            Bound,
            NotBound,
        }
        use BindState::{Bound, NotBound};
        let mut modifier_layers: Vec<BTreeMap<AtLeastKey, BindState>> = Vec::new();
        // Fill array with arrays of modifiers with the same modifier count.
        for (&AtLeastKey(modifiers), _) in self.bindings.iter() {
            let count = modifiers.modifier_count();
            while modifier_layers.len() < count as usize {
                modifier_layers.push(BTreeMap::new());
            }
            modifier_layers[count as usize - 1].insert(AtLeastKey(modifiers), Bound);
        }
        // Build the collisions map.
        let mut mod_layer_index = 0;
        // for mod_layer_index in 0..modifier_layers.len() {
        while mod_layer_index < modifier_layers.len() {
            let mod_layer = std::mem::take(&mut modifier_layers[mod_layer_index]);
            let len = mod_layer.len();
            dbg!(&mod_layer);
            for (i, (AtLeastKey(mod_i), _)) in mod_layer.iter().take(len - 1).enumerate() {
                let start = i + 1;
                for (AtLeastKey(mod_o), _) in mod_layer.iter().skip(start).take(len - start) {
                    let permutation = *mod_i | *mod_o;
                    let permutation_mod_count = permutation.modifier_count() as usize;
                    while modifier_layers.len() < permutation_mod_count {
                        modifier_layers.push(BTreeMap::new());
                    }
                    let layer = &mut modifier_layers[permutation_mod_count - 1];

                    if *layer.entry(AtLeastKey(permutation)).or_insert(NotBound) == NotBound {
                        let perm_collisions =
                            collisions.entry(permutation).or_insert(HashSet::new());
                        perm_collisions.insert(*mod_i);
                        perm_collisions.insert(*mod_o);
                    }
                }
            }
            mod_layer_index += 1;
        }
        dbg!(&collisions);
        collisions
    }
}
/// The actions and policies associated with each edge of a key stroke (press and release).
struct Edges {
    /// Action on press, if any.
    press: Option<Action>,
    /// Action on release, if any.
    release: Option<Action>,
    /// Policy of the binding.
    policy: BindingPolicy,
    /// Whether the binding should be active when the bindings are paused.
    active_when_paused: bool,
}
impl Default for Edges {
    fn default() -> Self {
        Self {
            press: Default::default(),
            release: Default::default(),
            policy: Default::default(),
            active_when_paused: false,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_colisions_three_single_and_one_double_modifiers() {
        let v = ();
        let mut bindings = AtLeastBindings::new();
        let shift = Modifiers::LEFT_SHIFT;
        let ctrl = Modifiers::LEFT_CTRL;
        let alt = Modifiers::LEFT_ALT;
        bindings.insert(shift, v);
        bindings.insert(ctrl, v);
        bindings.insert(alt, v);
        bindings.insert(shift | ctrl, v);

        let collisions = bindings.collisions();

        let alt_shift_ctrl_collision = collisions.get(&(shift | alt | ctrl)).unwrap();
        assert!(alt_shift_ctrl_collision.contains(&(shift | alt)));
        assert!(alt_shift_ctrl_collision.contains(&(shift | ctrl)));
        assert!(alt_shift_ctrl_collision.contains(&(alt | ctrl)));

        let alt_ctrl_collision = collisions.get(&(alt | ctrl)).unwrap();
        assert!(alt_ctrl_collision.contains(&alt));
        assert!(alt_ctrl_collision.contains(&ctrl));

        let shift_ctrl_collision = collisions.get(&(shift | ctrl));
        assert!(shift_ctrl_collision.is_none());
    }
    #[test]
    fn test_colisions_three_single_modifiers() {
        let v = ();
        let mut bindings = AtLeastBindings::new();
        let shift = Modifiers::LEFT_SHIFT;
        let ctrl = Modifiers::LEFT_CTRL;
        let alt = Modifiers::LEFT_ALT;
        bindings.insert(shift, v);
        bindings.insert(ctrl, v);
        bindings.insert(alt, v);

        let collisions = bindings.collisions();
        // dbg!(&collisions);

        let alt_ctrl_collision = collisions.get(&(alt | ctrl)).unwrap();
        assert!(alt_ctrl_collision.contains(&ctrl));
        assert!(alt_ctrl_collision.contains(&alt));

        let alt_shift_ctrl_collision = collisions.get(&(shift | alt | ctrl)).unwrap();
        assert!(alt_shift_ctrl_collision.contains(&(shift | ctrl)));
        assert!(alt_shift_ctrl_collision.contains(&(alt | ctrl)));
        assert!(alt_shift_ctrl_collision.contains(&(shift | alt)));
    }
    #[test]
    fn test_colisions_two_single_modifiers() {
        let v = ();
        let mut bindings = AtLeastBindings::new();
        let shift = Modifiers::LEFT_SHIFT;
        let ctrl = Modifiers::LEFT_CTRL;

        bindings.insert(shift, v);
        bindings.insert(ctrl, v);
        let collisions = bindings.collisions();
        let shift_ctrl_collision = collisions.get(&(shift | ctrl)).unwrap();
        assert!(shift_ctrl_collision.contains(&shift));
        assert!(shift_ctrl_collision.contains(&ctrl));
    }
}
