use super::{GameRoom, Player};
use crate::content::ContentRegistries;
use crate::instance::InstanceManager;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::{RwLock, mpsc};

const LOAD_TEST_PLAYERS: usize = 128;
const LOAD_TEST_TICKS: usize = 100;

fn server_path(relative: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

async fn build_room() -> GameRoom {
    let data_dir = server_path("data");
    let content = Arc::new(
        ContentRegistries::load(&data_dir, &server_path("maps"))
            .await
            .expect("load test content"),
    );

    GameRoom::new(
        "load_test",
        content,
        Arc::new(RwLock::new(HashMap::new())),
        Arc::new(InstanceManager::new()),
        None,
    )
    .await
}

#[tokio::test]
async fn production_content_registries_load() {
    let data_dir = server_path("data");
    ContentRegistries::load(&data_dir, &server_path("maps"))
        .await
        .expect("authoritative content");
}

fn percentile(sorted: &[f64], percentile: f64) -> f64 {
    let index = ((sorted.len() - 1) as f64 * percentile).round() as usize;
    sorted[index.min(sorted.len() - 1)]
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "release-mode capacity test; run explicitly with --ignored"]
async fn full_tick_stays_within_budget_for_128_players() {
    let room = build_room().await;
    let mut receivers = Vec::with_capacity(LOAD_TEST_PLAYERS);

    {
        let mut players = room.players.write().await;
        for index in 0..LOAD_TEST_PLAYERS {
            let player_id = format!("load_{index}");
            let mut player = Player::new(
                &player_id,
                &format!("Load{index}"),
                8 + (index % 16) as i32,
                2 + ((index / 16) % 8) as i32,
                "male",
                "tan",
                None,
                None,
            );
            player.active = true;
            players.insert(player_id, player);
        }
    }

    for index in 0..LOAD_TEST_PLAYERS {
        let (sender, receiver) = mpsc::channel(512);
        room.register_player_sender(&format!("load_{index}"), sender)
            .await;
        receivers.push(receiver);
    }

    for _ in 0..5 {
        room.tick().await;
    }

    let mut durations = Vec::with_capacity(LOAD_TEST_TICKS);
    for _ in 0..LOAD_TEST_TICKS {
        let started = Instant::now();
        room.tick().await;
        durations.push(started.elapsed().as_secs_f64() * 1000.0);
    }

    durations.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let average = durations.iter().sum::<f64>() / durations.len() as f64;
    let p95 = percentile(&durations, 0.95);
    let p99 = percentile(&durations, 0.99);
    let max = *durations.last().unwrap();

    println!(
        "128-player full tick: avg={average:.2}ms p95={p95:.2}ms p99={p99:.2}ms max={max:.2}ms"
    );

    assert!(
        p95 < 50.0,
        "128-player p95 tick time {p95:.2}ms exceeded the 50ms budget"
    );
    assert!(
        p99 < 50.0,
        "128-player p99 tick time {p99:.2}ms exceeded the 50ms budget"
    );

    drop(receivers);
}
