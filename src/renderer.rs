use crate::udp::ToBevyVec;
use ahash::AHashMap;
use bevy::prelude::*;

#[derive(Clone, Copy, Debug)]
pub struct CustomColor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl From<CustomColor> for Color {
    fn from(color: CustomColor) -> Self {
        Self::srgba(color.r, color.g, color.b, color.a)
    }
}

impl CustomColor {
    #[inline]
    pub const fn rgba(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }
}

#[derive(Clone, Debug)]
pub enum Render {
    Line2D { start: Vec2, end: Vec2, color: CustomColor },
    Line { start: Vec3, end: Vec3, color: CustomColor },
    LineStrip { positions: Vec<Vec3>, color: CustomColor },
}

#[derive(Clone, Debug)]
pub enum RenderMessage {
    AddRender(i32, Vec<Render>),
    RemoveRender(i32),
}

#[derive(Resource, Default)]
pub struct RenderGroups {
    pub groups: AHashMap<i32, Vec<Render>>,
}

fn render_gizmos(renders: Res<RenderGroups>, mut gizmos: Gizmos) {
    for renders in renders.groups.values() {
        for render in renders.iter() {
            match render {
                Render::Line2D { start, end, color } => {
                    gizmos.line_2d(*start, *end, *color);
                }
                Render::Line { start, end, color } => {
                    gizmos.line(start.to_bevy(), end.to_bevy(), *color);
                }
                Render::LineStrip { positions, color } => {
                    gizmos.linestrip(positions.iter().copied().map(ToBevyVec::to_bevy), *color);
                }
            }
        }
    }
}

#[derive(Resource)]
pub struct DoRendering(pub bool);

pub struct UdpRendererPlugin;

impl Plugin for UdpRendererPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(RenderGroups::default())
            .insert_resource(DoRendering(true))
            .add_systems(Update, render_gizmos.run_if(|do_rendering: Res<DoRendering>| do_rendering.0));
    }
}
