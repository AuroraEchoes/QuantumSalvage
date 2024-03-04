use std::time::Instant;
use std::{f32::consts::PI, time::Duration};

use crate::dialogue::{update_dialogue, Dialogue, DialoguePlugin};
use crate::ui::{
    spawn_ui, update_score_text, update_shield_ui, update_throttle_ui, update_weapon_ui,
};
use crate::{BackgroundPNG, GameLifecycleState};
use bevy::ecs::schedule::common_conditions::in_state;
use bevy::ecs::schedule::{IntoSystemConfigs, NextState, OnEnter, OnExit, State, States};
use bevy::hierarchy::{BuildChildren, Children, DespawnRecursiveExt};
use bevy::time::TimerMode;
use bevy::ui::node_bundles::ImageBundle;
use bevy::ui::{Style, UiImage, Val};
use bevy::{
    app::{Plugin, PluginGroup, PreUpdate, Update},
    asset::{Assets, Handle},
    core::Name,
    core_pipeline::core_2d::Camera2d,
    ecs::{
        bundle::Bundle,
        component::Component,
        entity::Entity,
        event::EventReader,
        query::{With, Without},
        system::{Query, Res, ResMut, Resource},
    },
    input::{keyboard::KeyCode, ButtonInput},
    math::{Quat, Vec2, Vec3},
    prelude::{default, App, AssetServer, Camera2dBundle, Commands, Startup},
    reflect::Reflect,
    render::texture::{Image, ImagePlugin},
    sprite::{SpriteBundle, SpriteSheetBundle, TextureAtlas, TextureAtlasLayout},
    time::{Time, Timer},
    transform::components::Transform,
    window::Window,
    DefaultPlugins,
};
use bevy_inspector_egui::quick::WorldInspectorPlugin;
use bevy_rapier2d::{
    geometry::{ActiveCollisionTypes, ActiveEvents, ActiveHooks, Collider},
    pipeline::CollisionEvent,
    plugin::{NoUserData, RapierPhysicsPlugin},
    render::RapierDebugRenderPlugin,
};
use rand::Rng;

pub const TURN_SPEED: f32 = 0.025;
pub const ACCELERATION_SPEED: f32 = 0.0005;
pub const BULLET_SPEED: f32 = 0.01;
pub const MAX_VELOCITY: f32 = 0.05;

pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.register_type::<Bullet>()
            .register_type::<NPCLogic>()
            .register_type::<Spacecraft>()
            .insert_state(GameState::Regular)
            .add_plugins((
                // WorldInspectorPlugin::default(),
                // RapierDebugRenderPlugin::default(),
                RapierPhysicsPlugin::<NoUserData>::pixels_per_meter(100.0),
                DialoguePlugin,
            ))
            .add_systems(
                OnEnter(GameLifecycleState::Game),
                (setup, spawn_ui, init_nonfatal_explosion_images_res),
            )
            .add_systems(OnExit(GameLifecycleState::Game), zoom_back_in)
            .add_systems(
                Update,
                (handle_inputs, check_for_usage_decision)
                    .run_if(in_state(GameState::Paused))
                    .run_if(in_state(GameLifecycleState::Game)),
            )
            .add_systems(
                Update,
                (
                    handle_inputs,
                    pause_for_captured_ship,
                    move_spaceships,
                    handle_npc_logic,
                    move_bullets,
                    tick_timer,
                    collide_bullets.after(handle_explosions),
                    kill_far_bullets.after(handle_explosions),
                    swap_ships.after(handle_explosions),
                    update_score,
                    spawn_ships,
                )
                    .run_if(in_state(GameLifecycleState::Game))
                    .run_if(in_state(GameState::Regular)),
            )
            .add_systems(
                Update,
                (
                    camera_follow,
                    update_weapon_ui,
                    update_throttle_ui,
                    update_shield_ui,
                    update_score_text,
                    update_dialogue,
                    handle_explosions,
                    recharge_shield,
                    kill_dead_ships.after(handle_explosions),
                    enforce_border,
                )
                    .run_if(in_state(GameLifecycleState::Game)),
            );
    }
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, States)]
pub enum GameState {
    Regular,
    Paused,
}

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    background: Res<BackgroundPNG>,
    mut texture_atlases: ResMut<Assets<TextureAtlasLayout>>,
    mut camera: Query<&mut Transform, With<Camera2d>>,
) {
    if let Ok(mut camera) = camera.get_single_mut() {
        camera.scale.x = 2.;
        camera.scale.y = 2.
    }
    commands.spawn(SpriteBundle {
        texture: background.0.clone(),
        transform: Transform::default().with_scale(Vec3::new(15., 15., 0.)),
        ..default()
    });
    let textures = ShipTextures {
        ship_one: asset_server.load("ships/Ship1/Ship1.png"),
        ship_two: asset_server.load("ships/Ship2/Ship2.png"),
        ship_three: asset_server.load("ships/Ship3/Ship3.png"),
        ship_four: asset_server.load("ships/Ship4/Ship4.png"),
        ship_five: asset_server.load("ships/Ship5/Ship5.png"),
        ship_six: asset_server.load("ships/Ship6/Ship6.png"),
    };
    let recharge_atlas = TextureAtlasLayout::from_grid(Vec2::new(32., 32.), 5, 1, None, None);
    commands.insert_resource(ShieldRechargeTextures {
        image: asset_server.load("shield_recharge.png"),
        atlas: texture_atlases.add(recharge_atlas),
    });
    commands.insert_resource(BulletTexture(
        asset_server.load("ships/Shots/Shot1/shot1_asset.png"),
    ));
    commands.insert_resource(CarryoverEnemyPoints(10));
    commands.insert_resource(PausedWhatToDoImage(
        asset_server.load("captured_ship_options.png"),
    ));
    commands.insert_resource(AllyTexture(asset_server.load("ally_flag.png")));

    commands
        .spawn(PlayerBundle::create_ship(
            ShipType::Ship4,
            Vec2::new(0., 0.),
            &textures,
        ))
        .insert(Name::new("Player"));
    commands.insert_resource(PlayerScore {
        score: 0,
        add_score_timer: Timer::new(Duration::from_secs(10), TimerMode::Repeating),
        survived_time: Instant::now(),
    });

    commands.insert_resource(textures)
}

fn remove_overly_long_tag(
    mut commands: Commands,
    query: Query<
        Entity,
        With<MyFateLiesInTheBalanceAndIWouldReallyAppreciateIfIfYouDidntKillMeMarker>,
    >,
) {
    if let Ok(entity) = query.get_single() {
        commands
            .entity(entity)
            .remove::<MyFateLiesInTheBalanceAndIWouldReallyAppreciateIfIfYouDidntKillMeMarker>();
    }
}

fn zoom_back_in(mut camera: Query<&mut Transform, With<Camera2d>>) {
    if let Ok(mut camera) = camera.get_single_mut() {
        camera.scale.x = 2.;
        camera.scale.y = 2.
    }
}

fn spawn_enemy(
    mut commands: &mut Commands,
    base_pos: Vec2,
    ship_type: ShipType,
    ship_textures: &ShipTextures,
) {
    let mut rand = rand::thread_rng();
    let mut position = Vec2::new(rand.gen_range(-2f32..2f32), rand.gen_range(-2f32..2f32));
    let poss_spawn_coords = [
        rand.gen_range(-2.5..-1.2),
        rand.gen_range(1.2..2.5),
        rand.gen_range(-2.5..-1.2),
        rand.gen_range(1.2..2.5),
    ];
    let a = rand.gen_range(0..=1);
    let b = rand.gen_range(2..=3);
    let pos = Vec2::new(poss_spawn_coords[a], poss_spawn_coords[b]) + base_pos;
    commands
        .spawn(EnemySpacecraftBundle::create_ship(
            ship_type,
            pos,
            ship_textures,
        ))
        .insert(Name::new(format!("Enemy")));
}

fn tick_timer(time: Res<Time>, mut ships: Query<&mut Spacecraft>) {
    for mut ship in ships.iter_mut() {
        ship.weapon_cooldown.tick(time.delta());
    }
}

#[derive(Bundle)]
pub struct EnemySpacecraftBundle {
    spacecraft: Spacecraft,
    sprite: SpriteBundle,
    logic: NPCLogic,
    collider: ColliderBundle,
}

impl EnemySpacecraftBundle {
    pub fn create_ship(ship_type: ShipType, pos: Vec2, ship_textures: &ShipTextures) -> Self {
        let template_ship = ShipProfile::from_type(ship_type);
        let transform = Transform {
            translation: Vec3::new(0., 0., 10.),
            rotation: Quat::from_rotation_z(3. * PI / 2.),
            scale: Vec3::new(
                template_ship.relative_scale,
                template_ship.relative_scale,
                1.,
            ),
        };
        Self {
            spacecraft: Spacecraft::from_template(ship_type, pos),
            logic: NPCLogic {},
            sprite: SpriteBundle {
                texture: ship_textures.texture(ship_type),
                transform,
                ..default()
            },
            collider: ship_type.collider(),
        }
    }
}

#[derive(Component, Reflect)]
pub struct NPCLogic;

#[derive(Component, Reflect)]
pub struct Spacecraft {
    pub position: Vec2,
    pub heading: f32,
    pub delta_rotation: f32,
    pub velocity: f32,
    pub health: i32,
    pub weapon_cooldown: Timer,
    pub shield_recharge: Timer,
    pub ship_type: ShipType,
}

#[derive(Bundle)]
pub struct PlayerBundle {
    craft: Spacecraft,
    marker: PlayerMarker,
    sprite: SpriteBundle,
    collider: ColliderBundle,
}

impl PlayerBundle {
    fn create_ship(ship_type: ShipType, pos: Vec2, ship_textures: &ShipTextures) -> Self {
        let template_ship = ShipProfile::from_type(ship_type);
        let transform = Transform {
            translation: Vec3::new(0., 0., 10.),
            rotation: Quat::from_rotation_z(3. * PI / 2.),
            scale: Vec3::new(
                template_ship.relative_scale,
                template_ship.relative_scale,
                1.,
            ),
        };

        Self {
            craft: Spacecraft::from_template(ship_type, pos),
            marker: PlayerMarker,
            sprite: SpriteBundle {
                texture: ship_textures.texture(ship_type),
                transform,
                ..default()
            },
            collider: ship_type.collider(),
        }
    }
}

#[derive(Component)]
pub struct PlayerMarker;

impl Spacecraft {
    pub fn from_template(template: ShipType, pos: Vec2) -> Self {
        let template_ship = ShipProfile::from_type(template);
        let mut shield_recharge_timer =
            Timer::new(template_ship.shield_recharge_time, TimerMode::Once);
        shield_recharge_timer.set_elapsed(template_ship.shield_recharge_time);
        Self {
            position: pos,
            heading: 0.,
            delta_rotation: 0.,
            velocity: 0.,
            health: template_ship.max_health,
            ship_type: template,
            weapon_cooldown: Timer::new(template_ship.gun_reload_time, TimerMode::Once),
            shield_recharge: shield_recharge_timer,
        }
    }

    pub fn rotate(&mut self, amount: f32) {
        self.heading += amount;
        self.delta_rotation -= amount;
    }

    pub fn end_frame(&mut self) {
        self.delta_rotation = 0.;
    }

    pub fn collide(
        &mut self,
        damage: i32,
        reduce_to_one: bool,
        mut score: &mut PlayerScore,
    ) -> bool {
        // Whether to swap
        if self.health - damage <= 0 && reduce_to_one {
            self.health = 1;
            score.score += 15;
            true
        } else {
            self.health -= damage;
            if reduce_to_one {
                score.score += 5
            }
            false
        }
    }
}

fn camera_follow(
    mut transforms: Query<&mut Transform, (With<Camera2d>, Without<PlayerMarker>)>,
    player_ship: Query<(&Spacecraft, &Transform), (With<PlayerMarker>, Without<Camera2d>)>,
) {
    if let Ok(mut cam_transform) = transforms.get_single_mut() {
        if let Ok((ship, transform)) = player_ship.get_single() {
            cam_transform.rotate_z(ship.delta_rotation);
            cam_transform.translation.x = transform.translation.x;
            cam_transform.translation.y = transform.translation.y;
        }
    }
}

pub fn move_spaceships(
    mut ships: Query<(&mut Spacecraft, &mut Transform)>,
    window: Query<&Window>,
    time: Res<Time>,
) {
    for (mut ship, mut transform) in ships.iter_mut() {
        ship.shield_recharge.tick(time.delta());
        // Transform player in clip-space coordinates
        let delta_pos = Vec2::new(ship.heading.sin(), ship.heading.cos()) * ship.velocity;
        ship.position += delta_pos;

        // Translate and apply to sprite component
        if let Ok(window) = window.get_single() {
            let window_dimensions = Vec2::new(window.width(), window.height());
            let ship_pos = ship.position * (window_dimensions / 2.);
            transform.translation = ship_pos.extend(10.);
            transform.rotate_z(ship.delta_rotation);
        }
    }
}

#[derive(Component)]
pub struct RechargingShieldMarker;

pub fn handle_inputs(
    mut commands: Commands,
    inputs: Res<ButtonInput<KeyCode>>,
    mut player_ship: Query<(Entity, &mut Spacecraft), With<PlayerMarker>>,
    mut dialogue: ResMut<Dialogue>,
    bullet_texture: Res<BulletTexture>,
    state: Res<State<GameState>>,
) {
    if let Ok((entity, mut player_ship)) = player_ship.get_single_mut() {
        player_ship.end_frame();
        if inputs.pressed(KeyCode::ArrowLeft) {
            player_ship.rotate(-TURN_SPEED);
        }
        if inputs.pressed(KeyCode::ArrowRight) {
            player_ship.rotate(TURN_SPEED);
        }
        if inputs.pressed(KeyCode::ArrowUp) {
            player_ship.velocity += ACCELERATION_SPEED;
            player_ship.velocity = player_ship.velocity.clamp(0., MAX_VELOCITY);
        }
        if inputs.pressed(KeyCode::ArrowDown) {
            player_ship.velocity -= ACCELERATION_SPEED;
            player_ship.velocity = player_ship.velocity.clamp(0., MAX_VELOCITY);
        }
        if inputs.pressed(KeyCode::Space) {
            if player_ship.weapon_cooldown.finished() {
                ship_fire(&mut commands, &mut player_ship, &*bullet_texture, true)
            }
        }
        if inputs.pressed(KeyCode::KeyS) {
            if player_ship.shield_recharge.finished() {
                commands.entity(entity).insert(RechargingShieldMarker);
                player_ship.shield_recharge.reset();
            }
        }
        if inputs.pressed(KeyCode::Enter) {
            dialogue.hide()
        }
        if inputs.just_released(KeyCode::Digit1) && state.get().eq(&GameState::Paused) {
            commands.spawn(ShipUsageDecision::Transfer);
        } else if inputs.just_released(KeyCode::Digit2) && state.get().eq(&GameState::Paused) {
            commands.spawn(ShipUsageDecision::Keep);
        } else if inputs.just_released(KeyCode::Digit3) && state.get().eq(&GameState::Paused) {
            commands.spawn(ShipUsageDecision::Destroy);
        }
    }
}

#[derive(Component)]
pub enum ShipUsageDecision {
    Transfer,
    Keep,
    Destroy,
}

#[derive(Resource)]
pub struct AllyTexture(Handle<Image>);

#[derive(Component)]
pub struct ShipUsageImageMarker;

#[derive(Component)]
pub struct MyFateLiesInTheBalanceAndIWouldReallyAppreciateIfIfYouDidntKillMeMarker;

fn check_for_usage_decision(
    mut commands: Commands,
    mut state: ResMut<NextState<GameState>>,
    usage: Query<&ShipUsageDecision>,
    image: Query<Entity, With<ShipUsageImageMarker>>,
    mut ship: Query<
        (Entity, &mut Spacecraft),
        With<MyFateLiesInTheBalanceAndIWouldReallyAppreciateIfIfYouDidntKillMeMarker>,
    >,
    mut score: ResMut<PlayerScore>,
    ally_texture: Res<AllyTexture>,
) {
    if let Ok(decision) = usage.get_single() {
        match decision {
            ShipUsageDecision::Transfer => {
                for (transfer_entity, _) in ship.iter() {
                    commands.entity(transfer_entity).insert(SwapToShipMarker).remove::<MyFateLiesInTheBalanceAndIWouldReallyAppreciateIfIfYouDidntKillMeMarker>();
                }
            }
            ShipUsageDecision::Keep => {
                for (new_ally_entity, _) in ship.iter() {
                    commands.entity(new_ally_entity).insert(Captured).remove::<MyFateLiesInTheBalanceAndIWouldReallyAppreciateIfIfYouDidntKillMeMarker>().with_children(|parent|
                    {
                        parent.spawn(SpriteBundle {
                            transform: Transform::from_xyz(0., 30., 80.).with_scale(Vec3::new(3., 3., 1.)),
                            texture: ally_texture.0.clone(),
                            ..default()
                        });
                    });
                }
            }
            ShipUsageDecision::Destroy => {
                for (future_destruction_entity, mut im_about_to_explode) in ship.iter_mut() {
                    commands.entity(future_destruction_entity).remove::<MyFateLiesInTheBalanceAndIWouldReallyAppreciateIfIfYouDidntKillMeMarker>();
                    im_about_to_explode.collide(100, false, &mut *score);
                }
            }
        }
        commands.entity(image.single()).despawn();
        state.set(GameState::Regular);
    }
}

#[derive(Component)]
pub struct TimerComp(Timer);

pub fn recharge_shield(
    mut commands: Commands,
    time: Res<Time>,
    mut unsetup_recharging: Query<
        (Entity, &Spacecraft),
        (With<RechargingShieldMarker>, Without<Children>),
    >,
    mut setup_recharging: Query<
        (Entity, &mut Spacecraft),
        (With<RechargingShieldMarker>, With<Children>),
    >,
    mut children: Query<(Entity, &mut TimerComp, &mut TextureAtlas)>,
    shield_textures: Res<ShieldRechargeTextures>,
) {
    for (entity, ship) in unsetup_recharging.iter_mut() {
        commands.entity(entity).with_children(|parent| {
            parent
                .spawn(SpriteSheetBundle {
                    texture: shield_textures.image.clone(),
                    atlas: TextureAtlas {
                        layout: shield_textures.atlas.clone(),
                        index: 0,
                    },
                    transform: Transform::from_xyz(0., 0., 50.).with_scale(Vec3::new(4., 4., 1.)),
                    ..default()
                })
                .insert(RechargingShieldMarker)
                .insert(TimerComp(ship.shield_recharge.clone()));
        });
    }
    for (entity, mut ship) in setup_recharging.iter_mut() {
        ship.velocity = 0.;
        ship.delta_rotation = 0.;
        if ship.shield_recharge.just_finished() {
            commands.entity(entity).remove::<RechargingShieldMarker>();
            ship.health += 1;
            ship.health = ship
                .health
                .min(ShipProfile::from_type(ship.ship_type).max_health);
        }
    }
    for (entity, mut timer, mut atlas) in children.iter_mut() {
        timer.0.tick(time.delta());
        let index = match timer.0.fraction() {
            x if x >= 0. && x < 0.2 => 0,
            x if x >= 0.2 && x < 0.4 => 1,
            x if x >= 0.4 && x < 0.6 => 2,
            x if x >= 0.6 && x < 0.8 => 3,
            _ => 4,
        };
        atlas.index = index;
        if timer.0.just_finished() {
            print!("Despawning {:?}", entity);
            commands.entity(entity).despawn();
        }
    }
}

#[derive(Component)]
pub struct Captured;

pub fn handle_npc_logic(
    mut commands: Commands,
    mut enemies: Query<
        (&mut NPCLogic, &mut Spacecraft),
        (Without<Captured>, Without<PlayerMarker>),
    >,
    mut captured: Query<(&mut NPCLogic, &mut Spacecraft), (With<Captured>, Without<PlayerMarker>)>,
    player: Query<&Spacecraft, With<PlayerMarker>>,
    bullet_texture: Res<BulletTexture>,
) {
    if let Ok(player) = player.get_single() {
        for (mut logic, mut craft) in enemies.iter_mut() {
            craft.end_frame();
            let ideal_direction = player.position - craft.position;
            let ideal_heading = f32::atan2(ideal_direction.x, ideal_direction.y);
            let ideal_heading_delta = ideal_heading - craft.heading;
            let delta_heading = ideal_heading_delta.clamp(-TURN_SPEED, TURN_SPEED);
            craft.rotate(delta_heading);
            let max_speed = ShipProfile::from_type(craft.ship_type).max_velocity;
            let dist = craft.position.distance(player.position);
            let ideal_speed = match dist {
                x if x > 1.2 => (1. * max_speed).min(max_speed),
                x if x <= 1.2 && x >= 0.5 => (x * (1. / 0.7) * max_speed).min(max_speed),
                x if x < 0.5 => (0. * max_speed).min(max_speed),
                _ => max_speed,
            };
            craft.velocity = ideal_speed * 0.15;
            if craft.weapon_cooldown.finished() && dist < 1.2 {
                ship_fire(&mut commands, &mut craft, &*bullet_texture, false)
            }
        }
        for (mut logic, mut craft) in captured.iter_mut() {
            let mut enemies = enemies.iter().collect::<Vec<_>>();
            enemies.sort_by(|(_, enemy_one), (_, enemy_two)| {
                craft
                    .position
                    .distance(enemy_one.position)
                    .partial_cmp(&craft.position.distance(enemy_two.position))
                    .unwrap()
            });
            if let Some((_, target)) = enemies.get(0) {
                craft.end_frame();
                let ideal_direction = target.position - craft.position;
                let ideal_heading = f32::atan2(ideal_direction.x, ideal_direction.y);
                let ideal_heading_delta = ideal_heading - craft.heading;
                let delta_heading = ideal_heading_delta.clamp(-TURN_SPEED, TURN_SPEED);
                craft.rotate(delta_heading);
                let max_speed = ShipProfile::from_type(craft.ship_type).max_velocity;
                let dist = craft.position.distance(target.position);
                let ideal_speed = match dist {
                    x if x > 1.2 => 1. * max_speed,
                    x if x <= 1.2 && x >= 0.5 => x * (1. / 0.7) * max_speed,
                    x if x < 0.5 => 0. * max_speed,
                    _ => max_speed,
                };
                craft.velocity = ideal_speed * 0.15;
                if craft.weapon_cooldown.finished() && dist < 1.2 {
                    ship_fire(&mut commands, &mut craft, &*bullet_texture, false)
                }
            }
        }
    }
}

#[derive(Resource)]
pub struct BulletTexture(Handle<Image>);

pub fn ship_fire(
    mut commands: &mut Commands,
    parent: &mut Spacecraft,
    bullet_texture: &BulletTexture,
    player_shot: bool,
) {
    match parent.ship_type {
        ShipType::Ship1 | ShipType::Ship3 | ShipType::Ship5 => {
            spawn_bullet(commands, parent, bullet_texture, 0., player_shot)
        }
        ShipType::Ship2 => {
            spawn_bullet(commands, parent, bullet_texture, -0.03, player_shot);
            spawn_bullet(commands, parent, bullet_texture, 0.03, player_shot);
        }
        ShipType::Ship4 | ShipType::Ship6 => {
            spawn_bullet(commands, parent, bullet_texture, -0.05, player_shot);
            spawn_bullet(commands, parent, bullet_texture, 0., player_shot);
            spawn_bullet(commands, parent, bullet_texture, 0.05, player_shot);
        }
    }
}

pub fn spawn_bullet(
    mut commands: &mut Commands,
    mut parent: &mut Spacecraft,
    bullet_texture: &BulletTexture,
    lateral_offset: f32,
    player_shot: bool,
) {
    let parent_template = ShipProfile::from_type(parent.ship_type);
    let lateral_heading = parent.heading - (PI / 2.);
    let lateral_offset_vec =
        Vec2::new(lateral_heading.sin(), lateral_heading.cos()) * lateral_offset;
    let bullet_offset = lateral_offset_vec
        + Vec2::new(parent.heading.sin(), parent.heading.cos()).normalize()
            * 0.12
            * parent_template.relative_scale;

    commands
        .spawn(BulletBundle {
            bullet: Bullet {
                heading: parent.heading,
                position: parent.position + bullet_offset,
                velocity: parent_template.base_bullet_velocity + parent.velocity,
                player_shot,
            },
            sprite: SpriteBundle {
                texture: bullet_texture.0.clone(),
                transform: Transform::from_xyz(0., 0., 30.)
                    .with_scale(Vec3::new(2., 2., 1.))
                    .with_rotation(Quat::from_rotation_z(3. * PI / 2. - parent.heading)),
                ..default()
            },
            collider: ColliderBundle {
                collider: Collider::cuboid(3., 3.),
                events: ActiveEvents::all(),
                hooks: ActiveHooks::all(),
                types: ActiveCollisionTypes::all(),
            },
        })
        .insert(Name::new("Bullet"));
    parent.weapon_cooldown.reset();
}

#[derive(Component, Reflect)]
pub struct Bullet {
    heading: f32,
    position: Vec2,
    velocity: f32,
    player_shot: bool,
}

#[derive(Bundle)]
pub struct BulletBundle {
    bullet: Bullet,
    sprite: SpriteBundle,
    collider: ColliderBundle,
}

#[derive(Bundle)]
pub struct ColliderBundle {
    collider: Collider,
    events: ActiveEvents,
    hooks: ActiveHooks,
    types: ActiveCollisionTypes,
}

pub fn move_bullets(mut bullets: Query<(&mut Bullet, &mut Transform)>, window: Query<&Window>) {
    for (mut bullet, mut transform) in bullets.iter_mut() {
        // Transform player in clip-space coordinates
        let delta_pos = Vec2::new(bullet.heading.sin(), bullet.heading.cos()) * bullet.velocity;
        bullet.position += delta_pos;

        // Translate and apply to sprite component
        if let Ok(window) = window.get_single() {
            let window_dimensions = Vec2::new(window.width(), window.height());
            let bullet_pos = bullet.position * (window_dimensions / 2.);
            transform.translation = bullet_pos.extend(30.);
        }
    }
}

#[derive(Resource)]
pub struct PausedWhatToDoImage(Handle<Image>);

fn pause_for_captured_ship(
    mut commands: Commands,
    mut state: ResMut<NextState<GameState>>,
    ship_killed: Query<
        Entity,
        With<MyFateLiesInTheBalanceAndIWouldReallyAppreciateIfIfYouDidntKillMeMarker>,
    >,
    image: Res<PausedWhatToDoImage>,
) {
    if let Ok(_) = ship_killed.get_single() {
        state.set(GameState::Paused);
        commands
            .spawn(ImageBundle {
                style: Style {
                    width: Val::Percent(60.),
                    height: Val::Percent(40.),
                    align_self: bevy::ui::AlignSelf::Center,
                    justify_self: bevy::ui::JustifySelf::Center,
                    ..default()
                },
                image: UiImage::new(image.0.clone()),
                ..default()
            })
            .insert(ShipUsageImageMarker);
    }
}

fn collide_bullets(
    mut commands: Commands,
    mut collision_events: EventReader<CollisionEvent>,
    mut ships: Query<(Entity, &mut Spacecraft)>,
    bullets: Query<(Entity, &Bullet)>,
    mut score: ResMut<PlayerScore>,
) {
    for event in collision_events.read() {
        match event {
            CollisionEvent::Started(a, b, _) => {
                let mut a_shotby_p = false;
                let mut b_shotby_p = false;
                for (entity, bullet) in bullets.iter() {
                    if &entity == a && bullet.player_shot {
                        a_shotby_p = true;
                    }
                    if &entity == b && bullet.player_shot {
                        b_shotby_p = true;
                    }
                }
                for (entity, mut ship) in ships.iter_mut() {
                    if &entity == a {
                        if let Some(mut entity) = commands.get_entity(*a) {
                            println!("HEREA");
                            entity.insert(ExplosionMarker);
                            if ship.collide(1, b_shotby_p, &mut *score) {
                                entity.insert(MyFateLiesInTheBalanceAndIWouldReallyAppreciateIfIfYouDidntKillMeMarker);
                            }
                        }

                        if let Some(mut entity) = commands.get_entity(*b) {
                            if let Some((mut s, _)) = ships
                                .iter_mut()
                                .map(|(e, s)| (s, &e == b))
                                .filter(|(_, x)| *x)
                                .next()
                            {
                                s.collide(1, a_shotby_p, &mut *score);
                            } else {
                                entity.despawn();
                            }
                        }
                        return;
                    } else if &entity == b {
                        if let Some(mut entity) = commands.get_entity(*b) {
                            println!("HEREB");
                            entity.insert(ExplosionMarker);
                            if ship.collide(1, a_shotby_p, &mut *score) {
                                entity.insert(MyFateLiesInTheBalanceAndIWouldReallyAppreciateIfIfYouDidntKillMeMarker);
                            }
                        }

                        if let Some(mut entity) = commands.get_entity(*a) {
                            if let Some((mut s, _)) = ships
                                .iter_mut()
                                .map(|(e, s)| (s, &e == b))
                                .filter(|(_, x)| *x)
                                .next()
                            {
                                s.collide(1, a_shotby_p, &mut *score);
                            } else {
                                entity.despawn();
                            }
                        }

                        return;
                    }
                }
            }
            CollisionEvent::Stopped(_, _, _) => {}
        }
    }
}

#[derive(Resource)]
pub struct NonfatalExplosionImages {
    image: Handle<Image>,
    atlas: Handle<TextureAtlasLayout>,
}

fn init_nonfatal_explosion_images_res(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlasLayout>>,
) {
    commands.insert_resource(NonfatalExplosionImages {
        image: asset_server.load("explosion_nonfatal.png"),
        atlas: texture_atlases.add(TextureAtlasLayout::from_grid(
            Vec2::new(64., 64.),
            6,
            1,
            None,
            None,
        )),
    })
}

pub fn handle_explosions(
    mut commands: Commands,
    unsetup_explosions: Query<(Entity, &Transform), With<ExplosionMarker>>,
    mut setup_expolosions: Query<(Entity, &mut Explosion, &mut TextureAtlas)>,
    time: Res<Time>,
    images: Res<NonfatalExplosionImages>,
) {
    for (e, transform) in unsetup_explosions.iter() {
        let explosion_transform =
            Transform::from_xyz(0., 0., 30.).with_scale(Vec3::new(3., 3., 1.));
        if let Some(mut entity) = commands.get_entity(e) {
            entity.with_children(|parent| {
                parent
                    .spawn(Explosion {
                        time_until_next_stage: Timer::new(
                            Duration::from_millis(300),
                            bevy::time::TimerMode::Repeating,
                        ),
                    })
                    .insert(SpriteSheetBundle {
                        texture: images.image.clone(),
                        atlas: TextureAtlas {
                            layout: images.atlas.clone(),
                            index: 0,
                        },
                        transform: explosion_transform,
                        ..default()
                    });
            });
        }
        commands.entity(e).remove::<ExplosionMarker>();
    }
    for (e, mut marker, mut atlas) in setup_expolosions.iter_mut() {
        marker.time_until_next_stage.tick(time.delta());
        if marker.time_until_next_stage.just_finished() {
            if atlas.index < 5 {
                atlas.index += 1;
                if let Some(mut entity) = commands.get_entity(e) {
                    entity.despawn();
                }
            }
        }
    }
}

#[derive(Component)]
pub struct ExplosionMarker;

#[derive(Component)]
pub struct Explosion {
    time_until_next_stage: Timer,
}

fn enforce_border(
    mut commands: Commands,
    mut player: Query<(Entity, &mut Spacecraft), (With<PlayerMarker>, Without<ExplosionMarker>)>,
    mut dialogue: ResMut<Dialogue>,
    mut score: ResMut<PlayerScore>,
) {
    if let Ok((entity, mut player)) = player.get_single_mut() {
        let dist = player.position.distance(Vec2::new(0., 0.));
        if dist >= 10. {
            if dist >= 15. {
                commands.entity(entity).insert(ExplosionMarker);
                player.collide(100, false, &mut *score);
            }
            dialogue.set_text("Captain! If we go much further out, we'll explode.".to_string());
            dialogue.show();
        } else {
            dialogue.hide()
        }
    }
}

fn kill_dead_ships(
    mut commands: Commands,
    ships: Query<
        (Entity, &Spacecraft),
        (
            Without<ExplosionMarker>,
            Without<PlayerMarker>,
            Without<MyFateLiesInTheBalanceAndIWouldReallyAppreciateIfIfYouDidntKillMeMarker>,
        ),
    >,
    player: Query<(Entity, &Spacecraft), (Without<ExplosionMarker>, With<PlayerMarker>)>,
    mut state: ResMut<NextState<GameLifecycleState>>,
) {
    if let Ok((entity, player)) = player.get_single() {
        for (entity, ship) in ships.iter() {
            if ship.health <= 0 || ship.position.distance(player.position) >= 10. {
                commands.entity(entity).despawn_recursive();
            }
        }
        if player.health <= 0 {
            commands.entity(entity).despawn();
            state.set(GameLifecycleState::EndScreen);
        }
    }
}

fn kill_far_bullets(
    mut commands: Commands,
    bullets: Query<(Entity, &Bullet)>,
    player_pos: Query<&Spacecraft, With<PlayerMarker>>,
) {
    if let Ok(player) = player_pos.get_single() {
        for (entity, bullet) in bullets.iter() {
            if bullet.position.distance(player.position) > 2. {
                commands.entity(entity).despawn_recursive();
            }
        }
    }
}

#[derive(Component)]
pub struct SwapToShipMarker;

fn swap_ships(
    mut commands: Commands,
    swap_from: Query<Entity, With<PlayerMarker>>,
    mut swap_to: Query<(Entity, &Transform, &mut Spacecraft), With<SwapToShipMarker>>,
    mut camera: Query<&mut Transform, (With<Camera2d>, Without<Spacecraft>)>,
) {
    if let Ok((dest_entity, dest_transform, dest_spacecraft)) = swap_to.get_single_mut() {
        if let Ok(curr_entity) = swap_from.get_single() {
            commands
                .entity(curr_entity)
                .remove::<PlayerMarker>()
                .insert(NPCLogic);
            commands
                .entity(dest_entity)
                .remove::<NPCLogic>()
                .remove::<SwapToShipMarker>()
                .insert(PlayerMarker);
        }
        if let Ok(mut cam_pos) = camera.get_single_mut() {
            cam_pos.rotation = dest_transform.rotation;
            cam_pos.rotate_z(-3. * PI / 2.)
        }
    }
}

#[derive(Clone, Component, Copy, Reflect)]
pub enum ShipType {
    Ship1,
    Ship2,
    Ship3,
    Ship4,
    Ship5,
    Ship6,
}

impl ShipType {
    pub fn collider(&self) -> ColliderBundle {
        ColliderBundle {
            collider: Collider::cuboid(40., 20.),
            events: ActiveEvents::all(),
            hooks: ActiveHooks::all(),
            types: ActiveCollisionTypes::all(),
        }
    }
}

pub struct ShipProfile {
    pub max_health: i32,
    max_velocity: f32,
    shield_recharge_time: Duration,
    gun_reload_time: Duration,
    shots: i32,
    base_bullet_velocity: f32,
    relative_scale: f32,
}

impl ShipProfile {
    pub fn from_type(ship_type: ShipType) -> Self {
        match ship_type {
            ShipType::Ship1 => ShipProfile {
                max_health: 2,
                max_velocity: MAX_VELOCITY,
                shield_recharge_time: Duration::from_secs(8),
                gun_reload_time: Duration::from_millis(1000),
                shots: 1,
                base_bullet_velocity: BULLET_SPEED,
                relative_scale: 1.,
            },
            ShipType::Ship2 => ShipProfile {
                max_health: 3,
                max_velocity: MAX_VELOCITY * 1.3,
                shield_recharge_time: Duration::from_secs(6),
                gun_reload_time: Duration::from_millis(1300),
                shots: 2,
                base_bullet_velocity: BULLET_SPEED * 0.9,
                relative_scale: 1.2,
            },
            ShipType::Ship3 => ShipProfile {
                max_health: 5,
                max_velocity: MAX_VELOCITY * 1.7,
                shield_recharge_time: Duration::from_secs(6),
                gun_reload_time: Duration::from_millis(600),
                shots: 1,
                base_bullet_velocity: BULLET_SPEED * 1.3,
                relative_scale: 1.4,
            },
            ShipType::Ship4 => ShipProfile {
                max_health: 6,
                max_velocity: MAX_VELOCITY * 1.4,
                shield_recharge_time: Duration::from_secs(5),
                gun_reload_time: Duration::from_millis(1000),
                shots: 3,
                base_bullet_velocity: BULLET_SPEED * 1.,
                relative_scale: 1.6,
            },
            ShipType::Ship5 => ShipProfile {
                max_health: 7,
                max_velocity: MAX_VELOCITY * 2.3,
                shield_recharge_time: Duration::from_secs(3),
                gun_reload_time: Duration::from_millis(1400),
                shots: 1,
                base_bullet_velocity: BULLET_SPEED * 3.,
                relative_scale: 1.8,
            },
            ShipType::Ship6 => ShipProfile {
                max_health: 10,
                max_velocity: MAX_VELOCITY * 2.,
                shield_recharge_time: Duration::from_secs(2),
                gun_reload_time: Duration::from_millis(400),
                shots: 3,
                base_bullet_velocity: BULLET_SPEED * 2.,
                relative_scale: 2.4,
            },
        }
    }
}

#[derive(Resource)]
pub struct ShipTextures {
    ship_one: Handle<Image>,
    ship_two: Handle<Image>,
    ship_three: Handle<Image>,
    ship_four: Handle<Image>,
    ship_five: Handle<Image>,
    ship_six: Handle<Image>,
}

#[derive(Resource)]
pub struct ShieldRechargeTextures {
    image: Handle<Image>,
    atlas: Handle<TextureAtlasLayout>,
}

impl ShipTextures {
    pub fn texture(&self, ship_type: ShipType) -> Handle<Image> {
        match ship_type {
            ShipType::Ship1 => self.ship_one.clone(),
            ShipType::Ship2 => self.ship_two.clone(),
            ShipType::Ship3 => self.ship_three.clone(),
            ShipType::Ship4 => self.ship_four.clone(),
            ShipType::Ship5 => self.ship_five.clone(),
            ShipType::Ship6 => self.ship_six.clone(),
        }
    }
}

#[derive(Resource)]
pub struct CarryoverEnemyPoints(i32);

pub fn spawn_ships(
    mut commands: Commands,
    enemies: Query<&Spacecraft, (Without<PlayerMarker>, Without<Captured>)>,
    player: Query<&Spacecraft, With<PlayerMarker>>,
    score: Res<PlayerScore>,
    mut spawn_points: ResMut<CarryoverEnemyPoints>,
    textures: Res<ShipTextures>,
) {
    if let Ok(player) = player.get_single() {
        spawn_points.0 += (0.4 * player.position.distance(Vec2::new(0., 0.))
            + (score.survived_time.elapsed().as_secs_f32() / 20.)
            + (score.score as f32 / 50.))
            .floor() as i32
            - points_currently_deployed(enemies.iter().map(|s| &s.ship_type).collect::<Vec<_>>());
        loop {
            let next_ship =
                take_ship_stock(enemies.iter().map(|s| &s.ship_type).collect::<Vec<_>>());
            let points_req = points_for_ship(&next_ship);
            if spawn_points.0 > points_req {
                spawn_enemy(&mut commands, player.position, next_ship, &textures);
                spawn_points.0 -= points_req;
            } else {
                break;
            }
        }
    }
}

fn points_currently_deployed(ships: Vec<&ShipType>) -> i32 {
    ships.iter().map(|s| points_for_ship(s)).count() as i32
}

fn points_for_ship(ship: &ShipType) -> i32 {
    match ship {
        ShipType::Ship1 => 3,
        ShipType::Ship2 => 7,
        ShipType::Ship3 => 15,
        ShipType::Ship4 => 21,
        ShipType::Ship5 => 31,
        ShipType::Ship6 => 50,
    }
}

fn take_ship_stock(ships: Vec<&ShipType>) -> ShipType {
    let (mut t1, mut t2, mut t3, mut t4, mut t5, mut t6) = (0., 0., 0., 0., 0., 0.);
    for ship in ships.iter() {
        match ship {
            ShipType::Ship1 => t1 += 1.,
            ShipType::Ship2 => t2 += 1.,
            ShipType::Ship3 => t3 += 1.,
            ShipType::Ship4 => t3 += 1.,
            ShipType::Ship5 => t4 += 1.,
            ShipType::Ship6 => t5 += 1.,
        };
    }
    let count = ships.len() as f32;
    t1 /= count;
    t2 /= count;
    t3 /= count;
    t4 /= count;
    t5 /= count;
    t6 /= count;
    t1 -= 0.44;
    t2 -= 0.25;
    t3 -= 0.15;
    t4 -= 0.1;
    t5 -= 0.05;
    t6 -= 0.01;
    let mut types = [
        (ShipType::Ship1, t1),
        (ShipType::Ship2, t2),
        (ShipType::Ship3, t3),
        (ShipType::Ship4, t4),
        (ShipType::Ship5, t5),
        (ShipType::Ship6, t6),
    ];
    types.sort_by(|(_, a), (_, b)| a.total_cmp(b));
    types[0].0
}

fn temp_choose_type(x: usize) -> ShipType {
    if x < 2 {
        ShipType::Ship1
    } else if x < 4 {
        ShipType::Ship2
    } else if x < 6 {
        ShipType::Ship3
    } else if x < 9 {
        ShipType::Ship4
    } else if x < 13 {
        ShipType::Ship5
    } else {
        ShipType::Ship6
    }
}

fn update_score(time: Res<Time>, mut score: ResMut<PlayerScore>) {
    score.add_score_timer.tick(time.delta());
    if score.add_score_timer.just_finished() {
        score.score += 5;
        score.add_score_timer.reset();
    }
}

#[derive(Resource)]
pub struct PlayerScore {
    pub score: u32,
    pub add_score_timer: Timer,
    pub survived_time: Instant,
}
