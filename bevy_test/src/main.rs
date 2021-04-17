use bevy::prelude::*;
use bevy::asset::AssetLoader;

fn main() {
    App::build()
        .insert_resource(WindowDescriptor {
            title: "Madenon".to_string(),
            width: 725.,
            height: 544.,
            vsync: true,
            resizable: false,
            ..Default::default()
        })
        .add_plugins(DefaultPlugins)
        .add_startup_system(setup.system())
        .add_system(keyboard_input_system.system())
        .run();
}

struct Background;

struct Overlay;


fn setup(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    mut materials: ResMut<Assets<ColorMaterial>>,
) {
    commands.spawn_bundle(OrthographicCameraBundle::new_2d());
    commands.spawn_bundle(SpriteBundle {
        ..Default::default()
    }).insert(Background);
    commands.spawn_bundle(SpriteBundle {
        ..Default::default()
    }).insert(Image);
    commands.spawn_bundle(SpriteBundle {
        material: materials.add(asset_server.load("overlay.png").into()),
        ..Default::default()
    }).insert(Overlay);

    commands.insert_resource(GameState {
        engine: engine::EngineState::new(r"C:\Users\Host\Downloads\Kanon"),
        choice: 0,
        choices: vec![],
    });

    commands.spawn_bundle(UiCameraBundle::default());
    commands
        .spawn_bundle(TextBundle {
            style: Style {
                align_self: AlignSelf::FlexEnd,
                position_type: PositionType::Absolute,
                position: Rect {
                    top: Val::Px(440.0),
                    left: Val::Px(0.0),
                    ..Default::default()
                },
                max_size: Size::new(Val::Px(725.0), Val::Px(80.0)),
                margin: Rect::all(Val::Px(10.0)),
                flex_direction: FlexDirection::Column,
                ..Default::default()
            },
            text: Text::with_section(
                "",
                TextStyle {
                    font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                    font_size: 20.0,
                    color: Color::WHITE,
                },
                TextAlignment {
                    // vertical: VerticalAlign::Top,
                    // horizontal: HorizontalAlign::Left,
                    ..Default::default()
                },
            ),
            ..Default::default()
        }).insert(GameText);

    commands.spawn().insert(Timer::from_seconds(0.0, false));
}

struct GameText;

struct Image;

struct GameState {
    engine: engine::EngineState,
    choice: usize,
    choices: Vec<String>,
}

fn keyboard_input_system(
    keyboard_input: Res<Input<KeyCode>>,
    asset_server: Res<AssetServer>,
    mut state: ResMut<GameState>,
    mut materials: ResMut<Assets<ColorMaterial>>,
    mut text_query: Query<&mut Text, With<GameText>>,
    mut color_query: QuerySet<(
        Query<&mut Handle<ColorMaterial>, With<Background>>,
        Query<&mut Handle<ColorMaterial>, With<Image>>
    )>,
    background: Query<(Entity, With<Background>)>,
) {
    if keyboard_input.just_pressed(KeyCode::Down) {
        state.choice = (state.choice + 1) % 2;
        render_choices(&mut *text_query.single_mut().unwrap(), &mut state, asset_server);
    } else if keyboard_input.just_pressed(KeyCode::Up) {
        if state.choice == 0 {
            state.choice = state.choices.len() - 1;
        } else {
            state.choice -= 1;
        }
        render_choices(&mut *text_query.single_mut().unwrap(), &mut state, asset_server);
    } else if keyboard_input.just_pressed(KeyCode::Space) {
        next(asset_server, &mut state, &mut materials, &mut text_query, &mut color_query)
    }
}

fn next(
    asset_server: Res<AssetServer>,
    mut state: &mut ResMut<GameState>,
    materials: &mut ResMut<Assets<ColorMaterial>>,
    text_query: &mut Query<&mut Text, With<GameText>>,
    color_query: &mut QuerySet<(
        Query<&mut Handle<ColorMaterial>, With<Background>>,
        Query<&mut Handle<ColorMaterial>, With<Image>>,
    )>,
) {
    loop {
        match engine::step(&mut state.engine) {
            engine::StepResult::Text(None, what) => {
                let mut text = text_query.single_mut().unwrap();
                text.sections = vec![
                    TextSection {
                        value: what,
                        style: TextStyle {
                            font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                            font_size: 20.0,
                            color: Color::WHITE,
                        }
                    }
                ];
                break;
            }
            engine::StepResult::Text(Some(who), what) => {
                let mut text = text_query.single_mut().unwrap();
                text.sections = vec![
                    TextSection {
                        value: who + ": ",
                        style: TextStyle {
                            font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                            font_size: 20.0,
                            color: Color::RED,
                        }
                    },
                    TextSection {
                        value: what,
                        style: TextStyle {
                            font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                            font_size: 20.0,
                            color: Color::WHITE,
                        }
                    }
                ];
                break;
            }
            engine::StepResult::Jump(file) => {
                println!("// Loading script {}", &file);
                state.engine.load_script(&file);
                continue;
            }
            engine::StepResult::Background(path) => {
                *color_query.q0_mut().single_mut().unwrap() =
                    materials.add(asset_server.load(path).into());
                continue;
            }
            engine::StepResult::Image(path, x, y) => {
                *color_query.q1_mut().single_mut().unwrap() =
                    materials.add(asset_server.load(path).into());
                continue;
            }
            engine::StepResult::Choice(choices) => {
                state.choices = choices.clone();
                render_choices(&mut *text_query.single_mut().unwrap(), &mut state, asset_server);
                break;
            }
            _ => (),
        }
    }
}

fn render_choices(text: &mut Text, state: &mut GameState,
                  asset_server: Res<AssetServer>, ) {
    text.sections.drain(1..);
    for (idx, choice) in state.choices.as_slice().iter().enumerate() {
        text.sections.push(TextSection {
            value: choice.to_string() + "\n",
            style: TextStyle {
                font: asset_server.load("fonts/FiraSans-Bold.ttf"),
                font_size: 20.0,
                color: match state.choice == idx {
                    false => Color::WHITE,
                    true => Color::RED,
                },
            },
        });
    }
    state.engine.set_choice(state.choice);
}
