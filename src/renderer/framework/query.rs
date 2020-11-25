use crate::renderer::framework::gl;
use crate::renderer::surface::SurfaceSharedData;
use crate::scene::node::Node;
use rg3d_core::pool::Handle;

/// This struct represents an occlusion query
/// useful to determine if a node is visible or
/// not. The basic structure is:
/// ```
/// let query = Query::new(node_handle);
/// query.begin();
/// // draw node without color and depth buffer
/// query.end();
///
/// if !query.is_occluded() {
///     // draw node
/// }
/// ```
#[derive(Debug, Copy, Clone)]
pub struct Query {
    pub node: Handle<Node>,
    id: u32,
    was_occluded: bool,
}

impl Query {
    pub fn new(node: Handle<Node>) -> Self {
        let mut id = 0;
        unsafe {
            gl::GenQueries(1, &mut id);
        }

        Query {
            node,
            id,
            was_occluded: false,
        }
    }

    /// Starts an occlusion query. Any draw calls after
    /// this method call manipulate the result of the
    /// occlusion query.
    #[inline]
    pub fn begin(&self) {
        unsafe {
            gl::BeginQuery(gl::ANY_SAMPLES_PASSED_CONSERVATIVE, self.id);
        }
    }

    /// Ends the current occlusion query.
    #[inline]
    pub fn end(&self) {
        unsafe {
            gl::EndQuery(gl::ANY_SAMPLES_PASSED_CONSERVATIVE);
        }
    }

    /// If the node that has been drawn using this Query is
    /// hidden by another object this method returns `true`.
    /// Calling this method is only allowed after this query
    /// has been finished by `self.end()`.
    #[inline]
    pub fn update_occlusion_result(&mut self) -> bool {
        let mut any_samples_passed = 1;
        unsafe { gl::GetQueryObjectuiv(self.id, gl::QUERY_RESULT, &mut any_samples_passed) }

        let is_occluded = match any_samples_passed {
            0 => true,
            _ => false,
        };

        self.was_occluded = is_occluded;

        is_occluded
    }

    /// Caches last result of a query.
    pub fn was_occluded(&self) -> bool {
        self.was_occluded
    }
}
