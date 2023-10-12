use bevy::prelude::*;
use bevy_ecs_ldtk::{LdtkLevel, LevelSelection};
use bevy_rapier2d::prelude::Velocity;
use pecs::prelude::*;

use crate::{
    components::{Mierda, Player},
    sprites::{AnimationDirection, CharacterAnimation, FlashingTimer},
    ui::{self, UiGameOver},
    utils::CloneEntity,
};

#[derive(Event, Clone)]
pub struct PlayerAttackEvent {
    pub entity: Entity,
}

#[derive(Event, Clone)]
pub struct PlayerHitEvent {
    pub entity: Entity,
}

#[derive(Event, Clone)]
pub struct GameOverEvent;

#[derive(Event, Clone)]
pub struct MierdaHitEvent(pub Entity);

#[derive(Event, Clone)]
pub struct SpawnMierdaEvent {
    pub(crate) count: u32,
}

pub fn event_spawn_mierda(
    mut commands: Commands,
    mut ev_spawn_mierda: EventReader<SpawnMierdaEvent>,
    level_selection: Res<LevelSelection>,
    level_handles: Query<(Entity, &Handle<LdtkLevel>)>,
    level_assets: Res<Assets<LdtkLevel>>,
    _texture_atlasses: ResMut<Assets<TextureAtlas>>,
    los_mierdas: Query<(Entity, &Parent, &mut Visibility, &Mierda)>,
    levels: Query<(Entity, &Handle<LdtkLevel>)>,
) {
    for ev_spawn in ev_spawn_mierda.iter() {
        for (_, level_handle) in level_handles.iter() {
            let level = &level_assets.get(level_handle).unwrap().level;

            if level_selection.is_match(&0, level) {
                let (parent_entity, _) = levels
                    .iter()
                    .find(|(_, handle)| *handle == level_handle)
                    .unwrap();

                for _i in 0..ev_spawn.count {
                    for (mierda_entity, mierda_parent, _mierda_visibility, mierda) in
                        los_mierdas.iter()
                    {
                        if !mierda.is_dummy {
                            continue;
                        }

                        let mierda_parent = mierda_parent.get();

                        if parent_entity != mierda_parent {
                            continue;
                        }

                        let mut parent = commands.entity(mierda_parent);

                        let mut new_entity: Option<Entity> = None;
                        parent.with_children(|cm| {
                            let ne = cm.spawn_empty().id();
                            new_entity = Some(ne);
                        });

                        let new_entity = new_entity.unwrap();
                        commands.entity(new_entity).insert(Mierda {
                            is_dummy: false,
                            health: 100,
                            move_direction: Vec2::ZERO,
                            hit_at: None,
                        });

                        commands.add(CloneEntity {
                            source: mierda_entity,
                            destination: new_entity,
                        });
                    }
                }
            }
        }
    }
}

pub fn event_player_attack(
    mut ev_player_attack: EventReader<PlayerAttackEvent>,
    mut ev_mierda_hit: EventWriter<MierdaHitEvent>,
    mut q_player: Query<(Entity, &Transform, &CharacterAnimation), With<Player>>,
    mut los_mierdas: Query<(Entity, &Transform, &mut Mierda)>,
) {
    for ev in ev_player_attack.iter() {
        let (_, transform, char_animation) = q_player.get_mut(ev.entity).unwrap();

        let player_position = transform.translation;
        let player_orientation = char_animation.direction;

        // find all mierdas in range
        for (entity, mierda_transform, _) in los_mierdas.iter_mut().filter(|(_, _, m)| !m.is_dummy)
        {
            let mierda_position = mierda_transform.translation;

            let distance = player_position.distance(mierda_position);

            if distance >= 75. {
                continue;
            }

            // cause damage accrodign to player_orientation
            let is_merda_attacked = match player_orientation {
                AnimationDirection::Up => player_position.y < mierda_position.y,
                AnimationDirection::Down => player_position.y > mierda_position.y,
                AnimationDirection::Left => player_position.x > mierda_position.x,
                AnimationDirection::Right => player_position.x < mierda_position.x,
            };

            if !is_merda_attacked {
                continue;
            }

            ev_mierda_hit.send(MierdaHitEvent(entity));
        }
    }
}

pub fn event_mierda_hit(
    mut commands: Commands,
    q_player: Query<(&Transform, &Player)>,
    mut los_mierdas: Query<(Entity, &Transform, &mut Velocity, &mut Mierda)>,
    mut ev_mierda_hit: EventReader<MierdaHitEvent>,
    mut ev_mierda_spawn: EventWriter<SpawnMierdaEvent>,
) {
    for event in ev_mierda_hit.iter() {
        let los_mierdas_count = los_mierdas.iter().len();

        for (player_transform, _) in q_player.iter() {
            let player_position = player_transform.translation;

            let (mierda_entity, mierda_transform, mut mierda_velocity, mut mierda) =
                los_mierdas.get_mut(event.0).unwrap();
            let mierda_position = mierda_transform.translation;
            let vector_attack = (mierda_position - player_position).normalize();
            mierda_velocity.linvel.x += vector_attack.x * 200.;
            mierda_velocity.linvel.y += vector_attack.y * 200.;

            let timer = Timer::new(std::time::Duration::from_millis(200), TimerMode::Once);
            mierda.hit_at = Some(timer.clone());

            commands.entity(mierda_entity).insert(FlashingTimer {
                timer: timer.clone(),
            });

            // despawn mierda async
            commands
                .promise(|| (mierda_entity))
                .then(asyn!(state => {
                    state.asyn().timeout(0.3)
                }))
                .then(asyn!(state, mut commands: Commands  => {
                    commands.entity(state.value).despawn_recursive();

                }));

            if los_mierdas_count < 256 {
                ev_mierda_spawn.send(SpawnMierdaEvent { count: 2 });
            }
        }
    }
}

pub fn event_player_hit(
    mut ev_player_hit_reader: EventReader<PlayerHitEvent>,
    mut ev_game_over: EventWriter<GameOverEvent>,
    mut q_player: Query<(Entity, &mut Player)>,
    mut q_ui_healthbar: Query<(Entity, &mut Style, &ui::UiPlayerHealth)>,
) {
    for ev in ev_player_hit_reader.iter() {
        let (_, mut player) = q_player.get_mut(ev.entity).unwrap();

        if player.health < 10 {
            ev_game_over.send(GameOverEvent);
            continue;
        } else {
            player.health -= 10;

            for (_, mut style, _) in q_ui_healthbar.iter_mut() {
                style.width = Val::Percent(player.health as f32);
            }
        }
    }
}

pub fn event_game_over(
    mut ev_game_over: EventReader<GameOverEvent>,
    mut q_ui_game_over: Query<(&mut Visibility, &UiGameOver)>,
) {
    for _ in ev_game_over.iter() {
        for (mut visibility, _) in q_ui_game_over.iter_mut() {
            *visibility = Visibility::Visible;
        }
    }
}
