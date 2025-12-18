use crate::{
    flat::rocketsim,
    udp::{ToBevyVec, ToBevyVecFlat},
};
use ahash::AHashMap;
use bevy::prelude::*;

#[derive(Resource, Default)]
pub struct RenderGroups {
    pub groups: AHashMap<i32, Vec<rocketsim::Render>>,
}

impl From<rocketsim::Color> for Color {
    fn from(value: rocketsim::Color) -> Self {
        Self::srgba(value.r, value.g, value.b, value.a)
    }
}

fn render_gizmos(renders: Res<RenderGroups>, mut gizmos: Gizmos) {
    for renders in renders.groups.values() {
        for render in renders.iter() {
            match render {
                rocketsim::Render::Line2D(r) => {
                    gizmos.line_2d(r.start.to_bevy_flat(), r.end.to_bevy_flat(), r.color);
                }
                rocketsim::Render::Line3D(r) => {
                    gizmos.line(r.start.to_bevy(), r.end.to_bevy(), r.color);
                }
                rocketsim::Render::LineStrip(r) => {
                    gizmos.linestrip(r.positions.iter().copied().map(ToBevyVec::to_bevy), r.color);
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
