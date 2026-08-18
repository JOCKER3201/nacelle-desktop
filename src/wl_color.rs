//! The Wayland colour pipeline: telling the compositor what colour
//! space this program's surface is in, via the Color Management
//! protocol.
//!
//! Exists only in a native Wayland session — the protocol IS the
//! session's compositor speaking. Under gamescope or X11 there is
//! nobody to talk to, the module never starts, and the COLOR settings
//! are shown greyed out and ignored.
//!
//! The connection here is a second one to the SAME display winit
//! holds: libwayland is built for that (one socket, many queues), and
//! the surface being described is winit's own, wrapped from the raw
//! pointer. Only requests about colour travel on this queue, so the
//! two event loops never trip over each other.

use std::collections::HashSet;
use std::os::fd::AsFd;

use wayland_client::backend::{Backend, ObjectId};
use wayland_client::protocol::wl_registry::{self, WlRegistry};
use wayland_client::protocol::wl_surface::WlSurface;
use wayland_client::{Connection, Dispatch, EventQueue, Proxy, QueueHandle};
use wayland_protocols::wp::color_management::v1::client::{
    wp_color_management_surface_v1::WpColorManagementSurfaceV1,
    wp_color_manager_v1::{self, WpColorManagerV1},
    wp_image_description_creator_icc_v1::WpImageDescriptionCreatorIccV1,
    wp_image_description_creator_params_v1::WpImageDescriptionCreatorParamsV1,
    wp_image_description_v1::{self, WpImageDescriptionV1},
};

/// What the compositor said it can do, plus the fate of the image
/// description being created right now.
#[derive(Default)]
struct State {
    manager: Option<(u32, u32)>,
    features: HashSet<u32>,
    tf_named: HashSet<u32>,
    primaries_named: HashSet<u32>,
    /// Some(true) = ready, Some(false) = failed with the message kept.
    desc_done: Option<bool>,
    desc_error: Option<String>,
}

pub struct ColorMgr {
    conn: Connection,
    queue: EventQueue<State>,
    state: State,
    manager: WpColorManagerV1,
    surface: WpColorManagementSurfaceV1,
}

/// What one space name asks the protocol for: its named primaries and a
/// LIST of transfer functions, best first.
///
/// A list and not one curve, because compositors disagree about the sRGB
/// one (KWin, for one, offers gamma 2.2 and not the piecewise sRGB), and
/// refusing over that would grey the whole feature out on the very
/// compositor it was built for.
///
/// A free function so that asking "can this be shown?"
/// ([`ColorMgr::supports`]) and asking for it ([`ColorMgr::apply_space`])
/// read ONE table. The settings window hides what the answer is no for,
/// so the two must not be able to disagree: a space missing from the
/// list would still be applicable, and one offered but unapplicable
/// would be a control that does nothing.
fn preset(
    space: &str,
) -> Option<(
    wp_color_manager_v1::Primaries,
    &'static [wp_color_manager_v1::TransferFunction],
)> {
    use wp_color_manager_v1::{Primaries, TransferFunction as Tf};
    match space {
        "srgb" => Some((Primaries::Srgb, &[Tf::Srgb, Tf::Gamma22, Tf::Bt1886])),
        "display p3" => {
            Some((Primaries::DisplayP3, &[Tf::Srgb, Tf::Gamma22, Tf::Bt1886]))
        }
        "adobe rgb" => Some((Primaries::AdobeRgb, &[Tf::Gamma22, Tf::Srgb])),
        "bt2020 pq" => Some((Primaries::Bt2020, &[Tf::St2084Pq])),
        "bt2020 hlg" => Some((Primaries::Bt2020, &[Tf::Hlg])),
        "scrgb linear" => Some((Primaries::Srgb, &[Tf::ExtLinear])),
        _ => None,
    }
}

impl ColorMgr {
    /// Attaches to winit's display and surface, both as raw pointers
    /// from the window handle. None when the compositor does not speak
    /// the protocol — which is the whole "greyed out" story.
    pub fn start(display: *mut std::ffi::c_void, surface: *mut std::ffi::c_void) -> Option<ColorMgr> {
        if display.is_null() || surface.is_null() {
            return None;
        }
        let backend = unsafe { Backend::from_foreign_display(display.cast()) };
        let conn = Connection::from_backend(backend);
        let mut queue: EventQueue<State> = conn.new_event_queue();
        let qh = queue.handle();
        let mut state = State::default();
        let _registry = conn.display().get_registry(&qh, ());
        queue.roundtrip(&mut state).ok()?;
        let (name, version) = state.manager?;
        let registry = conn.display().get_registry(&qh, ());
        let manager: WpColorManagerV1 = registry.bind(
            name,
            version.min(WpColorManagerV1::interface().version),
            &qh,
            (),
        );
        // The manager announces its abilities right after binding.
        queue.roundtrip(&mut state).ok()?;

        let sid = unsafe {
            ObjectId::from_ptr(WlSurface::interface(), surface.cast()).ok()?
        };
        let wl_surface = WlSurface::from_id(&conn, sid).ok()?;
        let surface = manager.get_surface(&wl_surface, &qh, ());
        conn.flush().ok()?;
        eprintln!("nacelle-desktop: colour management at the compositor's, v{version}");
        Some(ColorMgr { conn, queue, state, manager, surface })
    }

    /// Applies the preferences: an ICC profile when one is chosen (the
    /// more specific wish wins), a named colour space otherwise, and
    /// the compositor's own default for "auto".
    pub fn apply(&mut self, space: &str, icc: Option<&std::path::Path>) {
        if let Some(path) = icc {
            if self.apply_icc(path) {
                return;
            }
        }
        self.apply_space(space);
    }

    /// Whether this compositor can be asked for `space` at all.
    ///
    /// The SAME test [`ColorMgr::apply_space`] makes before it builds a
    /// description, asked ahead of time: the settings window leaves out
    /// what this machine cannot show rather than offering it and
    /// printing a line into the log when it is picked. "auto" is always
    /// answerable — it asks for nothing.
    pub fn supports(&self, space: &str) -> bool {
        let Some((prim, tfs)) = preset(space) else { return true };
        tfs.iter().any(|tf| self.state.tf_named.contains(&(*tf as u32)))
            && self
                .state
                .features
                .contains(&(wp_color_manager_v1::Feature::Parametric as u32))
            && self.state.primaries_named.contains(&(prim as u32))
    }

    fn apply_space(&mut self, space: &str) {
        let Some((prim, tfs)) = preset(space) else {
            // "auto": the compositor's preference stands.
            self.surface.unset_image_description();
            let _ = self.conn.flush();
            return;
        };
        let tf = tfs
            .iter()
            .copied()
            .find(|tf| self.state.tf_named.contains(&(*tf as u32)));
        let (Some(tf), true, true) = (
            tf,
            self.state
                .features
                .contains(&(wp_color_manager_v1::Feature::Parametric as u32)),
            self.state.primaries_named.contains(&(prim as u32)),
        ) else {
            eprintln!(
                "nacelle-desktop: the compositor does not offer '{space}' — leaving its default"
            );
            return;
        };
        let qh = self.queue.handle();
        let creator: WpImageDescriptionCreatorParamsV1 =
            self.manager.create_parametric_creator(&qh, ());
        creator.set_primaries_named(prim);
        creator.set_tf_named(tf);
        let desc = creator.create(&qh, ());
        self.finish(desc, space);
    }

    fn apply_icc(&mut self, path: &std::path::Path) -> bool {
        if !self
            .state
            .features
            .contains(&(wp_color_manager_v1::Feature::IccV2V4 as u32))
        {
            eprintln!("nacelle-desktop: the compositor does not take ICC profiles");
            return false;
        }
        let Ok(file) = std::fs::File::open(path) else {
            eprintln!("nacelle-desktop: cannot open ICC profile {}", path.display());
            return false;
        };
        let len = file.metadata().map(|m| m.len()).unwrap_or(0);
        if len == 0 || len > 32 * 1024 * 1024 {
            eprintln!("nacelle-desktop: ICC profile {} has a nonsense size", path.display());
            return false;
        }
        let qh = self.queue.handle();
        let creator: WpImageDescriptionCreatorIccV1 = self.manager.create_icc_creator(&qh, ());
        creator.set_icc_file(file.as_fd(), 0, len as u32);
        let desc = creator.create(&qh, ());
        self.finish(desc, &path.display().to_string())
    }

    /// Waits for the description to be ready and sets it. The ready
    /// event follows the create over the same socket, so a couple of
    /// roundtrips is all the waiting there is.
    fn finish(&mut self, desc: WpImageDescriptionV1, label: &str) -> bool {
        self.state.desc_done = None;
        self.state.desc_error = None;
        for _ in 0..10 {
            if self.queue.roundtrip(&mut self.state).is_err() {
                return false;
            }
            match self.state.desc_done {
                Some(true) => {
                    self.surface.set_image_description(
                        &desc,
                        wp_color_manager_v1::RenderIntent::Perceptual,
                    );
                    // Immutable once set; the compositor keeps what it
                    // needs and the object may go.
                    desc.destroy();
                    let _ = self.conn.flush();
                    eprintln!("nacelle-desktop: surface colour set to {label}");
                    return true;
                }
                Some(false) => {
                    eprintln!(
                        "nacelle-desktop: the compositor refused {label}: {}",
                        self.state.desc_error.as_deref().unwrap_or("no reason given")
                    );
                    desc.destroy();
                    let _ = self.conn.flush();
                    return false;
                }
                None => {}
            }
        }
        eprintln!("nacelle-desktop: no answer about {label}; leaving colours as they are");
        desc.destroy();
        let _ = self.conn.flush();
        false
    }
}

impl Dispatch<WlRegistry, ()> for State {
    fn event(
        state: &mut Self,
        _: &WlRegistry,
        event: wl_registry::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        if let wl_registry::Event::Global { name, interface, version } = event {
            if interface == WpColorManagerV1::interface().name {
                state.manager = Some((name, version));
            }
        }
    }
}

impl Dispatch<WpColorManagerV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &WpColorManagerV1,
        event: wp_color_manager_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        use wp_color_manager_v1::Event;
        match event {
            Event::SupportedFeature { feature } => {
                if let Ok(f) = feature.into_result() {
                    state.features.insert(f as u32);
                }
            }
            Event::SupportedTfNamed { tf } => {
                if let Ok(tf) = tf.into_result() {
                    state.tf_named.insert(tf as u32);
                }
            }
            Event::SupportedPrimariesNamed { primaries } => {
                if let Ok(p) = primaries.into_result() {
                    state.primaries_named.insert(p as u32);
                }
            }
            _ => {}
        }
    }
}

impl Dispatch<WpImageDescriptionV1, ()> for State {
    fn event(
        state: &mut Self,
        _: &WpImageDescriptionV1,
        event: wp_image_description_v1::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
        match event {
            wp_image_description_v1::Event::Ready { .. } => {
                state.desc_done = Some(true);
            }
            wp_image_description_v1::Event::Failed { msg, .. } => {
                state.desc_done = Some(false);
                state.desc_error = Some(msg);
            }
            _ => {}
        }
    }
}

impl Dispatch<WpColorManagementSurfaceV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &WpColorManagementSurfaceV1,
        _: <WpColorManagementSurfaceV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WpImageDescriptionCreatorParamsV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &WpImageDescriptionCreatorParamsV1,
        _: <WpImageDescriptionCreatorParamsV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}

impl Dispatch<WpImageDescriptionCreatorIccV1, ()> for State {
    fn event(
        _: &mut Self,
        _: &WpImageDescriptionCreatorIccV1,
        _: <WpImageDescriptionCreatorIccV1 as Proxy>::Event,
        _: &(),
        _: &Connection,
        _: &QueueHandle<Self>,
    ) {
    }
}
