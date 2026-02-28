use serde::Serialize;
use std::cmp::Ordering;
use std::collections::{HashMap, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Instant;

const LOOP_SAMPLE_CAP: usize = 12_000; // ~10 minutes at 20Hz
const ROOM_TICK_SAMPLE_CAP: usize = 24_000;
const PER_ROOM_SAMPLE_CAP: usize = 4_000;
const AUTOSAVE_SAMPLE_CAP: usize = 2_048;
const HANDLER_SAMPLE_CAP: usize = 20_000;
const WS_SEND_SAMPLE_CAP: usize = 20_000;
const SPIKE_SAMPLE_CAP: usize = 500;
const MAX_TRACKED_ROOMS: usize = 128;

const TICK_SLOW_THRESHOLD_MS: f64 = 50.0;
const HANDLER_SLOW_THRESHOLD_MS: f64 = 20.0;
const WS_SEND_SLOW_THRESHOLD_MS: f64 = 50.0;
const AUTOSAVE_SLOW_THRESHOLD_MS: f64 = 250.0;

#[derive(Clone)]
pub struct PerfMetrics {
    started_at: Instant,
    inner: Arc<Mutex<PerfState>>,
}

struct PerfState {
    tick_loop_ms: VecDeque<f64>,
    room_tick_ms: VecDeque<f64>,
    autosave_total_ms: VecDeque<f64>,
    autosave_snapshot_ms: VecDeque<f64>,
    autosave_write_ms: VecDeque<f64>,
    handler_ms: VecDeque<f64>,
    ws_send_ms: VecDeque<f64>,
    per_room: HashMap<String, RoomSamples>,
    counters: PerfCounters,
    recent_spikes: VecDeque<PerfSpike>,
}

struct RoomSamples {
    last_seen: Instant,
    samples: VecDeque<f64>,
}

#[derive(Clone, Copy, Default, Serialize)]
pub struct PerfCounters {
    pub tick_loop_overruns: u64,
    pub slow_room_ticks: u64,
    pub slow_autosaves: u64,
    pub slow_handlers: u64,
    pub slow_ws_sends: u64,
    pub movement_attempts: u64,
    pub movement_rejections: u64,
    pub movement_rejections_tile_blocked: u64,
    pub movement_rejections_player_blocked: u64,
    pub movement_rejections_npc_blocked: u64,
    pub movement_rejections_chair_blocked: u64,
    pub movement_rejections_arena_blocked: u64,
    pub movement_stale_packets_ignored: u64,
    pub movement_seq_gap_events: u64,
    pub movement_input_gap_events: u64,
    pub movement_stale_intent_clears: u64,
    pub state_sync_send_attempts: u64,
    pub state_sync_capacity_skips: u64,
    pub state_sync_try_send_drops: u64,
    pub state_sync_full_sends: u64,
    pub state_sync_delta_sends: u64,
    pub state_sync_fallback_self_only_sends: u64,
    pub state_sync_raw_bytes: u64,
    pub state_sync_wire_bytes: u64,
}

#[derive(Clone, Serialize)]
pub struct PerfSpike {
    pub timestamp: String,
    pub metric: String,
    pub value_ms: f64,
    pub context: String,
}

#[derive(Default, Clone, Serialize)]
pub struct SampleSummary {
    pub samples: usize,
    pub avg_ms: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
    pub latest_ms: f64,
}

#[derive(Clone, Serialize)]
pub struct RoomPerfSummary {
    pub room: String,
    pub samples: usize,
    pub avg_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
    pub latest_ms: f64,
}

#[derive(Clone, Serialize)]
pub struct PerfSnapshot {
    pub timestamp: String,
    pub uptime_seconds: u64,
    pub thresholds_ms: PerfThresholds,
    pub counters: PerfCounters,
    pub derived_rates: PerfDerivedRates,
    pub tick_loop_ms: SampleSummary,
    pub room_tick_ms: SampleSummary,
    pub autosave_total_ms: SampleSummary,
    pub autosave_snapshot_ms: SampleSummary,
    pub autosave_write_ms: SampleSummary,
    pub handler_ms: SampleSummary,
    pub ws_send_ms: SampleSummary,
    pub top_rooms: Vec<RoomPerfSummary>,
    pub recent_spikes: Vec<PerfSpike>,
}

#[derive(Clone, Serialize)]
pub struct PerfThresholds {
    pub tick_slow: f64,
    pub autosave_slow: f64,
    pub handler_slow: f64,
    pub ws_send_slow: f64,
}

#[derive(Clone, Serialize)]
pub struct PerfDerivedRates {
    pub movement_reject_rate_pct: f64,
    pub movement_reject_tile_share_pct: f64,
    pub movement_reject_player_share_pct: f64,
    pub movement_reject_npc_share_pct: f64,
    pub movement_reject_chair_share_pct: f64,
    pub movement_reject_arena_share_pct: f64,
    pub movement_stale_packet_rate_pct: f64,
    pub movement_seq_gap_rate_pct: f64,
    pub movement_input_gap_rate_pct: f64,
    pub movement_stale_intent_clear_rate_pct: f64,
    pub state_sync_drop_rate_pct: f64,
    pub state_sync_capacity_skip_rate_pct: f64,
    pub state_sync_full_share_pct: f64,
    pub state_sync_delta_share_pct: f64,
    pub state_sync_wire_vs_raw_pct: f64,
}

impl PerfMetrics {
    pub fn new() -> Self {
        Self {
            started_at: Instant::now(),
            inner: Arc::new(Mutex::new(PerfState {
                tick_loop_ms: VecDeque::with_capacity(LOOP_SAMPLE_CAP),
                room_tick_ms: VecDeque::with_capacity(ROOM_TICK_SAMPLE_CAP),
                autosave_total_ms: VecDeque::with_capacity(AUTOSAVE_SAMPLE_CAP),
                autosave_snapshot_ms: VecDeque::with_capacity(AUTOSAVE_SAMPLE_CAP),
                autosave_write_ms: VecDeque::with_capacity(AUTOSAVE_SAMPLE_CAP),
                handler_ms: VecDeque::with_capacity(HANDLER_SAMPLE_CAP),
                ws_send_ms: VecDeque::with_capacity(WS_SEND_SAMPLE_CAP),
                per_room: HashMap::new(),
                counters: PerfCounters::default(),
                recent_spikes: VecDeque::with_capacity(SPIKE_SAMPLE_CAP),
            })),
        }
    }

    pub fn record_tick_loop(&self, duration_ms: f64, room_count: usize) {
        let mut inner = self.inner.lock().unwrap();
        push_capped(&mut inner.tick_loop_ms, duration_ms, LOOP_SAMPLE_CAP);

        if duration_ms > TICK_SLOW_THRESHOLD_MS {
            inner.counters.tick_loop_overruns += 1;
            push_capped(
                &mut inner.recent_spikes,
                PerfSpike {
                    timestamp: now_rfc3339(),
                    metric: "tick_loop".to_string(),
                    value_ms: duration_ms,
                    context: format!("rooms={}", room_count),
                },
                SPIKE_SAMPLE_CAP,
            );
        }
    }

    pub fn record_room_tick(&self, room_name: &str, duration_ms: f64) {
        let mut inner = self.inner.lock().unwrap();
        push_capped(&mut inner.room_tick_ms, duration_ms, ROOM_TICK_SAMPLE_CAP);

        let now = Instant::now();
        let samples = inner
            .per_room
            .entry(room_name.to_string())
            .or_insert_with(|| RoomSamples {
                last_seen: now,
                samples: VecDeque::with_capacity(PER_ROOM_SAMPLE_CAP),
            });
        samples.last_seen = now;
        push_capped(&mut samples.samples, duration_ms, PER_ROOM_SAMPLE_CAP);

        prune_rooms_if_needed(&mut inner.per_room);

        if duration_ms > TICK_SLOW_THRESHOLD_MS {
            inner.counters.slow_room_ticks += 1;
            push_capped(
                &mut inner.recent_spikes,
                PerfSpike {
                    timestamp: now_rfc3339(),
                    metric: "room_tick".to_string(),
                    value_ms: duration_ms,
                    context: format!("room={}", room_name),
                },
                SPIKE_SAMPLE_CAP,
            );
        }
    }

    pub fn record_autosave(
        &self,
        snapshot_phase_ms: u128,
        write_phase_ms: u128,
        total_ms: u128,
        room_count: usize,
        snapshot_count: usize,
    ) {
        let snapshot_ms = snapshot_phase_ms as f64;
        let write_ms = write_phase_ms as f64;
        let total_ms = total_ms as f64;

        let mut inner = self.inner.lock().unwrap();
        push_capped(
            &mut inner.autosave_snapshot_ms,
            snapshot_ms,
            AUTOSAVE_SAMPLE_CAP,
        );
        push_capped(&mut inner.autosave_write_ms, write_ms, AUTOSAVE_SAMPLE_CAP);
        push_capped(&mut inner.autosave_total_ms, total_ms, AUTOSAVE_SAMPLE_CAP);

        if total_ms > AUTOSAVE_SLOW_THRESHOLD_MS {
            inner.counters.slow_autosaves += 1;
            push_capped(
                &mut inner.recent_spikes,
                PerfSpike {
                    timestamp: now_rfc3339(),
                    metric: "autosave_total".to_string(),
                    value_ms: total_ms,
                    context: format!("rooms={} snapshots={}", room_count, snapshot_count),
                },
                SPIKE_SAMPLE_CAP,
            );
        }
    }

    pub fn record_handler(&self, msg_name: &str, duration_ms: f64) {
        let mut inner = self.inner.lock().unwrap();
        push_capped(&mut inner.handler_ms, duration_ms, HANDLER_SAMPLE_CAP);

        if duration_ms > HANDLER_SLOW_THRESHOLD_MS {
            inner.counters.slow_handlers += 1;
            push_capped(
                &mut inner.recent_spikes,
                PerfSpike {
                    timestamp: now_rfc3339(),
                    metric: "handler".to_string(),
                    value_ms: duration_ms,
                    context: format!("msg={}", msg_name),
                },
                SPIKE_SAMPLE_CAP,
            );
        }
    }

    pub fn record_ws_send(&self, path: &str, duration_ms: f64, bytes: usize) {
        let mut inner = self.inner.lock().unwrap();
        push_capped(&mut inner.ws_send_ms, duration_ms, WS_SEND_SAMPLE_CAP);

        if duration_ms > WS_SEND_SLOW_THRESHOLD_MS {
            inner.counters.slow_ws_sends += 1;
            push_capped(
                &mut inner.recent_spikes,
                PerfSpike {
                    timestamp: now_rfc3339(),
                    metric: "ws_send".to_string(),
                    value_ms: duration_ms,
                    context: format!("path={} bytes={}", path, bytes),
                },
                SPIKE_SAMPLE_CAP,
            );
        }
    }

    pub fn record_movement(
        &self,
        attempts: usize,
        rejections: usize,
        rejected_tile_blocked: usize,
        rejected_player_blocked: usize,
        rejected_npc_blocked: usize,
        rejected_chair_blocked: usize,
        rejected_arena_blocked: usize,
    ) {
        if attempts == 0 {
            return;
        }

        let mut inner = self.inner.lock().unwrap();
        inner.counters.movement_attempts += attempts as u64;
        inner.counters.movement_rejections += rejections as u64;
        inner.counters.movement_rejections_tile_blocked += rejected_tile_blocked as u64;
        inner.counters.movement_rejections_player_blocked += rejected_player_blocked as u64;
        inner.counters.movement_rejections_npc_blocked += rejected_npc_blocked as u64;
        inner.counters.movement_rejections_chair_blocked += rejected_chair_blocked as u64;
        inner.counters.movement_rejections_arena_blocked += rejected_arena_blocked as u64;
    }

    pub fn record_movement_anomalies(
        &self,
        stale_packets_ignored: usize,
        seq_gap_events: usize,
        input_gap_events: usize,
        stale_intent_clears: usize,
    ) {
        if stale_packets_ignored == 0
            && seq_gap_events == 0
            && input_gap_events == 0
            && stale_intent_clears == 0
        {
            return;
        }

        let mut inner = self.inner.lock().unwrap();
        inner.counters.movement_stale_packets_ignored += stale_packets_ignored as u64;
        inner.counters.movement_seq_gap_events += seq_gap_events as u64;
        inner.counters.movement_input_gap_events += input_gap_events as u64;
        inner.counters.movement_stale_intent_clears += stale_intent_clears as u64;
    }

    pub fn record_state_sync(
        &self,
        send_attempts: usize,
        capacity_skips: usize,
        try_send_drops: usize,
        full_sends: usize,
        delta_sends: usize,
        fallback_self_only_sends: usize,
        raw_bytes: usize,
        wire_bytes: usize,
    ) {
        if send_attempts == 0
            && capacity_skips == 0
            && try_send_drops == 0
            && full_sends == 0
            && delta_sends == 0
            && fallback_self_only_sends == 0
            && raw_bytes == 0
            && wire_bytes == 0
        {
            return;
        }

        let mut inner = self.inner.lock().unwrap();
        inner.counters.state_sync_send_attempts += send_attempts as u64;
        inner.counters.state_sync_capacity_skips += capacity_skips as u64;
        inner.counters.state_sync_try_send_drops += try_send_drops as u64;
        inner.counters.state_sync_full_sends += full_sends as u64;
        inner.counters.state_sync_delta_sends += delta_sends as u64;
        inner.counters.state_sync_fallback_self_only_sends += fallback_self_only_sends as u64;
        inner.counters.state_sync_raw_bytes += raw_bytes as u64;
        inner.counters.state_sync_wire_bytes += wire_bytes as u64;
    }

    pub fn snapshot(&self, top_rooms: usize, spikes: usize) -> PerfSnapshot {
        let inner = self.inner.lock().unwrap();

        let mut top_room_summaries: Vec<RoomPerfSummary> = inner
            .per_room
            .iter()
            .filter_map(|(room, samples)| {
                let summary = summarize(&samples.samples);
                if summary.samples == 0 {
                    return None;
                }

                Some(RoomPerfSummary {
                    room: room.clone(),
                    samples: summary.samples,
                    avg_ms: summary.avg_ms,
                    p95_ms: summary.p95_ms,
                    max_ms: summary.max_ms,
                    latest_ms: summary.latest_ms,
                })
            })
            .collect();

        top_room_summaries.sort_by(|a, b| {
            b.p95_ms
                .partial_cmp(&a.p95_ms)
                .unwrap_or(Ordering::Equal)
                .then_with(|| b.max_ms.partial_cmp(&a.max_ms).unwrap_or(Ordering::Equal))
        });
        top_room_summaries.truncate(top_rooms);

        let recent_spikes: Vec<PerfSpike> = inner
            .recent_spikes
            .iter()
            .rev()
            .take(spikes)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect();

        PerfSnapshot {
            timestamp: now_rfc3339(),
            uptime_seconds: self.started_at.elapsed().as_secs(),
            thresholds_ms: PerfThresholds {
                tick_slow: TICK_SLOW_THRESHOLD_MS,
                autosave_slow: AUTOSAVE_SLOW_THRESHOLD_MS,
                handler_slow: HANDLER_SLOW_THRESHOLD_MS,
                ws_send_slow: WS_SEND_SLOW_THRESHOLD_MS,
            },
            counters: inner.counters,
            derived_rates: PerfDerivedRates {
                movement_reject_rate_pct: percent(
                    inner.counters.movement_rejections,
                    inner.counters.movement_attempts,
                ),
                movement_reject_tile_share_pct: percent(
                    inner.counters.movement_rejections_tile_blocked,
                    inner.counters.movement_rejections,
                ),
                movement_reject_player_share_pct: percent(
                    inner.counters.movement_rejections_player_blocked,
                    inner.counters.movement_rejections,
                ),
                movement_reject_npc_share_pct: percent(
                    inner.counters.movement_rejections_npc_blocked,
                    inner.counters.movement_rejections,
                ),
                movement_reject_chair_share_pct: percent(
                    inner.counters.movement_rejections_chair_blocked,
                    inner.counters.movement_rejections,
                ),
                movement_reject_arena_share_pct: percent(
                    inner.counters.movement_rejections_arena_blocked,
                    inner.counters.movement_rejections,
                ),
                movement_stale_packet_rate_pct: percent(
                    inner.counters.movement_stale_packets_ignored,
                    inner.counters.movement_attempts,
                ),
                movement_seq_gap_rate_pct: percent(
                    inner.counters.movement_seq_gap_events,
                    inner.counters.movement_attempts,
                ),
                movement_input_gap_rate_pct: percent(
                    inner.counters.movement_input_gap_events,
                    inner.counters.movement_attempts,
                ),
                movement_stale_intent_clear_rate_pct: percent(
                    inner.counters.movement_stale_intent_clears,
                    inner.counters.movement_attempts,
                ),
                state_sync_drop_rate_pct: percent(
                    inner.counters.state_sync_try_send_drops,
                    inner.counters.state_sync_send_attempts,
                ),
                state_sync_capacity_skip_rate_pct: percent(
                    inner.counters.state_sync_capacity_skips,
                    inner.counters.state_sync_send_attempts
                        + inner.counters.state_sync_capacity_skips,
                ),
                state_sync_full_share_pct: percent(
                    inner.counters.state_sync_full_sends,
                    inner.counters.state_sync_send_attempts,
                ),
                state_sync_delta_share_pct: percent(
                    inner.counters.state_sync_delta_sends,
                    inner.counters.state_sync_send_attempts,
                ),
                state_sync_wire_vs_raw_pct: percent(
                    inner.counters.state_sync_wire_bytes,
                    inner.counters.state_sync_raw_bytes,
                ),
            },
            tick_loop_ms: summarize(&inner.tick_loop_ms),
            room_tick_ms: summarize(&inner.room_tick_ms),
            autosave_total_ms: summarize(&inner.autosave_total_ms),
            autosave_snapshot_ms: summarize(&inner.autosave_snapshot_ms),
            autosave_write_ms: summarize(&inner.autosave_write_ms),
            handler_ms: summarize(&inner.handler_ms),
            ws_send_ms: summarize(&inner.ws_send_ms),
            top_rooms: top_room_summaries,
            recent_spikes,
        }
    }
}

fn push_capped<T>(queue: &mut VecDeque<T>, value: T, max_size: usize) {
    if queue.len() >= max_size {
        queue.pop_front();
    }
    queue.push_back(value);
}

fn prune_rooms_if_needed(per_room: &mut HashMap<String, RoomSamples>) {
    if per_room.len() <= MAX_TRACKED_ROOMS {
        return;
    }

    if let Some((oldest_name, _)) = per_room
        .iter()
        .min_by_key(|(_, room_samples)| room_samples.last_seen)
        .map(|(name, samples)| (name.clone(), samples.last_seen))
    {
        per_room.remove(&oldest_name);
    }
}

fn now_rfc3339() -> String {
    chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true)
}

fn summarize(samples: &VecDeque<f64>) -> SampleSummary {
    if samples.is_empty() {
        return SampleSummary::default();
    }

    let mut sorted = samples.iter().copied().collect::<Vec<_>>();
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));

    let sample_count = sorted.len();
    let sum: f64 = sorted.iter().sum();

    SampleSummary {
        samples: sample_count,
        avg_ms: round2(sum / sample_count as f64),
        p50_ms: round2(percentile(&sorted, 0.50)),
        p95_ms: round2(percentile(&sorted, 0.95)),
        p99_ms: round2(percentile(&sorted, 0.99)),
        max_ms: round2(*sorted.last().unwrap_or(&0.0)),
        latest_ms: round2(*samples.back().unwrap_or(&0.0)),
    }
}

fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[rank.min(sorted.len() - 1)]
}

fn round2(value: f64) -> f64 {
    (value * 100.0).round() / 100.0
}

fn percent(num: u64, den: u64) -> f64 {
    if den == 0 {
        return 0.0;
    }
    round2((num as f64 * 100.0) / den as f64)
}

#[cfg(test)]
mod tests {
    use super::summarize;
    use std::collections::VecDeque;

    #[test]
    fn summarize_computes_percentiles() {
        let mut samples = VecDeque::new();
        for i in 1..=100 {
            samples.push_back(i as f64);
        }
        let summary = summarize(&samples);

        assert_eq!(summary.samples, 100);
        assert_eq!(summary.p50_ms, 51.0);
        assert_eq!(summary.p95_ms, 95.0);
        assert_eq!(summary.max_ms, 100.0);
        assert_eq!(summary.latest_ms, 100.0);
    }
}
