//! Contains all structures and methods to create and manage base scene graph nodes.
//!
//! Base scene graph node is a simplest possible node, it is used to build more complex
//! ones using composition. It contains all fundamental properties for each scene graph
//! nodes, like local and global transforms, name, lifetime, etc. Base node is a building
//! block for all complex node hierarchies - it contains list of children and handle to
//! parent node.

use crate::core::algebra::{Matrix4, Vector3};
use crate::core::math::Matrix4Ext;
use crate::{
    core::{
        pool::Handle,
        visitor::{Visit, VisitResult, Visitor},
    },
    resource::model::Model,
    scene::{node::Node, transform::Transform},
};
use std::cell::Cell;

/// Level of detail is a collection of objects for given normalized distance range.
/// Objects will be rendered **only** if they're in specified range.
/// Normalized distance is a distance in (0; 1) range where 0 - closest to camera,
/// 1 - farthest. Real distance can be obtained by multiplying normalized distance
/// with z_far of current projection matrix.
#[derive(Debug, Default)]
pub struct LevelOfDetail {
    begin: f32,
    end: f32,
    /// List of objects, where each object represents level of detail of parent's
    /// LOD group.
    pub objects: Vec<Handle<Node>>,
}

impl LevelOfDetail {
    /// Creates new level of detail.    
    pub fn new(begin: f32, end: f32, objects: Vec<Handle<Node>>) -> Self {
        for object in objects.iter() {
            // Invalid handles are not allowed.
            assert!(object.is_some());
        }
        let begin = begin.min(end);
        let end = end.max(begin);
        Self {
            begin: begin.min(1.0).max(0.0),
            end: end.min(1.0).max(0.0),
            objects,
        }
    }

    /// Sets new starting point in distance range. Input value will be clamped in
    /// (0; 1) range.
    pub fn set_begin(&mut self, percent: f32) {
        self.begin = percent.min(1.0).max(0.0);
        if self.begin > self.end {
            std::mem::swap(&mut self.begin, &mut self.end);
        }
    }

    /// Returns starting point of the range.
    pub fn begin(&self) -> f32 {
        self.begin
    }

    /// Sets new end point in distance range. Input value will be clamped in
    /// (0; 1) range.
    pub fn set_end(&mut self, percent: f32) {
        self.end = percent.min(1.0).max(0.0);
        if self.end < self.begin {
            std::mem::swap(&mut self.begin, &mut self.end);
        }
    }

    /// Returns end point of the range.
    pub fn end(&self) -> f32 {
        self.end
    }
}

impl Visit for LevelOfDetail {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.begin.visit("Begin", visitor)?;
        self.end.visit("End", visitor)?;
        self.objects.visit("Objects", visitor)?;

        visitor.leave_region()
    }
}

/// LOD (Level-Of-Detail) group is a set of cascades (levels), where each cascade takes specific
/// distance range. Each cascade contains list of objects that should or shouldn't be rendered
/// if distance satisfy cascade range. LOD may significantly improve performance if your scene
/// contains lots of high poly objects and objects may be far away from camera. Distant objects
/// in this case will be rendered with lower details freeing precious GPU resources for other
/// useful tasks.   
///
/// Lod group must contain non-overlapping cascades, each cascade with its own set of objects
/// that belongs to level of detail. Engine does not care if you create overlapping cascades,
/// it is your responsibility to create non-overlapping cascades.
#[derive(Debug, Default)]
pub struct LodGroup {
    /// Set of cascades.
    pub levels: Vec<LevelOfDetail>,
}

impl Visit for LodGroup {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.levels.visit("Levels", visitor)?;

        visitor.leave_region()
    }
}

/// See module docs.
#[derive(Debug)]
pub struct Base {
    name: String,
    local_transform: Transform,
    visibility: bool,
    pub(in crate) global_visibility: Cell<bool>,
    pub(in crate) parent: Handle<Node>,
    pub(in crate) children: Vec<Handle<Node>>,
    pub(in crate) global_transform: Cell<Matrix4<f32>>,
    /// Bone-specific matrix. Non-serializable.
    pub(in crate) inv_bind_pose_transform: Matrix4<f32>,
    /// A resource from which this node was instantiated from, can work in pair
    /// with `original` handle to get corresponding node from resource.
    pub(in crate) resource: Option<Model>,
    /// Handle to node in scene of model resource from which this node
    /// was instantiated from.
    pub(in crate) original: Handle<Node>,
    /// When `true` it means that this node is instance of `resource`.
    /// More precisely - this node is root of whole descendant nodes
    /// hierarchy which was instantiated from resource.
    pub(in crate) is_resource_instance: bool,
    /// Maximum amount of Some(time) that node will "live" or None
    /// if node has undefined lifetime.
    pub(in crate) lifetime: Option<f32>,
    depth_offset: f32,
    lod_group: Option<LodGroup>,
}

impl Base {
    /// Sets name of node. Can be useful to mark a node to be able to find it later on.
    pub fn set_name<N: AsRef<str>>(&mut self, name: N) -> &mut Self {
        self.name = name.as_ref().to_owned();
        self
    }

    /// Returns name of node.
    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    /// Returns shared reference to local transform of a node, can be used to fetch
    /// some local spatial properties, such as position, rotation, scale, etc.
    pub fn local_transform(&self) -> &Transform {
        &self.local_transform
    }

    /// Returns mutable reference to local transform of a node, can be used to set
    /// some local spatial properties, such as position, rotation, scale, etc.
    pub fn local_transform_mut(&mut self) -> &mut Transform {
        &mut self.local_transform
    }

    /// Sets new local transform of a node.
    pub fn set_local_transform(&mut self, transform: Transform) -> &mut Self {
        self.local_transform = transform;
        self
    }

    /// Sets lifetime of node in seconds, lifetime is useful for temporary objects.
    /// Example - you firing a gun, it produces two particle systems for each shot:
    /// one for gunpowder fumes and one when bullet hits some surface. These particle
    /// systems won't last very long - usually they will disappear in 1-2 seconds
    /// but nodes will still be in scene consuming precious CPU clocks. This is where
    /// lifetimes become handy - you just set appropriate lifetime for a particle
    /// system node and it will be removed from scene when time will end. This is
    /// efficient algorithm because scene holds every object in pool and allocation
    /// or deallocation of node takes very little amount of time.
    pub fn set_lifetime(&mut self, time_seconds: f32) -> &mut Self {
        self.lifetime = Some(time_seconds);
        self
    }

    /// Returns current lifetime of a node. Will be None if node has undefined lifetime.
    /// For more info about lifetimes see [`set_lifetime`].
    pub fn lifetime(&self) -> Option<f32> {
        self.lifetime
    }

    /// Returns handle of parent node.
    pub fn parent(&self) -> crate::core::pool::Handle<Node> {
        self.parent
    }

    /// Returns slice of handles to children nodes. This can be used, for example, to
    /// traverse tree starting from some node.
    pub fn children(&self) -> &[Handle<Node>] {
        self.children.as_slice()
    }

    /// Returns global transform matrix, such matrix contains combined transformation
    /// of transforms of parent nodes. This is the final matrix that describes real
    /// location of object in the world.
    pub fn global_transform(&self) -> Matrix4<f32> {
        self.global_transform.get()
    }

    /// Returns inverse of bind pose matrix. Bind pose matrix - is special matrix
    /// for bone nodes, it stores initial transform of bone node at the moment
    /// of "binding" vertices to bones.
    pub fn inv_bind_pose_transform(&self) -> Matrix4<f32> {
        self.inv_bind_pose_transform
    }

    /// Returns true if this node is model resource instance root node.
    pub fn is_resource_instance(&self) -> bool {
        self.is_resource_instance
    }

    /// Returns resource from which this node was instantiated from.
    pub fn resource(&self) -> Option<Model> {
        self.resource.clone()
    }

    /// Sets local visibility of a node.
    pub fn set_visibility(&mut self, visibility: bool) -> &mut Self {
        self.visibility = visibility;
        self
    }

    /// Returns local visibility of a node.
    pub fn visibility(&self) -> bool {
        self.visibility
    }

    /// Returns combined visibility of an node. This is the final visibility of a node.
    /// Global visibility calculated using visibility of all parent nodes until root one,
    /// so if some parent node upper on tree is invisible then all its children will be
    /// invisible. It defines if object will be rendered. It is *not* the same as real
    /// visibility point of view of some camera. To check if object is visible from some
    /// camera, use frustum visibility check. However this still can't tell you if object
    /// is behind obstacle or not.
    pub fn global_visibility(&self) -> bool {
        self.global_visibility.get()
    }

    /// Handle to node in scene of model resource from which this node
    /// was instantiated from.
    pub fn original_handle(&self) -> Handle<Node> {
        self.original
    }

    /// Returns position of the node in absolute coordinates.
    pub fn global_position(&self) -> Vector3<f32> {
        self.global_transform.get().position()
    }

    /// Returns "look" vector of global transform basis, in most cases return vector
    /// will be non-normalized.
    pub fn look_vector(&self) -> Vector3<f32> {
        self.global_transform.get().look()
    }

    /// Returns "side" vector of global transform basis, in most cases return vector
    /// will be non-normalized.
    pub fn side_vector(&self) -> Vector3<f32> {
        self.global_transform.get().side()
    }

    /// Returns "up" vector of global transform basis, in most cases return vector
    /// will be non-normalized.
    pub fn up_vector(&self) -> Vector3<f32> {
        self.global_transform.get().up()
    }

    /// Sets depth range offset factor. It allows you to move depth range by given
    /// value. This can be used to draw weapons on top of other stuff in scene.
    ///
    /// # Details
    ///
    /// This value is used to modify projection matrix before render node.
    /// Element m[4][3] of projection matrix usually set to -1 to which makes w coordinate
    /// of in homogeneous space to be -z_fragment for further perspective divide. We can
    /// abuse this to shift z of fragment by some value.
    pub fn set_depth_offset_factor(&mut self, factor: f32) {
        self.depth_offset = factor.abs().min(1.0).max(0.0);
    }

    /// Returns depth offset factor.
    pub fn depth_offset_factor(&self) -> f32 {
        self.depth_offset
    }

    /// Sets new lod group.
    pub fn set_lod_group(&mut self, lod_group: LodGroup) -> Option<LodGroup> {
        self.lod_group.replace(lod_group)
    }

    /// Returns shared reference to current lod group.
    pub fn lod_group(&self) -> Option<&LodGroup> {
        self.lod_group.as_ref()
    }

    /// Returns mutable reference to current lod group.
    pub fn lod_group_mut(&mut self) -> Option<&mut LodGroup> {
        self.lod_group.as_mut()
    }

    /// Shallow copy of node data. You should never use this directly, shallow copy
    /// will produce invalid node in most cases!
    pub fn raw_copy(&self) -> Self {
        Self {
            name: self.name.clone(),
            local_transform: self.local_transform.clone(),
            global_transform: self.global_transform.clone(),
            visibility: self.visibility,
            global_visibility: self.global_visibility.clone(),
            inv_bind_pose_transform: self.inv_bind_pose_transform,
            resource: self.resource.clone(),
            is_resource_instance: self.is_resource_instance,
            lifetime: self.lifetime,
            // Rest of data is *not* copied!
            ..Default::default()
        }
    }
}

impl Default for Base {
    fn default() -> Self {
        BaseBuilder::new().build()
    }
}

impl Visit for Base {
    fn visit(&mut self, name: &str, visitor: &mut Visitor) -> VisitResult {
        visitor.enter_region(name)?;

        self.name.visit("Name", visitor)?;
        self.local_transform.visit("Transform", visitor)?;
        self.visibility.visit("Visibility", visitor)?;
        self.parent.visit("Parent", visitor)?;
        self.children.visit("Children", visitor)?;
        self.resource.visit("Resource", visitor)?;
        self.is_resource_instance
            .visit("IsResourceInstance", visitor)?;
        self.lifetime.visit("Lifetime", visitor)?;
        self.depth_offset.visit("DepthOffset", visitor)?;
        let _ = self.lod_group.visit("LodGroup", visitor);

        visitor.leave_region()
    }
}

/// Base node builder allows you to create nodes in declarative manner.
pub struct BaseBuilder {
    name: Option<String>,
    visibility: Option<bool>,
    local_transform: Option<Transform>,
    children: Option<Vec<Handle<Node>>>,
    lifetime: Option<f32>,
    depth_offset: f32,
    lod_group: Option<LodGroup>,
}

impl Default for BaseBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl BaseBuilder {
    /// Creates new builder instance.
    pub fn new() -> Self {
        Self {
            name: None,
            visibility: None,
            local_transform: None,
            children: None,
            lifetime: None,
            depth_offset: 0.0,
            lod_group: None,
        }
    }

    /// Sets desired name.
    pub fn with_name<P: AsRef<str>>(mut self, name: P) -> Self {
        self.name = Some(name.as_ref().to_owned());
        self
    }

    /// Sets desired visibility.
    pub fn with_visibility(mut self, visibility: bool) -> Self {
        self.visibility = Some(visibility);
        self
    }

    /// Sets desired local transform.
    pub fn with_local_transform(mut self, transform: Transform) -> Self {
        self.local_transform = Some(transform);
        self
    }

    /// Sets desired list of children nodes.
    pub fn with_children(mut self, children: Vec<Handle<Node>>) -> Self {
        self.children = Some(children);
        self
    }

    /// Sets desired lifetime.
    pub fn with_lifetime(mut self, time_seconds: f32) -> Self {
        self.lifetime = Some(time_seconds);
        self
    }

    /// Sets desired depth offset.
    pub fn with_depth_offset(mut self, offset: f32) -> Self {
        self.depth_offset = offset;
        self
    }

    /// Sets desired lod group.
    pub fn with_lod_group(mut self, lod_group: LodGroup) -> Self {
        self.lod_group = Some(lod_group);
        self
    }

    /// Creates new instance of base scene node. Do not forget to add
    /// node to scene or pass to other nodes as base.
    pub fn build(self) -> Base {
        Base {
            name: self.name.unwrap_or_default(),
            children: self.children.unwrap_or_default(),
            local_transform: self.local_transform.unwrap_or_else(Transform::identity),
            lifetime: self.lifetime,
            visibility: self.visibility.unwrap_or(true),
            global_visibility: Cell::new(true),
            parent: Handle::NONE,
            global_transform: Cell::new(Matrix4::identity()),
            inv_bind_pose_transform: Matrix4::identity(),
            resource: None,
            original: Handle::NONE,
            is_resource_instance: false,
            depth_offset: self.depth_offset,
            lod_group: self.lod_group,
        }
    }

    /// Creates new node instance.
    pub fn build_node(self) -> Node {
        Node::Base(self.build())
    }
}
