//! Strucs associated with bindings.

use serde::{Deserialize, Serialize};

use crate::input::{KeyCode, ModifierMatch, Modifiers};
use std::collections::{BTreeMap, HashMap, HashSet};

/// Id for an action.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct ActionId(u64);
impl ActionId {
    pub fn new(id: u64) -> Self {
        ActionId(id)
    }
    pub fn id(&self) -> u64 {
        self.0
    }
}
/// What to do with the event if it is bound.
#[derive(Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy)]
pub enum BindingPolicy {
    /// Trigger the binding and then forward the key to the compositor.
    Forward,
    /// Trigger the binding but do no forward to compositor.
    Swallow,
}
impl Default for BindingPolicy {
    fn default() -> Self {
        Self::Swallow
    }
}
/// A struct that wraps what a binding needs.
#[derive(Debug)]
pub struct Binding {
    /// Key for the binding.
    pub key: KeyCode,
    /// Required modifiers for the binding.
    pub modifiers: ModifierMatch,
    /// Policy of the binding.
    pub policy: BindingPolicy,
    /// `true` if the binding should run when the client is paused.
    pub active_when_paused: bool,
    /// Id of the action for a press on the client side.
    pub on_press: Option<ActionId>,
    /// Id of the action for a release on the client side.
    pub on_release: Option<ActionId>,
}
/// Bindings associated with each key.
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Bindings(HashMap<KeyCode, ModifierMap<Edges>>);
impl Bindings {
    pub fn new() -> Self {
        Self(HashMap::new())
    }
    pub fn nb_bindings(&self) -> usize {
        let mut nb_bindings = 0;
        for mod_map in self.0.values() {
            nb_bindings += mod_map.exact.len() + mod_map.at_least.bindings.len();
        }
        nb_bindings
    }
    /// Insert binding into the struct.
    ///
    /// Returns previous [`Binding`] associated with key+modifiers, if any.
    pub fn insert(&mut self, binding: Binding) -> Option<Binding> {
        let Binding {
            key,
            modifiers,
            policy,
            active_when_paused,
            on_press,
            on_release,
        } = binding;
        let edges = Edges {
            press: on_press,
            release: on_release,
            policy: policy,
            active_when_paused: active_when_paused,
        };
        let mod_map = self.0.entry(key).or_default();
        mod_map.insert(modifiers, edges).map(|v| {
            let Edges {
                press,
                release,
                policy,
                active_when_paused,
            } = v;
            Binding {
                key,
                modifiers,
                policy,
                active_when_paused,
                on_press: press,
                on_release: release,
            }
        })
    }
    /// Collisions between at-least modifier bindings. Each map key represents a combination of
    /// modifiers with key that could be pressed.
    /// The associated value describes at-least modifier bindings that conflict with the pressed modifiers.
    /// Also contains collisions that are not yet present but that will be once the previous
    /// collisions are handled.
    ///
    /// For examplle, if Shift, Ctrl and Alt are bound, it will return
    /// that Shift+Ctrl collides with shift and ctrl, Ctrl+Alt collides with Ctrl and Alt and that Shift+Alt collide with Shift and Alt.
    /// It will also include Shift+Ctrl+Alt because without it,
    pub fn collisions(&self) -> HashMap<(KeyCode, Modifiers), HashSet<Modifiers>> {
        let mut map = HashMap::new();
        for (keycode, modifiers_map) in self.0.iter() {
            let collsions_map = modifiers_map.collisions();
            for (modifiers, collision_set) in collsions_map {
                map.insert((*keycode, modifiers), collision_set);
            }
        }
        map
    }
}
//TODO: Maybe make moudle from ModifierMap<T> with its content (not Edges cause its not its content
//before someone sets T to be Edges)

#[derive(Debug, PartialEq, Eq, Clone)]
enum BindState {
    Bound,
    ToBind,
}
/// Map of at-least and exact modifiers and their associated value.
#[derive(Serialize, Deserialize, Clone, Debug)]
struct ModifierMap<T> {
    /// Bindings for modifiers that must match exactly.
    exact: HashMap<Modifiers, T>,
    /// Bindings for modifiers that must match with at least these modifiers.
    at_least: AtLeastBindings<T>,
}
impl<T> ModifierMap<T> {
    pub fn new() -> Self {
        Default::default()
    }
    /// Insert bindings with associated [`ModifierMatch`].
    pub fn insert(&mut self, modifiers: ModifierMatch, v: T) -> Option<T> {
        match modifiers {
            ModifierMatch::AtLeast(m) => self.at_least.insert(m, v),
            ModifierMatch::Exact(m) => self.exact.insert(m, v),
        }
    }
    /// Check whether a [`ModifierMatch`] is contained in the [`ModifierMap`].
    pub fn contains_key(&self, modifiers: &ModifierMatch) -> bool {
        match modifiers {
            ModifierMatch::AtLeast(modifiers) => self.at_least.contains_key(modifiers),
            ModifierMatch::Exact(modifiers) => self.exact.contains_key(modifiers),
        }
    }
    /// Collisions between at-least modifier bindings. Each map key represents a combination of
    /// modifiers with key that could be pressed.
    /// The associated value describes at-least modifier bindings that conflict with the pressed modifiers.
    /// Also contains collisions that are not yet present but that will be once the previous
    /// collisions are handled.
    ///
    /// For examplle, if Shift, Ctrl and Alt are bound, it will return
    /// that Shift+Ctrl collides with shift and ctrl, Ctrl+Alt collides with Ctrl and Alt and that Shift+Alt collide with Shift and Alt.
    /// It will also include Shift+Ctrl+Alt because without it,
    pub fn collisions(&self) -> HashMap<Modifiers, HashSet<Modifiers>> {
        let mut collisions: HashMap<Modifiers, HashSet<Modifiers>> = HashMap::new();
        use BindState::{Bound, ToBind};
        let mut at_least_mod_layers: Vec<BTreeMap<AtLeastKey, BindState>> = Vec::new();
        // Fill array with arrays of modifiers with the same modifier count.
        for (&AtLeastKey(modifiers), _) in self.at_least.bindings.iter() {
            let count = modifiers.modifier_count() as usize;
            // Cannot conflict with no modifiers.
            if count == 0 {
                continue;
            }
            if at_least_mod_layers.len() < count {
                at_least_mod_layers.resize(count, BTreeMap::new());
            }
            at_least_mod_layers[count as usize - 1].insert(AtLeastKey(modifiers), Bound);
        }
        // Build the collisions map between AtLeast bindings.
        let mut mod_layer_index = 0;
        while mod_layer_index < at_least_mod_layers.len() {
            let mod_layer = std::mem::take(&mut at_least_mod_layers[mod_layer_index]);
            let len = mod_layer.len();
            for (i, (AtLeastKey(mod_i), _)) in
                mod_layer.iter().take(len.saturating_sub(1)).enumerate()
            {
                let start = i + 1;
                for (AtLeastKey(mod_o), _) in mod_layer.iter().skip(start).take(len - start) {
                    let union = *mod_i | *mod_o;
                    let union_mod_count = union.modifier_count() as usize;
                    if at_least_mod_layers.len() < union_mod_count {
                        at_least_mod_layers.resize(union_mod_count, BTreeMap::new());
                    }
                    if Self::union_needs_binding(*mod_i, *mod_o, &at_least_mod_layers, &self.exact)
                    {
                        let layer = &mut at_least_mod_layers[union_mod_count - 1];
                        let entry = layer.entry(AtLeastKey(union)).or_insert(ToBind);
                        assert_eq!(*entry, ToBind, "bound union {union:?} should be covered");
                        let perm_collisions = collisions.entry(union).or_default();
                        perm_collisions.insert(*mod_i);
                        perm_collisions.insert(*mod_o);
                    }
                }
            }
            mod_layer_index += 1;
        }

        collisions
    }
    /// Checks whether the given exact and AtLeast modifiers prevent collisions for a specific modifiers union.
    /// If they do not prevent collisions, return `true`, if they do prevent collisions return `false`.
    ///
    /// Note that the modifiers must have the same count of individual modifiers.
    fn union_needs_binding(
        a: Modifiers,
        b: Modifiers,
        at_least: &[BTreeMap<AtLeastKey, BindState>],
        exact_bindings: &HashMap<Modifiers, T>,
    ) -> bool {
        let union = a | b;
        let ind_count = a.modifier_count() as usize;

        // Checks if a more specific AtLeast binding already covers the modifiers.
        let has_at_least_cover = |modi: Modifiers| {
            for tree in at_least
                .iter()
                .skip(ind_count)
                .take(modi.modifier_count() as usize - ind_count)
            {
                if tree.iter().any(|(key, state)| {
                    modi.contains(key.0) && !(key.0 == union && *state == BindState::ToBind)
                }) {
                    return true;
                }
            }
            false
        };
        let mut traversal = vec![union];
        let mut seen = HashSet::new();
        while let Some(modi) = traversal.pop() {
            if has_at_least_cover(modi) {
                continue;
            }
            if exact_bindings.contains_key(&modi) {
                for single_modifier in Modifiers::ALL_LIST {
                    if !modi.contains(single_modifier) {
                        let next = modi | single_modifier;
                        if seen.insert(next) {
                            traversal.push(next);
                        }
                    }
                }
            } else {
                return true;
            }
        }
        false
    }
}
impl<T> Default for ModifierMap<T> {
    fn default() -> Self {
        Self {
            exact: Default::default(),
            at_least: Default::default(),
        }
    }
}
#[derive(Serialize, Deserialize, Debug, Eq, PartialEq, Copy, Clone)]
struct AtLeastKey(Modifiers);
impl PartialOrd for AtLeastKey {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for AtLeastKey {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        // Sort in reverse order. First the [`Modifiers`] with more total modifiers and then
        // arbirtarily using the bitmap value.
        other
            .0
            .modifier_count()
            .cmp(&self.0.modifier_count())
            .then_with(|| self.0.bits().cmp(&other.0.bits()))
    }
}
#[derive(Serialize, Deserialize, Clone, Debug)]
struct AtLeastBindings<T> {
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
    fn new() -> Self {
        Default::default()
    }
    /// Add new modifier with its bindings.
    /// If it did not have this key present, `None` is returned.
    /// If it did have this key present, the value is updated, and the old one is returned.
    fn insert(&mut self, modifiers: Modifiers, value: T) -> Option<T> {
        self.bindings.insert(AtLeastKey(modifiers), value)
    }
    /// Check if key is already inserted.
    fn contains_key(&self, modifiers: &Modifiers) -> bool {
        self.bindings.contains_key(&AtLeastKey(*modifiers))
    }
    /// Get all values whose [`Modifiers`] are contained within the argument's [`Modifiers`].
    fn get_all(&self, modifiers: Modifiers) -> Vec<&T> {
        let mut list = Vec::new();
        for (&AtLeastKey(modi), edges) in self.bindings.iter() {
            if modifiers.contains(modi) {
                list.push(edges);
            }
        }
        list
    }
    /// Get all values that match the most amount of modifiers possible.
    fn get_most_specific(&self, modifiers: Modifiers) -> Vec<&T> {
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
    /// Get the first binding that matches, prioritizing bindings with more modifiers.
    fn get_first_match(&self, modifiers: Modifiers) -> Option<&T> {
        for (&AtLeastKey(modi), v) in self.bindings.iter() {
            if modifiers.contains(modi) {
                return Some(&v);
            }
        }
        None
    }
}
/// The actions and policies associated with each edge of a key stroke (press and release).
#[derive(Serialize, Deserialize, Clone, Debug)]
struct Edges {
    /// Action on press, if any.
    press: Option<ActionId>,
    /// Action on release, if any.
    release: Option<ActionId>,
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
    use ModifierMatch::{AtLeast, Exact};

    #[test]
    fn test_colisions_three_single_and_one_double_modifiers() {
        let v = ();
        let mut bindings = ModifierMap::new();
        let shift = Modifiers::LEFT_SHIFT;
        let ctrl = Modifiers::LEFT_CTRL;
        let alt = Modifiers::LEFT_ALT;
        bindings.insert(AtLeast(shift), v);
        bindings.insert(AtLeast(ctrl), v);
        bindings.insert(AtLeast(alt), v);
        bindings.insert(AtLeast(shift | ctrl), v);
        bindings.insert(AtLeast(Modifiers::NONE), v);

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
        let mut bindings = ModifierMap::new();
        let shift = Modifiers::LEFT_SHIFT;
        let ctrl = Modifiers::LEFT_CTRL;
        let alt = Modifiers::LEFT_ALT;
        bindings.insert(AtLeast(shift), v);
        bindings.insert(AtLeast(ctrl), v);
        bindings.insert(AtLeast(alt), v);

        let collisions = bindings.collisions();

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
        let mut bindings = ModifierMap::new();
        let shift = Modifiers::LEFT_SHIFT;
        let ctrl = Modifiers::LEFT_CTRL;

        bindings.insert(AtLeast(shift), v);
        bindings.insert(AtLeast(ctrl), v);

        let collisions = bindings.collisions();
        let shift_ctrl_collision = collisions.get(&(shift | ctrl)).unwrap();
        assert!(shift_ctrl_collision.contains(&shift));
        assert!(shift_ctrl_collision.contains(&ctrl));
    }
}
