use bevy::{
    app::{Plugin, Startup, Update},
    asset::{AssetServer, Handle},
    core::Name,
    ecs::{
        component::Component,
        query::With,
        system::{Commands, Query, Res, Resource},
    },
    hierarchy::BuildChildren,
    prelude::default,
    render::{color::Color, texture::Image, view::Visibility},
    sprite::{BorderRect, ImageScaleMode, TextureSlicer},
    text::{Text, TextStyle},
    ui::{
        node_bundles::{ButtonBundle, ImageBundle, NodeBundle, TextBundle},
        Style, UiImage, UiRect, Val,
    },
};

pub struct DialoguePlugin;

impl Plugin for DialoguePlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_systems(Startup, init_dialogue_system)
            .add_systems(Update, update_dialogue)
            .insert_resource(Dialogue::init());
    }
}

fn init_dialogue_system(mut commands: Commands, asset_server: Res<AssetServer>) {
    let dialogue_background: Handle<Image> = asset_server.load("dialogue_box.png");
    let jupiter_crash = asset_server.load("jupiterc.ttf");
    let char_spin: Handle<Image> = asset_server.load("char_spin.png");
    let dialogue_bg_slice = TextureSlicer {
        border: BorderRect {
            left: 3.,
            right: 3.,
            top: 3.,
            bottom: 3.,
        },
        center_scale_mode: bevy::sprite::SliceScaleMode::Stretch,
        sides_scale_mode: bevy::sprite::SliceScaleMode::Stretch,
        max_corner_scale: 24.,
    };

    commands
        .spawn(NodeBundle {
            style: Style {
                width: Val::Percent(100.),
                height: Val::Percent(100.),
                align_items: bevy::ui::AlignItems::Center,
                justify_content: bevy::ui::JustifyContent::Center,
                ..default()
            },
            ..default()
        })
        .insert(Name::new("Dialogue UI"))
        .insert(DialogueMarker)
        .with_children(|parent| {
            parent
                .spawn((
                    ButtonBundle {
                        style: Style {
                            position_type: bevy::ui::PositionType::Absolute,
                            bottom: Val::Px(30.),
                            width: Val::Percent(60.),
                            height: Val::Percent(25.),
                            justify_content: bevy::ui::JustifyContent::Start,
                            align_items: bevy::ui::AlignItems::Center,
                            padding: UiRect::all(Val::Percent(2.)),
                            ..default()
                        },
                        visibility: Visibility::Inherited,
                        image: UiImage::new(dialogue_background),
                        ..default()
                    },
                    ImageScaleMode::Sliced(dialogue_bg_slice.clone()),
                ))
                .with_children(|parent| {
                    parent.spawn(ImageBundle {
                        style: Style {
                            width: Val::Percent(17.),
                            height: Val::Percent(95.),
                            ..default()
                        },
                        image: UiImage::new(char_spin),
                        ..default()
                    });

                    parent.spawn(TextBundle::from_section(
                        "I'd just like to interject for a moment. What you're refering to as Linux, is in fact, GNU/Linux",
                        TextStyle {
                            font: jupiter_crash,
                            font_size: 40.,
                            color: Color::WHITE,
                        },
                    )).insert(DialogueTextMarker);
                });
        });
}

#[derive(Resource)]
pub struct Dialogue {
    visible: bool,
    contents: String,
}

#[derive(Component)]
pub struct DialogueMarker;

#[derive(Component)]
pub struct DialogueTextMarker;

impl Dialogue {
    pub fn init() -> Self {
        Self {
            visible: false,
            contents: "What you've been referring to".to_string(),
        }
    }

    pub fn show(&mut self) {
        self.visible = true;
    }

    pub fn hide(&mut self) {
        self.visible = false;
    }

    pub fn set_text(&mut self, contents: String) {
        self.contents = contents;
    }
}

pub fn update_dialogue(
    dialogue: Res<Dialogue>,
    mut vis: Query<&mut Visibility, With<DialogueMarker>>,
    mut text: Query<&mut Text, With<DialogueTextMarker>>,
) {
    if let Ok(mut vis) = vis.get_single_mut() {
        *vis = match dialogue.visible {
            true => Visibility::Visible,
            false => Visibility::Hidden,
        };
    }
    if let Ok(mut text) = text.get_single_mut() {
        text.sections[0].value = dialogue.contents.clone();
    }
}
