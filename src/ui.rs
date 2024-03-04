use bevy::{
    asset::{AssetServer, Assets, Handle},
    core::Name,
    ecs::{
        component::Component,
        entity::Entity,
        query::With,
        system::{Commands, Query, Res, ResMut, Resource},
    },
    hierarchy::{BuildChildren, ChildBuilder, Children},
    math::Vec2,
    prelude::default,
    render::{color::Color, texture::Image},
    sprite::{SpriteSheetBundle, TextureAtlas, TextureAtlasLayout},
    text::{Text, TextSection, TextStyle},
    ui::{
        node_bundles::{AtlasImageBundle, ImageBundle, NodeBundle, TextBundle},
        AlignItems, FlexDirection, JustifyContent, Node, PositionType, Style, UiImage, UiRect, Val,
    },
};

use crate::gameplay::{PlayerMarker, PlayerScore, ShipProfile, Spacecraft, MAX_VELOCITY};

#[derive(Component)]
pub struct WeaponRechargeMarker;
#[derive(Component)]
pub struct ThrottleMarker;
#[derive(Component)]
pub struct ShieldMarker;
#[derive(Component)]
pub struct ScoreMarker;

pub fn spawn_ui(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut texture_atlases: ResMut<Assets<TextureAtlasLayout>>,
) {
    let weapon_reload_image = asset_server.load("weapon_reloading_atlas.png");
    let weapon_reload_atlas = TextureAtlasLayout::from_grid(Vec2::new(31., 31.), 5, 1, None, None);
    let weapon_reload_atlas_handle = texture_atlases.add(weapon_reload_atlas);

    let throttle_image = asset_server.load("throttle_atlas.png");
    let throttle_atlas = TextureAtlasLayout::from_grid(Vec2::new(9., 40.), 8, 1, None, None);
    let throttle_atlas_handle = texture_atlases.add(throttle_atlas);

    let shield_empty = asset_server.load("shield_empty.png");
    let shield_full = asset_server.load("shield_full.png");

    let alpha_beta = asset_server.load("alphbeta.ttf");

    commands.insert_resource(ShieldImages {
        full: shield_full,
        empty: shield_empty,
    });

    commands
        .spawn(NodeBundle {
            style: Style {
                position_type: PositionType::Absolute,
                left: Val::Percent(2.),
                top: Val::Percent(2.),
                flex_direction: FlexDirection::Row,
                ..default()
            },
            ..default()
        })
        .insert(Name::new("Shields"))
        .insert(ShieldMarker);

    commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                flex_direction: FlexDirection::Row,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                ..default()
            },
            ..default()
        })
        .insert(Name::new("UI"))
        .with_children(|parent| {
            parent
                .spawn(AtlasImageBundle {
                    style: Style {
                        width: Val::Px(176.),
                        height: Val::Px(176.),
                        position_type: PositionType::Absolute,
                        left: Val::Px(10.),
                        bottom: Val::Px(10.),
                        ..default()
                    },
                    texture_atlas: weapon_reload_atlas_handle.into(),
                    image: UiImage::new(weapon_reload_image),
                    ..default()
                })
                .insert(WeaponRechargeMarker);
            parent
                .spawn(AtlasImageBundle {
                    style: Style {
                        width: Val::Px(81.),
                        height: Val::Px(342.),
                        position_type: PositionType::Absolute,
                        right: Val::Px(10.),
                        bottom: Val::Px(10.),
                        ..default()
                    },
                    texture_atlas: throttle_atlas_handle.into(),
                    image: UiImage::new(throttle_image),
                    ..default()
                })
                .insert(ThrottleMarker);
            parent
                .spawn(TextBundle {
                    style: Style {
                        position_type: PositionType::Absolute,
                        right: Val::Px(15.),
                        top: Val::Px(15.),
                        ..default()
                    },
                    text: Text {
                        sections: vec![TextSection {
                            value: "Score: XX".to_string(),
                            style: TextStyle {
                                font: alpha_beta,
                                font_size: 24.,
                                color: Color::WHITE,
                            },
                        }],
                        ..default()
                    },
                    ..default()
                })
                .insert(ScoreMarker);
        });
}

pub fn update_weapon_ui(
    mut image: Query<&mut TextureAtlas, With<WeaponRechargeMarker>>,
    ship: Query<&Spacecraft, With<PlayerMarker>>,
) {
    if let Ok(ship) = ship.get_single() {
        let index = if ship.weapon_cooldown.finished() {
            4
        } else {
            match ship.weapon_cooldown.fraction() {
                x if x >= 0. && x < 0.25 => 0,
                x if x >= 0.25 && x < 0.5 => 1,
                x if x >= 0.5 && x < 0.75 => 2,
                x if x >= 0.75 && x < 1. => 3,
                _ => 0,
            }
        };
        for mut atlas_image in &mut image {
            atlas_image.index = index;
        }
    }
}

pub fn update_throttle_ui(
    mut image: Query<&mut TextureAtlas, With<ThrottleMarker>>,
    ship: Query<&Spacecraft, With<PlayerMarker>>,
) {
    if let Ok(ship) = ship.get_single() {
        let index = ((ship.velocity / MAX_VELOCITY) * 7.).ceil() as usize;
        for mut atlas_image in &mut image {
            atlas_image.index = index;
        }
    }
}

#[derive(Resource)]
pub struct ShieldImages {
    full: Handle<Image>,
    empty: Handle<Image>,
}

pub fn update_shield_ui(
    mut commands: Commands,
    images: Res<ShieldImages>,
    ship: Query<&Spacecraft, With<PlayerMarker>>,
    shield_ui: Query<Entity, With<ShieldMarker>>,
) {
    if let Ok(entity) = shield_ui.get_single() {
        if let Ok(ship) = ship.get_single() {
            commands
                .entity(entity)
                .remove::<Children>()
                .insert(NodeBundle {
                    style: Style {
                        width: Val::Percent(100.),
                        height: Val::Percent(100.),
                        position_type: PositionType::Absolute,
                        left: Val::Px(5.),
                        top: Val::Px(5.),
                        padding: UiRect::all(Val::Px(3.)),
                        flex_direction: FlexDirection::Row,
                        justify_content: JustifyContent::Start,
                        align_items: AlignItems::Start,
                        ..default()
                    },
                    ..default()
                })
                .with_children(|parent| {
                    for i in 0..ShipProfile::from_type(ship.ship_type).max_health as i32 {
                        let image = match i < ship.health {
                            true => images.full.clone(),
                            false => images.empty.clone(),
                        };
                        parent.spawn(ImageBundle {
                            style: Style {
                                padding: UiRect::all(Val::Px(12.)),
                                width: Val::Px(54.),
                                height: Val::Px(60.),
                                ..default()
                            },
                            image: UiImage::new(image),
                            ..default()
                        });
                    }
                });
        } else {
            commands.entity(entity).despawn();
        }
    }
}

pub fn update_score_text(mut text: Query<&mut Text, With<ScoreMarker>>, score: Res<PlayerScore>) {
    if let Ok(mut text) = text.get_single_mut() {
        text.sections[0].value = format!("Score: {}", score.score)
    }
}
