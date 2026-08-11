//! On-screen warning popup (e.g. an element cannot be loaded for the
//! current screen size). Auto-hides after a few seconds; any click on it
//! dismisses it immediately.

use super::{Ctx, Rect};
use crate::font::FONT_UI;
use nacelle::theme::{self, bake::StateStyle, parse::State, Color, TokenId};
use std::sync::OnceLock;
use std::time::Instant;

fn tok(cell: &'static OnceLock<TokenId>, name: &'static str) -> TokenId {
    *cell.get_or_init(|| theme::id(name).unwrap_or(TokenId::MISSING))
}

/// The engine's colour in the draw list's clothes.
fn col(c: theme::ThemeColor) -> Color {
    Color { r: c.r, g: c.g, b: c.b, a: c.a }
}

/// One type role's hot ids, resolved once. Size is the theme's px times
/// the user's font preference, floored by the role's `min_px`; tracking
/// is the role's em value multiplied out per run — the same arithmetic
/// the object layer uses.
struct Role {
    name: &'static str,
    size: OnceLock<TokenId>,
    min: OnceLock<TokenId>,
    track: OnceLock<TokenId>,
}

impl Role {
    const fn new(name: &'static str) -> Self {
        Role { name, size: OnceLock::new(), min: OnceLock::new(), track: OnceLock::new() }
    }
    fn px(&self, ctx: &Ctx) -> f32 {
        let t = theme::resolved();
        let s = *self.size.get_or_init(|| {
            theme::id(&format!("type.{}.size", self.name)).unwrap_or(TokenId::MISSING)
        });
        let m = *self.min.get_or_init(|| {
            theme::id(&format!("type.{}.min_px", self.name)).unwrap_or(TokenId::MISSING)
        });
        (t.px(s) * ctx.ui_font_scale * ctx.panel_scale).max(t.px(m))
    }
    fn tracking(&self, px: f32) -> f32 {
        let t = theme::resolved();
        let k = *self.track.get_or_init(|| {
            theme::id(&format!("type.{}.tracking", self.name)).unwrap_or(TokenId::MISSING)
        });
        px * t.px(k)
    }
}

// The toast speaks in the section label's voice, its message in the
// body's (`toast.title.role` / `toast.body.role` in the master).
static ROLE_TOAST_TITLE: Role = Role::new("label.section");
static ROLE_TOAST_BODY: Role = Role::new("body");
// The dialog binds its text through `dialog.title.role` / `dialog.body.role`.
static ROLE_DIALOG_TITLE: Role = Role::new("title.window");
static ROLE_DIALOG_BODY: Role = Role::new("body");
static ROLE_BUTTON: Role = Role::new("button");

pub struct Popup {
    msg: Option<(String, Instant)>,
}

impl Popup {
    pub fn new() -> Self {
        Popup { msg: None }
    }

    pub fn show(&mut self, message: String) {
        self.msg = Some((message, Instant::now()));
    }

    /// Dismisses the popup if the click landed on it; returns true then.
    pub fn click(&mut self, x: f32, y: f32, w: f32, _h: f32) -> bool {
        if self.msg.is_some() {
            // Recompute the box like draw() does (without fonts: the
            // minimum width is a generous enough hit box).
            static MIN_W: OnceLock<TokenId> = OnceLock::new();
            static TH: OnceLock<TokenId> = OnceLock::new();
            static TOP: OnceLock<TokenId> = OnceLock::new();
            let t = theme::resolved();
            let bw = w * t.px(tok(&MIN_W, "toast.min_w_frac"));
            let bh = t.px(tok(&TH, "toast.h"));
            let bx = (w - bw) / 2.0;
            let by = t.px(tok(&TOP, "toast.top"));
            if x >= bx && x <= bx + bw && y >= by && y <= by + bh {
                self.msg = None;
                return true;
            }
        }
        false
    }

    pub fn draw(&mut self, ctx: &mut Ctx) {
        static DWELL: OnceLock<TokenId> = OnceLock::new();
        static MIN_W: OnceLock<TokenId> = OnceLock::new();
        static MAX_W: OnceLock<TokenId> = OnceLock::new();
        static TH: OnceLock<TokenId> = OnceLock::new();
        static TOP: OnceLock<TokenId> = OnceLock::new();
        static PAD_X: OnceLock<TokenId> = OnceLock::new();
        static TITLE_GAP: OnceLock<TokenId> = OnceLock::new();
        static MSG_GAP: OnceLock<TokenId> = OnceLock::new();
        static TITLE_C: OnceLock<TokenId> = OnceLock::new();
        static TEXT_C: OnceLock<TokenId> = OnceLock::new();
        let t = theme::resolved();
        let Some((msg, t0)) = &self.msg else { return };
        if t0.elapsed().as_secs_f32() * 1000.0 > t.px(tok(&DWELL, "toast.dwell_ms")) {
            self.msg = None;
            return;
        }
        let msg = msg.clone();

        let px = ROLE_TOAST_BODY.px(ctx);
        let title_px = ROLE_TOAST_TITLE.px(ctx);
        let text_w = ctx.fonts.measure(FONT_UI, px, &msg, ROLE_TOAST_BODY.tracking(px));
        let bw = (text_w + 2.0 * t.px(tok(&PAD_X, "toast.pad_x")))
            .max(ctx.w * t.px(tok(&MIN_W, "toast.min_w_frac")))
            .min(ctx.w * t.px(tok(&MAX_W, "toast.max_w_frac")));
        let bh = t.px(tok(&TH, "toast.h"));
        let bx = (ctx.w - bw) / 2.0;
        let by = t.px(tok(&TOP, "toast.top"));

        nacelle::object::window::frame(ctx, super::Rect::new(bx, by, bw, bh));
        ctx.dl.text_center(
            ctx.fonts,
            FONT_UI,
            title_px,
            bx + bw / 2.0,
            by + t.px(tok(&TITLE_GAP, "toast.title_gap")),
            "WARNING",
            col(t.color(tok(&TITLE_C, "component.toast.title"))),
            ROLE_TOAST_TITLE.tracking(title_px),
        );
        ctx.dl.text_center(
            ctx.fonts,
            FONT_UI,
            px,
            bx + bw / 2.0,
            by + t.px(tok(&MSG_GAP, "toast.msg_gap")),
            &msg,
            col(t.color(tok(&TEXT_C, "component.toast.text"))),
            ROLE_TOAST_BODY.tracking(px),
        );
    }
}

/// The dialog's frame rectangle: the screen minus the themed insets.
fn dialog_rect(w: f32, h: f32) -> Rect {
    static IX: OnceLock<TokenId> = OnceLock::new();
    static IY: OnceLock<TokenId> = OnceLock::new();
    let t = theme::resolved();
    let ix = t.px(tok(&IX, "dialog.inset_x"));
    let iy = t.px(tok(&IY, "dialog.inset_y"));
    Rect::new(ix, iy, w - 2.0 * ix, h - 2.0 * iy)
}

/// OK button rectangle of the resolution dialog — geometry shared by
/// drawing and hit-testing in main. Fractions of the dialog's own box.
pub fn resolution_dialog_ok_rect(w: f32, h: f32) -> Rect {
    static BW: OnceLock<TokenId> = OnceLock::new();
    static BH: OnceLock<TokenId> = OnceLock::new();
    static BY: OnceLock<TokenId> = OnceLock::new();
    let t = theme::resolved();
    let d = dialog_rect(w, h);
    let bw = d.w * t.px(tok(&BW, "dialog.button.w_frac"));
    let bh = d.h * t.px(tok(&BH, "dialog.button.h_frac"));
    Rect::new(
        d.x + (d.w - bw) / 2.0,
        d.y + d.h * t.px(tok(&BY, "dialog.button.y_frac")),
        bw,
        bh,
    )
}

/// Content of the standalone resolution dialog window, shown INSTEAD of
/// the program when the monitor resolution is below the minimum.
pub fn draw_resolution_dialog(ctx: &mut Ctx, mw: u32, mh: u32) {
    static BED: OnceLock<TokenId> = OnceLock::new();
    static CORNER: OnceLock<TokenId> = OnceLock::new();
    static RING_W: OnceLock<TokenId> = OnceLock::new();
    static RING_C: OnceLock<TokenId> = OnceLock::new();
    static TITLE_Y: OnceLock<TokenId> = OnceLock::new();
    static TITLE_C: OnceLock<TokenId> = OnceLock::new();
    static BODY_Y: OnceLock<TokenId> = OnceLock::new();
    static BODY_GAP: OnceLock<TokenId> = OnceLock::new();
    static BODY_C: OnceLock<TokenId> = OnceLock::new();
    let t = theme::resolved();
    let (w, h) = (ctx.w, ctx.h);
    let d = dialog_rect(w, h);
    ctx.dl.rect(0.0, 0.0, w, h, col(t.color(tok(&BED, "backdrop.solid"))));
    ctx.dl.chamfer_frame(
        d.x,
        d.y,
        d.w,
        d.h,
        t.px(tok(&CORNER, "dialog.corner")),
        t.px(tok(&RING_W, "dialog.border")),
        col(t.color(tok(&RING_C, "border.default"))),
    );

    let title_px = ROLE_DIALOG_TITLE.px(ctx);
    ctx.dl.text_center(
        ctx.fonts,
        FONT_UI,
        title_px,
        w / 2.0,
        d.y + d.h * t.px(tok(&TITLE_Y, "dialog.title_y_frac")),
        "WARNING",
        col(t.color(tok(&TITLE_C, "severity.warning.text"))),
        ROLE_DIALOG_TITLE.tracking(title_px),
    );
    let px = ROLE_DIALOG_BODY.px(ctx);
    let body_c = col(t.color(tok(&BODY_C, "type.body.fg")));
    let body_y = d.y + d.h * t.px(tok(&BODY_Y, "dialog.body_y_frac"));
    ctx.dl.text_center(
        ctx.fonts,
        FONT_UI,
        px,
        w / 2.0,
        body_y,
        &format!("Monitor resolution {mw}x{mh} is too small"),
        body_c,
        ROLE_DIALOG_BODY.tracking(px),
    );
    ctx.dl.text_center(
        ctx.fonts,
        FONT_UI,
        px,
        w / 2.0,
        body_y + d.h * t.px(tok(&BODY_GAP, "dialog.body_line_gap")),
        "nacelle-desktop requires a resolution of at least 1280x720",
        body_c,
        ROLE_DIALOG_BODY.tracking(px),
    );

    // OK button — a parallelogram like the control panel buttons; its
    // colours come from the button class's baked state ladder.
    static SKEW: OnceLock<TokenId> = OnceLock::new();
    static BORDER: OnceLock<TokenId> = OnceLock::new();
    static LEAD: OnceLock<TokenId> = OnceLock::new();
    static MODE: OnceLock<TokenId> = OnceLock::new();
    static OPTICAL: OnceLock<Option<u16>> = OnceLock::new();
    static BIAS: OnceLock<TokenId> = OnceLock::new();
    static CLASS: OnceLock<Option<u16>> = OnceLock::new();
    let br = resolution_dialog_ok_rect(w, h);
    let hover = br.contains(ctx.mouse.0, ctx.mouse.1);
    let class = *CLASS.get_or_init(|| theme::class_id("button"));
    let st = match class {
        Some(c) => t.class_state(c, if hover { State::Hover } else { State::Idle }),
        None => StateStyle::RAW,
    };
    let skew = t.px(tok(&SKEW, "button.skew"));
    ctx.dl.quad(
        [
            [br.x + skew, br.y],
            [br.right(), br.y],
            [br.right() - skew, br.bottom()],
            [br.x, br.bottom()],
        ],
        col(st.fill),
    );
    ctx.dl.polyline(
        &[
            [br.x + skew, br.y],
            [br.right(), br.y],
            [br.right() - skew, br.bottom()],
            [br.x, br.bottom()],
        ],
        t.px(tok(&BORDER, "button.border")),
        col(st.edge),
        true,
    );
    let bpx = ROLE_BUTTON.px(ctx);
    // Centre one leading-height line in the box, nudged by the theme's
    // cap-height bias when the rhythm block centres optically.
    let mode = tok(&MODE, "rhythm.center_mode");
    let optical =
        *OPTICAL.get_or_init(|| theme::enum_index(mode, "optical")) == Some(t.enum_of(mode));
    let mut ty = br.y + (br.h - bpx * t.px(tok(&LEAD, "type.button.leading"))) / 2.0;
    if optical {
        ty += bpx * t.px(tok(&BIAS, "rhythm.cap_center_bias"));
    }
    ctx.dl.text_center(
        ctx.fonts,
        FONT_UI,
        bpx,
        br.cx(),
        ty,
        "OK",
        col(st.text),
        ROLE_BUTTON.tracking(bpx),
    );
}
