use bevy::{
    app::{App, PluginGroup, Update},
    asset::{AssetMetaCheck, AssetServer, Handle},
    core_pipeline::core_2d::Camera2dBundle,
    ecs::{
        component::Component,
        entity::Entity,
        query::With,
        schedule::{
            common_conditions::in_state, IntoSystemConfigs, NextState, OnEnter, OnExit,
            States,
        },
        system::{Commands, Query, Res, ResMut, Resource},
    },
    hierarchy::{BuildChildren, DespawnRecursiveExt},
    input::{keyboard::KeyCode, ButtonInput},
    prelude::default,
    render::{
        color::Color,
        texture::{Image, ImagePlugin},
    },
    text::{Text, TextSection, TextStyle},
    ui::{
        node_bundles::{ImageBundle, NodeBundle, TextBundle},
        AlignItems, FlexDirection, JustifyContent, PositionType, Style, UiImage,
        UiRect, Val, ZIndex,
    },
    window::Window,
    DefaultPlugins,
};
use dialogue::Dialogue;
use gameplay::{GameplayPlugin, PlayerScore};

pub mod dialogue;
pub mod gameplay;
pub mod ui;

fn main() {
    App::new()
        .insert_resource(AssetMetaCheck::Never)
        .insert_state(GameLifecycleState::MainMenu)
        .add_plugins(DefaultPlugins.set(ImagePlugin::default_nearest()))
        .add_plugins(GameplayPlugin)
        .add_systems(OnEnter(GameLifecycleState::MainMenu), spawn_main_menu)
        .add_systems(
            Update,
            handle_inputs.run_if(in_state(GameLifecycleState::MainMenu)),
        )
        .add_systems(OnEnter(GameLifecycleState::Tutorial), init_tutorial)
        .add_systems(
            Update,
            handle_inputs_tutorial.run_if(in_state(GameLifecycleState::Tutorial)),
        )
        .add_systems(OnExit(GameLifecycleState::Tutorial), despawn_tutorial)
        .add_systems(OnEnter(GameLifecycleState::EndScreen), spawn_end_screen)
        .add_systems(OnExit(GameLifecycleState::MainMenu), kill_main_menu)
        .run();
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, States)]
pub enum GameLifecycleState {
    MainMenu,
    Tutorial,
    Game,
    EndScreen,
}

fn spawn_main_menu(mut commands: Commands, asset_server: Res<AssetServer>, window: Query<&Window>) {
    commands.insert_resource(BackgroundPNG(asset_server.load("background.png")));
    let window = window.single();
    let side_len = f32::max(window.width(), window.height());
    commands.spawn(Camera2dBundle::default());
    let background = asset_server.load("main_menu.png");
    commands
        .spawn(ImageBundle {
            style: Style {
                width: Val::Px(side_len),
                height: Val::Px(side_len),
                ..default()
            },
            image: UiImage::new(background),
            ..default()
        })
        .insert(MainMenuMarker);
}

fn handle_inputs(
    inputs: Res<ButtonInput<KeyCode>>,
    mut state: ResMut<NextState<GameLifecycleState>>,
) {
    if inputs.pressed(KeyCode::Space) {
        state.set(GameLifecycleState::Tutorial);
    }
    if inputs.pressed(KeyCode::KeyT) {
        state.set(GameLifecycleState::Game);
    }
}

#[derive(Component)]
pub struct MainMenuMarker;

fn kill_main_menu(mut commands: Commands, to_kill: Query<Entity, With<MainMenuMarker>>) {
    for entity in to_kill.iter() {
        commands.entity(entity).despawn_recursive();
    }
}

fn spawn_end_screen(
    mut commands: Commands,
    score: Res<PlayerScore>,
    asset_server: Res<AssetServer>,
) {
    let alphbeta = asset_server.load("alphbeta.ttf");
    let jupitercrash = asset_server.load("jupiterc.ttf");
    commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                flex_direction: FlexDirection::Column,
                ..default()
            },
            background_color: Color::BLACK.into(),
            ..default()
        })
        .with_children(|parent| {
            parent.spawn(TextBundle {
                style: Style {
                    padding: UiRect::all(Val::Percent(6.)),
                    ..default()
                },
                text: Text {
                    sections: vec![TextSection {
                        value: "Game Over".to_string(),
                        style: TextStyle {
                            font: jupitercrash,
                            font_size: 72.,
                            color: Color::WHITE,
                        },
                    }],
                    ..default()
                },
                ..default()
            });

            parent.spawn(TextBundle {
                text: Text {
                    sections: vec![TextSection {
                        value: format!(
                            "Score: {}\nTime Alive: {:?}",
                            score.score,
                            score.survived_time.elapsed()
                        ),
                        style: TextStyle {
                            font: alphbeta.clone(),
                            font_size: 24.,
                            color: Color::WHITE,
                        },
                    }],
                    ..default()
                },
                ..default()
            });
            parent.spawn(TextBundle {
                style: Style {
                    position_type: PositionType::Absolute,
                    bottom: Val::Px(1.),
                    left: Val::Px(1.),
                    ..default()
                },
                text: Text {
                    sections: vec![TextSection {
                        value: format!("{} // {:?}", score.score, score.survived_time.elapsed()),
                        style: TextStyle {
                            font: alphbeta,
                            font_size: 8.,
                            color: Color::Rgba {
                                red: 5. / 255.,
                                green: 5. / 255.,
                                blue: 5. / 255.,
                                alpha: 1.,
                            },
                        },
                    }],
                    ..default()
                },
                ..default()
            });
        });
}

#[derive(Resource)]
struct TutorialDialogue {
    dialogue: Vec<String>,
    index: usize,
}

fn init_tutorial(mut commands: Commands, background: Res<BackgroundPNG>, window: Query<&Window>) {
    let dialogues = vec![
        "Captain! We're entering a dangerous situation. *Press [Enter] to navigate to the next dialogue*.",
        "Our class of ship is the weakest that we'll see on the battlefield.",
        "Luckily, we have you to save us. Use the [ARROW KEYS] to tell the engine crew where to go.",
        "[LEFT] will turn left, [RIGHT] will turn right, [UP] will increase throttle, [DOWN] will decrease it.",
        "To shoot the laser cannons, press [SPACE].",
        "If you shoot a ship, you will capture it. Then, you can do three things with it.",
        "By pressing [1], you will switch perspective to that ship, controlling it yourself.",
        "By pressing [2], you'll turn the ship into an ally, fighting for us, but without you controlling it.",
        "By pressing [3], you'll scuttle the ship where is flies, destroying it.",
        "Hopefully that might give us a chance against the bigger ships out there.",
        "There's one final thing, captain. If you press [S], we'll begin to recharge shields.",
        "But be careful! We can't move while they're charging; we're sitting ducks.",
        "Good luck, and may the stars guide us"
    ];
    let diague_string = dialogues.iter().map(|d| d.to_string()).collect::<Vec<_>>();
    commands.insert_resource(TutorialDialogue {
        dialogue: diague_string,
        index: 0,
    });
    let window = window.single();
    let side_len = f32::max(window.width(), window.height());
    commands
        .spawn(ImageBundle {
            style: Style {
                width: Val::Px(side_len * 3.),
                height: Val::Px(side_len * 3.),
                ..default()
            },
            image: UiImage::new(background.0.clone()),
            z_index: ZIndex::Global(-1),
            ..default()
        })
        .insert(TutorialBackgroundMarker);
}

#[derive(Component)]
pub struct TutorialBackgroundMarker;

#[derive(Resource)]
pub struct BackgroundPNG(pub Handle<Image>);

fn handle_inputs_tutorial(
    inputs: Res<ButtonInput<KeyCode>>,
    mut dialogue: ResMut<Dialogue>,
    mut tutorial_words: ResMut<TutorialDialogue>,
    mut state: ResMut<NextState<GameLifecycleState>>,
) {
    if inputs.just_released(KeyCode::Enter) {
        tutorial_words.index += 1;
        if tutorial_words.index >= tutorial_words.dialogue.len() {
            state.set(GameLifecycleState::Game);
            return;
        }
    }
    dialogue.show();
    dialogue.set_text(tutorial_words.dialogue[tutorial_words.index].clone());
}

fn despawn_tutorial(
    mut commands: Commands,
    mut dialogue: ResMut<Dialogue>,
    background: Query<Entity, With<TutorialBackgroundMarker>>,
) {
    dialogue.hide();
    for e in background.iter() {
        commands.entity(e).despawn();
    }
}
