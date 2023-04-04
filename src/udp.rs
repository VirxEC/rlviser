use std::net::UdpSocket;

use bevy::prelude::*;

use crate::ServerPort;

#[derive(Component)]
struct Ball;

#[derive(Default)]
struct BallState {
    pos: Vec3,
}

#[derive(Resource, Default)]
struct State {
    ball: BallState,
}

#[derive(Resource)]
struct UdpConnection(UdpSocket);

fn establish_connection(port: Res<ServerPort>, mut commands: Commands) {
    let socket = UdpSocket::bind(("127.0.0.1", port.secondary_port)).unwrap();
    socket.connect(("127.0.0.1", port.primary_port)).unwrap();
    socket.set_nonblocking(true).unwrap();
    socket.send(&[1]).unwrap();
    commands.insert_resource(UdpConnection(socket));
}

fn setup_arena(mut commands: Commands, mut meshes: ResMut<Assets<Mesh>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let mut ball_material = StandardMaterial::from(Color::rgb(0.3, 0.3, 0.3));
    ball_material.perceptual_roughness = 0.8;

    commands.spawn((
        Ball,
        PbrBundle {
            mesh: meshes.add(Mesh::from(shape::UVSphere { radius: 91.25, ..default() })),
            material: materials.add(ball_material),
            ..default()
        },
    ));
}

trait ToBevyVec {
    fn to_bevy(self) -> Vec3;
}

impl ToBevyVec for Vec3 {
    fn to_bevy(self) -> Self {
        Self::new(self.x, self.z, self.y)
    }
}

trait FromBytes {
    fn from_bytes(bytes: &[u8]) -> Self;
}

impl FromBytes for Vec3 {
    fn from_bytes(bytes: &[u8]) -> Self {
        Vec3::new(
            f32::from_ne_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]),
            f32::from_ne_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]),
            f32::from_ne_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]),
        )
    }
}

fn step_arena(socket: Res<UdpConnection>, mut state: ResMut<State>) {
    let mut buf = [0; 1024];
    if socket.0.recv_from(&mut buf).is_err() {
        return;
    }

    let ball_location = Vec3::from_bytes(&buf[0..12]);

    state.ball.pos = ball_location;
}

fn update_ball(state: Res<State>, mut ball: Query<(&mut Transform, &Handle<StandardMaterial>), With<Ball>>, mut materials: ResMut<Assets<StandardMaterial>>) {
    let (mut transform, standard_material) = ball.single_mut();
    let new_pos = state.ball.pos.to_bevy();
    transform.translation = new_pos;

    let material = materials.get_mut(standard_material).unwrap();

    let amount = (transform.translation.z.abs() / 3500.).min(0.55);
    material.base_color = if new_pos.z > 0. {
        Color::rgb(amount.max(0.3), (amount * (2. / 3.)).max(0.3), 0.3)
    } else {
        Color::rgb(0.3, 0.3, amount.max(0.3))
    };
}

pub struct RocketSimPlugin;

impl Plugin for RocketSimPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(State::default())
            .add_startup_system(establish_connection)
            .add_startup_system(setup_arena)
            .add_system(step_arena)
            .add_systems((update_ball,).after(step_arena))
            .add_system(update_ball.run_if(|state: Res<State>| state.is_changed()));
    }
}
