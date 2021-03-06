extern crate nalgebra as na;
extern crate num;

use na::{Norm, FloatPnt, FloatVec};
use num::Zero;
use std::cmp;
use std::fmt::Debug;

/// Wraps a square distance.
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd)]
pub struct SqDist(pub f32);

impl SqDist {
    pub fn from_dist(d: f32) -> SqDist {
        SqDist(d.powi(2))
    }
}

/// What to do when a node `connects` with an attrator.
#[derive(Debug, Copy, Clone)]
pub enum ConnectAction {
    KillAttractor,
    DisableFor {
        iterations: u32,
    },
    DisableForConnectingRoot,
}

#[derive(Debug, Copy, Clone)]
pub struct Attractor<P, I: Copy> {
    /// The square distance within which it can influence a Node.
    pub attract_dist: SqDist,

    /// If there is a node closer than the square root of
    /// this distance, the information is exchanged with the
    /// node and the ```connect_action``` is performed.
    /// This can be for example: kill the attractor,
    /// or disable it for a while.
    pub connect_dist: SqDist,

    /// The strenght with which it influences a Node.
    pub strength: f32,

    /// The position of the attractor.
    pub position: P,

    /// The attractor carries a bit of information.
    /// When a node comes closer than ```connect_radius```
    /// this bit of information is exchanged.
    pub information: I,

    /// Action performed when a node comes closer
    /// than ```connect_radius```.
    pub connect_action: ConnectAction,

    /// Starting from which iteration this attractor is active
    pub active_from_iteration: u32,

    /// When set, this denies nodes of trees rooted at the specified
    /// NodeIdx to be attracted by this attractor. This allows to 
    /// simultaneous grow connects from Nodes to other Nodes without
    /// suffering from self-attraction. 
    pub not_for_root: Option<NodeIdx>,

    /// Same as not_for_root, but this is used by ConnectAction::DisableForConnectingRoot
    pub not_for_connecting_root: Option<NodeIdx>,
}

impl<P, I: Copy> Attractor<P, I> {
    fn is_active_in(&self, current_iteration: u32) -> bool {
        current_iteration >= self.active_from_iteration
    }

    fn disable_until(&mut self, iteration: u32) {
        self.active_from_iteration = iteration;
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct NodeIdx(pub u32);

#[derive(Debug)]
pub struct Node<P, F, I>
    where P: Debug,
          F: Debug,
          I: Copy + Debug
{
    /// Index of the direct parent.
    parent: NodeIdx,

    /// Index of the root node this node is associated with.
    pub root: NodeIdx,

    /// Number of nodes between this node and the root node.
    pub length: u32,

    /// Number of branches this node has. This count
    /// is increased whenever another node refers this node
    /// as parent.
    pub branches: u32,

    /// The node's coordinate position.
    pub position: P,

    /// Calculates the direction in which a new node is grown.
    /// This value is reset every iteration.
    growth: F,

    /// Number of attractors that this node is attracted by.
    growth_count: u32,

    /// For example an attractor could
    pub assigned_information: Option<I>,
}

impl<P, F, I> Node<P, F, I>
    where P: Debug,
          F: Debug,
          I: Copy + Debug
{
    fn transmit_information(&mut self, information: I) {
        self.assigned_information = Some(information);
    }

    pub fn is_leaf(&self) -> bool {
        self.branches == 0
    }

    pub fn is_root(&self) -> bool {
        if self.length == 0 {
            assert!(self.root == self.parent);
            true
        } else {
            false
        }
    }

    fn is_active(&self, max_length: u32, max_branches: u32) -> bool {
        self.length < max_length && self.branches < max_branches
    }
}

pub struct SpaceColonization<P, F, I>
    where P: FloatPnt<f32, F> + Debug,
          F: FloatVec<f32> + Zero + Copy + Debug,
          I: Copy + Default + Debug
{
    nodes: Vec<Node<P, F, I>>,
    attractors: Vec<Attractor<P, I>>,
    default_attract_dist: SqDist,
    default_connect_dist: SqDist,
    move_dist: f32,
    next_iteration: u32,
    max_length: u32,
    max_branches: u32,
    use_last_n_nodes: Option<usize>,
}

impl<P, F, I> SpaceColonization<P, F, I>
    where P: FloatPnt<f32, F> + Debug,
          F: FloatVec<f32> + Zero + Copy + Debug,
          I: Copy + Default + Debug
{
    pub fn new(default_attract_dist: SqDist,
               default_connect_dist: SqDist,
               max_length: u32,
               max_branches: u32,
               move_dist: f32)
               -> SpaceColonization<P, F, I> {
        SpaceColonization {
            nodes: Vec::new(),
            attractors: Vec::new(),
            default_attract_dist: default_attract_dist,
            default_connect_dist: default_connect_dist,
            max_length: max_length,
            max_branches: max_branches,
            move_dist: move_dist,
            next_iteration: 0,
            use_last_n_nodes: None, // XXX
        }
    }

    pub fn add_attractor(&mut self, attractor: Attractor<P, I>) {
        self.attractors.push(attractor);
    }

    pub fn add_default_attractor(&mut self, position: P) {
        self.attractors.push(Attractor {
            attract_dist: self.default_attract_dist,
            connect_dist: self.default_connect_dist,
            strength: 1.0,
            position: position,
            information: I::default(),
            connect_action: ConnectAction::KillAttractor,
            active_from_iteration: 0,
            not_for_root: None,
            not_for_connecting_root: None,
        });
    }

    pub fn add_root_node(&mut self, position: P) -> NodeIdx {
        self.add_root_node_with_information(position, None)
    }

    /// Returns the root node's index.
    pub fn add_root_node_with_information(&mut self,
                                          position: P,
                                          information: Option<I>)
                                          -> NodeIdx {
        // A root node has it's own index as parent and root.
        let len = self.nodes.len();
        let root_idx = NodeIdx(len as u32);
        self.nodes.push(Node {
            parent: root_idx,
            root: root_idx,
            length: 0,
            branches: 0,
            position: position,
            growth: Zero::zero(),
            growth_count: 0,
            assigned_information: information,
        });
        root_idx
    }

    fn get_node(&self, node_idx: NodeIdx) -> Option<&Node<P, F, I>> {
        self.nodes.get(node_idx.0 as usize)
    }

    fn get_node_mut(&mut self, node_idx: NodeIdx) -> Option<&mut Node<P, F, I>> {
        self.nodes.get_mut(node_idx.0 as usize)
    }

    fn add_leaf_node(&mut self, position: P, parent: NodeIdx) {
        let (root, length) = {
            let parent_node = self.get_node_mut(parent).unwrap();
            parent_node.branches += 1;
            (parent_node.root, parent_node.length + 1)
        };

        self.nodes.push(Node {
            parent: parent,
            root: root,
            length: length,
            branches: 0,
            position: position,
            growth: Zero::zero(),
            growth_count: 0,
            assigned_information: None,
        });
    }

    pub fn visit_attractor_points<V>(&self, visitor: &mut V)
        where V: FnMut(&P)
    {
        for attractor in self.attractors.iter() {
            visitor(&attractor.position)
        }
    }

    pub fn visit_attractors<V>(&self, visitor: &mut V)
        where V: FnMut(&Attractor<P, I>)
    {
        for attractor in self.attractors.iter() {
            visitor(attractor)
        }
    }


    pub fn visit_node_segments<V>(&self, visitor: &mut V)
        where V: FnMut(&P, &P)
    {
        for node in self.nodes.iter() {
            if !node.is_root() {
                visitor(&node.position,
                        &self.get_node(node.parent).unwrap().position);
            }
        }
    }

    /// Calls the visitor for every node that has information associated.
    /// The visitor is called with the node and it's associated root node.
    /// The visitor is not called for root nodes itself!
    pub fn visit_nodes_with_info_and_root<V>(&self, visitor: &mut V)
        where V: FnMut(&Node<P, F, I>, &Node<P, F, I>)
    {
        for node in self.nodes.iter() {
            if node.assigned_information.is_some() && !node.is_root() {
                visitor(node, &self.get_node(node.root).unwrap());
            }
        }
    }

    pub fn visit_root_nodes<V>(&self, visitor: &mut V)
        where V: FnMut(&Node<P, F, I>)
    {
        for node in self.nodes.iter() {
            if node.is_root() {
                visitor(node);
            }
        }
    }
}

impl<P, F, I> Iterator for SpaceColonization<P, F, I>
    where P: FloatPnt<f32, F> + Debug,
          F: FloatVec<f32> + Zero + Copy + Debug,
          I: Copy + Default + Debug
{
    type Item = usize;

    fn next(&mut self) -> Option<Self::Item> {
        let max_length = self.max_length;
        let max_branches = self.max_branches;

        let current_iteration = self.next_iteration;
        self.next_iteration += 1;
        let num_nodes = self.nodes.len();
        let use_last_nodes: usize = cmp::min(num_nodes, self.use_last_n_nodes.unwrap_or(num_nodes));
        let start_index = num_nodes - use_last_nodes;

        // for each attraction_point, find the nearest node that it influences
        let mut ap_idx = 0;
        'outer: while ap_idx < self.attractors.len() {
            let ap = {
                let ap_ref = &self.attractors[ap_idx];

                if !ap_ref.is_active_in(current_iteration) {
                    // is attractor is not active in the current iteration goto next.
                    ap_idx += 1;
                    continue;
                }

                *ap_ref
            };

            let nodes = &mut self.nodes[start_index..];

            // find the node nearest to the `ap` attraction point
            let mut nearest_node: Option<&mut Node<_, _, _>> = None;
            let mut nearest_distance = ap.attract_dist;
            let mut connect_node: Option<&mut Node<_, _, _>> = None;
            for node in nodes.iter_mut() {
                if !node.is_active(max_length, max_branches) {
                    // The node has become inactive
                    continue;
                }

                match ap.not_for_root {
                    Some(deny_root) if deny_root == node.root => {
                        // The attractor is not for this tree node.
                        continue;
                    }
                    _ => {}
                }

                match ap.not_for_connecting_root {
                    Some(deny_root) if deny_root == node.root => {
                        // The attractor is not for this tree node.
                        continue;
                    }
                    _ => {}
                }

                let dist = SqDist(node.position.sqdist(&ap.position));

                if dist < ap.connect_dist {
                    // This node is within the connect radius of a node.
                    // XXX: There might be a closer node, but we use
                    // the first we find.
                    connect_node = Some(node);
                    // outside the node loop, we perform some action
                    break;
                } else if dist < nearest_distance {
                    // ```node``` is within the influence of the attraction point,
                    // and it's closer than the currently closest node.
                    nearest_distance = dist;
                    nearest_node = Some(node);
                }
            }

            if let Some(node) = connect_node {
                node.transmit_information(ap.information);
                match ap.connect_action {
                    ConnectAction::KillAttractor => {
                        // remove attraction point
                        self.attractors.swap_remove(ap_idx);
                        // and continue with "next" (without increasing ap_idx)
                        continue 'outer;
                    }
                    ConnectAction::DisableFor {iterations} => {
                        self.attractors[ap_idx].disable_until(current_iteration + iterations);
                    }
                    ConnectAction::DisableForConnectingRoot => {
                        self.attractors[ap_idx].not_for_connecting_root = Some(node.root)
                    }
                }
            } else if let Some(node) = nearest_node {
                // update the force with the normalized vector towards the attraction point
                let v = (ap.position - node.position).normalize() * ap.strength;
                node.growth = node.growth + v;
                node.growth_count += 1;
            }

            // go to next attractor point
            ap_idx += 1;
        }

        // now create new nodes
        for i in start_index..num_nodes {
            let growth_count = self.nodes[i].growth_count;
            if growth_count > 0 {
                let growth_factor = 1.0; //((growth_count + 1) as f32).ln();
                let d = self.nodes[i].growth.normalize() * self.move_dist * growth_factor;
                let new_position = self.nodes[i].position + d;
                self.add_leaf_node(new_position, NodeIdx(i as u32));

                // and reset growth attraction forces
                self.nodes[i].growth = Zero::zero();
                self.nodes[i].growth_count = 0;
            }
        }

        // Note that nodes can oscillate, between two attraction points, so
        // it's better to stop after a certain number of iterations
        return Some(self.nodes.len() - num_nodes);
    }
}
