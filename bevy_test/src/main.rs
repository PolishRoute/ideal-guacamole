use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use bevy::asset::{AssetIo, AssetIoError, AssetPlugin, BoxedFuture, FileAssetIo};
use bevy::prelude::*;
use bevy::tasks::IoTaskPool;
use bevy_kira_audio::AudioChannel;

fn is_game_directory(path: impl AsRef<Path>) -> bool {
    let path = path.as_ref();
    let is_game_dir = path.exists()
        && path.is_dir()
        && path.join("Scripts").exists();
    if !is_game_dir {
        println!("Path '{}' is not a valid game directory", path.display());
    }
    is_game_dir
}

fn get_game_directory() -> Option<PathBuf> {
    std::env::args_os().nth(1).map(PathBuf::from)
        .filter(|d| is_game_directory(d))
        .or_else(|| std::env::current_dir().ok())
        .filter(|d| is_game_directory(d))
}

fn main() {
    let directory = get_game_directory()
        .unwrap_or_else(|| r"C:\Users\Host\Downloads\Kanon".into());
    println!("Loading game files from '{}'", directory.display());

    App::build()
        .insert_resource(WindowDescriptor {
            title: "Madenon".to_string(),
            width: 725.,
            height: 544.,
            vsync: true,
            resizable: false,
            ..Default::default()
        })
        .insert_resource(GameState {
            engine: engine::EngineState::new(&directory),
            view: ViewState::Text(TextData {
                who: None,
                what: None,
                cursor: 0,
            }),
            sound_channel: AudioChannel::new("sound".to_string()),
            music_channel: AudioChannel::new("music".to_string()),
            steps_after_save_load: VecDeque::new(),
            background_image: Handle::default(),
            date_image: Handle::default(),
            main_image: Handle::default(),
        })
        .insert_resource(ClearColor(Color::WHITE))
        .add_plugins_with(DefaultPlugins, |group| {
            group.add_after::<AssetPlugin, _>(LegAssetPlugin(
                directory.join("SEArchive.legArchive")))
        })
        .add_plugin(bevy_kira_audio::AudioPlugin)
        .add_startup_system(setup.system())
        .add_startup_system_to_stage(StartupStage::PostStartup, scripting_system.system())
        .add_system(keyboard_input_system.system())
        .add_system(typing_system.system())
        .add_system(image_presenting_system.system())
        .run();
}

struct BackgroundImage;

struct ForegroundImage;

struct DateImage;

struct TypingTimer(Timer);

struct GameText;

fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn_bundle(OrthographicCameraBundle::new_2d());
    commands.spawn_bundle(SpriteBundle {
        transform: Transform {
            translation: Vec3::new(0.0, 0.0, 0.0),
            ..Default::default()
        },
        ..Default::default()
    }).insert(BackgroundImage);
    commands.spawn_bundle(SpriteBundle {
        transform: Transform {
            translation: Vec3::new(0.0, 0.0, 1.0),
            ..Default::default()
        },
        ..Default::default()
    }).insert(ForegroundImage);
    commands.spawn_bundle(ImageBundle {
        transform: Transform {
            translation: Vec3::new(0.0, 0.0, 2.0),
            ..Default::default()
        },
        style: Style {
            position_type: PositionType::Absolute,
            position: Rect {
                left: Val::Px(0.0),
                top: Val::Px(0.0),
                ..Default::default()
            },
            ..Default::default()
        },
        ..Default::default()
    }).insert(DateImage);
    commands.spawn_bundle(ImageBundle {
        transform: Transform {
            translation: Vec3::new(0.0, 0.0, 3.0),
            ..Default::default()
        },
        style: Style {
            position_type: PositionType::Absolute,
            position: Rect {
                left: Val::Px(28.5),
                bottom: Val::Px(28.5),
                ..Default::default()
            },
            ..Default::default()
        },
        material: materials.add(asset_server.load("frame.png").into()),
        ..Default::default()
    });
    commands.spawn().insert(TypingTimer(Timer::from_seconds(0.05, true)));
    commands.spawn_bundle(UiCameraBundle::default());
    commands.spawn_bundle(TextBundle {
        style: Style {
            align_self: AlignSelf::FlexEnd,
            position_type: PositionType::Absolute,
            position: Rect {
                top: Val::Px(400.0),
                left: Val::Px(28.5 + 10.0),
                ..Default::default()
            },
            max_size: Size::new(Val::Px(725.0 - 38.5 * 2.0), Val::Px(80.0)),
            margin: Rect::all(Val::Px(10.0)),
            flex_direction: FlexDirection::Column,
            ..Default::default()
        },
        ..Default::default()
    }).insert(GameText);
}

#[derive(Debug)]
enum ViewState {
    Choice(ChoiceData),
    Text(TextData),
}

#[derive(Debug)]
struct ChoiceData {
    selected: usize,
    choices: Vec<String>,
}

#[derive(Debug)]
struct TextData {
    who: Option<String>,
    what: Option<String>,
    cursor: usize,
}

struct GameState {
    engine: engine::EngineState,
    sound_channel: AudioChannel,
    music_channel: AudioChannel,
    view: ViewState,
    steps_after_save_load: VecDeque<engine::StepResult>,
    main_image: Handle<ColorMaterial>,
    date_image: Handle<ColorMaterial>,
    background_image: Handle<ColorMaterial>,
}

fn keyboard_input_system(
    keyboard_input: Res<Input<KeyCode>>,
    asset_server: Res<AssetServer>,
    mut state: ResMut<GameState>,
    materials: ResMut<Assets<ColorMaterial>>,
    mut text_query: Query<&mut Text, With<GameText>>,
    audio: Res<bevy_kira_audio::Audio>,
) {
    if keyboard_input.just_pressed(KeyCode::F5) {
        match state.engine.save("data.sav") {
            Ok(()) => println!("Saved!"),
            Err(e) => println!("Not saved: {}", e),
        };
        return;
    }
    if keyboard_input.just_pressed(KeyCode::F6) {
        match state.engine.load("data.sav") {
            Ok(serialized) => {
                state.steps_after_save_load = serialized.into();
                scripting_system(asset_server, state, materials, text_query, audio);
                println!("Loaded!");
            }
            Err(e) => println!("Not loaded: {}", e),
        };
        return;
    }

    let GameState { engine, view, .. } = &mut *state;
    match view {
        ViewState::Choice(choice) => {
            if keyboard_input.just_pressed(KeyCode::Down) {
                choice.selected = (choice.selected + 1) % 2;
                render_choices(&mut *text_query.single_mut().unwrap(), engine, &asset_server, choice);
            } else if keyboard_input.just_pressed(KeyCode::Up) {
                if choice.selected == 0 {
                    choice.selected = choice.choices.len() - 1;
                } else {
                    choice.selected -= 1;
                }
                render_choices(&mut *text_query.single_mut().unwrap(), engine, &asset_server, choice);
            }
        }
        ViewState::Text { .. } => {}
    }

    if keyboard_input.just_pressed(KeyCode::Space) {
        scripting_system(asset_server, state, materials, text_query, audio)
    }
}

fn scripting_system(
    asset_server: Res<AssetServer>,
    mut state: ResMut<GameState>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut text_query: Query<&mut Text, With<GameText>>,
    audio: Res<bevy_kira_audio::Audio>,
) {
    loop {
        let step = match state.steps_after_save_load.pop_front() {
            Some(step) => step,
            None => engine::step(&mut state.engine)
        };

        match step {
            engine::StepResult::Text(who, what) => {
                state.view = ViewState::Text(TextData {
                    who,
                    what: Some(what),
                    cursor: 0,
                });
                break;
            }
            engine::StepResult::Jump(file) => {
                state.engine.load_script(&file);
                state.main_image = materials.add(asset_server.load("empty.png").into());
                state.date_image = materials.add(asset_server.load("empty.png").into());
                continue;
            }
            engine::StepResult::Background(path) => {
                state.background_image = materials.add(asset_server.load(path).into());
                continue;
            }
            engine::StepResult::Image(path, engine::ImageSlot::Main, _, _) => {
                state.main_image = materials.add(asset_server.load(path).into());
                continue;
            }
            engine::StepResult::Image(path, engine::ImageSlot::Date, _, _) => {
                state.date_image = materials.add(asset_server.load(path).into());
                continue;
            }
            engine::StepResult::Choice(choices) => {
                state.view = ViewState::Choice(ChoiceData {
                    choices: choices.clone(),
                    selected: 0,
                });
                let GameState { engine, view, .. } = &mut *state;
                if let ViewState::Choice(choice) = view {
                    render_choices(&mut *text_query.single_mut().unwrap(), engine, &asset_server, choice);
                }
                break;
            }
            engine::StepResult::Sound(path) => {
                if path == "~" {
                    audio.stop_channel(&state.sound_channel);
                } else {
                    audio.play_in_channel(
                        asset_server.load(PathBuf::from(path)),
                        &state.sound_channel,
                    );
                }
            }
            engine::StepResult::Music(path) => {
                audio.stop_channel(&state.music_channel);
                if path != "~" {
                    audio.play_looped_in_channel(
                        asset_server.load(PathBuf::from(path)),
                        &state.music_channel,
                    );
                }
            }
            _ => (),
        }
    }
}

fn render_choices(
    text: &mut Text,
    state: &mut engine::EngineState,
    asset_server: &AssetServer,
    choice_state: &mut ChoiceData,
) {
    text.sections.clear();
    for (idx, choice) in choice_state.choices.as_slice().iter().enumerate() {
        text.sections.push(TextSection {
            value: choice.to_string() + "\n",
            style: TextStyle {
                font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                font_size: 20.0,
                color: match choice_state.selected == idx {
                    false => Color::WHITE,
                    true => Color::RED,
                },
            },
        });
    }
    state.set_choice(choice_state.selected);
}

fn typing_system(
    time: Res<Time>,
    mut state: ResMut<GameState>,
    asset_server: ResMut<AssetServer>,
    mut text_query: Query<&mut Text, With<GameText>>,
    mut query: Query<&mut TypingTimer>,
) {
    let mut timer = query.single_mut().unwrap();
    timer.0.tick(time.delta());
    if !timer.0.just_finished() {
        return;
    }

    if let ViewState::Text(TextData { cursor, who, what }) = &mut state.view {
        *cursor += 1;

        let mut text = text_query.single_mut().unwrap();
        text.sections.clear();
        if let Some(who) = who {
            text.sections.push(TextSection {
                value: format!("{}: ", who),
                style: TextStyle {
                    font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                    font_size: 20.0,
                    color: Color::RED,
                },
            });
        }

        if let Some(what) = what {
            text.sections.push(TextSection {
                value: what.chars().take(*cursor).collect(),
                style: TextStyle {
                    font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                    font_size: 20.0,
                    color: Color::WHITE,
                },
            });
        }
    }
}

fn image_presenting_system(
    state: Res<GameState>,
    materials: Res<Assets<ColorMaterial>>,
    textures: Res<Assets<Texture>>,
    mut color_query: QuerySet<(
        Query<&mut Handle<ColorMaterial>, With<BackgroundImage>>,
        Query<&mut Handle<ColorMaterial>, With<ForegroundImage>>,
        Query<&mut Handle<ColorMaterial>, With<DateImage>>,
    )>,
) {
    let is_texture_loaded = |handle: &Handle<ColorMaterial>| -> bool {
        materials
            .get(handle)
            .and_then(|mat| mat.texture.as_ref())
            .and_then(|tex| textures.get(tex))
            .is_some()
    };

    if is_texture_loaded(&state.background_image) {
        *color_query.q0_mut().single_mut().unwrap() = state.background_image.clone();
    }
    if is_texture_loaded(&state.main_image) {
        *color_query.q1_mut().single_mut().unwrap() = state.main_image.clone();
    }
    if is_texture_loaded(&state.date_image) {
        *color_query.q2_mut().single_mut().unwrap() = state.date_image.clone();
    }
}

struct LegArchiveLoader {
    fallback: Box<dyn AssetIo>,
    leg: Mutex<leg_archive::Archive>,
}

impl LegArchiveLoader {
    fn new(fallback: Box<dyn AssetIo>, archive_path: impl AsRef<Path>) -> Self {
        Self {
            fallback,
            leg: Mutex::new(leg_archive::load(archive_path, false).unwrap()),
        }
    }
}


impl AssetIo for LegArchiveLoader {
    fn load_path<'a>(&'a self, path: &'a Path) -> BoxedFuture<'a, Result<Vec<u8>, AssetIoError>> {
        if let Some(x) = self.leg.lock().unwrap().read(path.to_str().unwrap()) {
            return Box::pin(std::future::ready(Ok(x.into_vec())));
        }
        self.fallback.load_path(path)
    }

    fn read_directory(&self, path: &Path) -> Result<Box<dyn Iterator<Item=PathBuf>>, AssetIoError> {
        self.fallback.read_directory(path)
    }

    fn is_directory(&self, path: &Path) -> bool {
        self.fallback.is_directory(path)
    }

    fn watch_path_for_changes(&self, path: &Path) -> Result<(), AssetIoError> {
        self.fallback.watch_path_for_changes(path)
    }

    fn watch_for_changes(&self) -> Result<(), AssetIoError> {
        self.fallback.watch_for_changes()
    }
}

struct LegAssetPlugin(PathBuf);

impl Plugin for LegAssetPlugin {
    fn build(&self, app: &mut AppBuilder) {
        let task_pool = app.world().get_resource::<IoTaskPool>().unwrap().0.clone();
        app.insert_resource(
            AssetServer::new(LegArchiveLoader::new(
                Box::new(FileAssetIo::new(&"./assets")),
                &self.0,
            ), task_pool)
        );
    }
}