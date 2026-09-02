//! Capture du flux micro (`cpal`) : rééchantillonnage 16 kHz mono, filtrage
//! des silences via `vad::is_speech`, et diffusion des frames PCM vers un
//! canal partagé consommé plus tard par `asr` (Spec_Backend_Desktop.md
//! §1.3, non branché pour l'instant).
//!
//! Le device et le `cpal::Stream` sont construits et possédés entièrement
//! par un thread dédié (jamais déplacés vers un autre thread, à l'image de
//! `hotkey::HotkeyManager` pour `GlobalHotKeyManager`) : start/stop se fait
//! via `.play()`/`.pause()` sur ce même thread, piloté par un canal de
//! commandes, pour rester sous la barre des 100 ms au déclenchement du
//! hotkey (pas de réouverture du device à chaque bascule).

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use crossbeam_channel::{bounded, unbounded, Receiver, Sender};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Emitter};

use super::vad;

const TARGET_SAMPLE_RATE: u32 = 16_000;
/// Nombre de frames consécutives sous le seuil VAD avant de couper la
/// remontée vers l'ASR — laisse passer un creux entre deux mots (et tout le
/// début d'une prise de parole, même hésitante) plutôt que de trancher au
/// premier silence. Un callback `cpal` ≈ 10 ms, donc ~0,8 s de tolérance.
const SILENCE_FRAMES_BEFORE_CUT: u32 = 80;
/// Garde-fou anti-emballement : au delà de ~5 min d'audio sur une seule prise
/// (push-to-talk : impossible en pratique, un relâchement de touche envoie
/// toujours `Stop`), on cesse d'accumuler. 5 min * 16 kHz = 4,8 M échantillons.
const MAX_SESSION_SAMPLES: usize = 5 * 60 * TARGET_SAMPLE_RATE as usize;

pub enum CaptureCommand {
    Start,
    Stop,
}

/// PCM 16 kHz mono, filtré par le VAD, prêt pour l'ASR.
pub type PcmFrame = Vec<f32>;

/// Initialise le device d'entrée par défaut (paused) et démarre le thread
/// de capture. Retourne un `Sender` pour piloter start/stop (utilisé par
/// `hotkey`) et un `Receiver` des frames PCM filtrées.
///
/// Le canal PCM est **non borné** : pendant une prise, personne ne le
/// consomme (le pipeline ne draine qu'à l'arrêt), donc un canal borné
/// perdait toute la parole au-delà de sa capacité — c'était le bug « rien ne
/// se transcrit » (Session 16 suite). Une prise push-to-talk est bornée dans
/// le temps par le relâchement de la touche ; `MAX_SESSION_SAMPLES` couvre le
/// cas dégénéré.
pub fn spawn(app: AppHandle) -> Result<(Sender<CaptureCommand>, Receiver<PcmFrame>), String> {
    let (cmd_tx, cmd_rx) = unbounded::<CaptureCommand>();
    let (pcm_tx, pcm_rx) = unbounded::<PcmFrame>();
    let (ready_tx, ready_rx) = bounded::<Result<(), String>>(1);

    // Diagnostic partagé callback ↔ thread de commandes : nombre de frames PCM
    // transmises / abandonnées et pic d'amplitude sur la session en cours,
    // logués à l'arrêt pour voir d'un coup d'œil si le micro produit quelque
    // chose. `reset_diag` demande au callback de repartir de zéro (compteurs +
    // `silence_run`) au prochain `Start`.
    let frames_sent = Arc::new(AtomicU32::new(0));
    let frames_dropped = Arc::new(AtomicU32::new(0));
    let peak_bits = Arc::new(AtomicU32::new(0));
    let reset_diag = Arc::new(AtomicBool::new(false));

    let (cb_sent, cb_dropped, cb_peak, cb_reset) = (
        frames_sent.clone(),
        frames_dropped.clone(),
        peak_bits.clone(),
        reset_diag.clone(),
    );

    std::thread::spawn(move || {
        let host = cpal::default_host();
        let device = match host.default_input_device() {
            Some(d) => d,
            None => {
                let _ = ready_tx.send(Err("aucun périphérique d'entrée audio trouvé".into()));
                return;
            }
        };
        let config = match device.default_input_config() {
            Ok(c) => c,
            Err(e) => {
                let _ = ready_tx.send(Err(format!("configuration d'entrée audio invalide : {e}")));
                return;
            }
        };
        println!(
            "[audio] device d'entrée : « {} » — {} Hz, {} canal/canaux, {:?}",
            device.name().unwrap_or_else(|_| "?".into()),
            config.sample_rate().0,
            config.channels(),
            config.sample_format()
        );
        if config.sample_format() != cpal::SampleFormat::F32 {
            let _ = ready_tx.send(Err(format!(
                "format d'échantillon non supporté en V1 : {:?} (F32 attendu)",
                config.sample_format()
            )));
            return;
        }

        let sample_rate = config.sample_rate().0;
        let channels = config.channels() as usize;
        let stream_config: cpal::StreamConfig = config.into();
        let resample_ratio = TARGET_SAMPLE_RATE as f32 / sample_rate as f32;

        let mut resample_pos: f32 = 0.0;
        let mut silence_run: u32 = 0;
        let mut session_samples: usize = 0;
        let mut last_level_emit = Instant::now();

        let stream = match device.build_input_stream(
            &stream_config,
            move |data: &[f32], _: &cpal::InputCallbackInfo| {
                if cb_reset.swap(false, Ordering::Relaxed) {
                    silence_run = 0;
                    session_samples = 0;
                }
                let mono: Vec<f32> = if channels <= 1 {
                    data.to_vec()
                } else {
                    data.chunks(channels)
                        .map(|frame| frame.iter().sum::<f32>() / channels as f32)
                        .collect()
                };
                if mono.len() < 2 {
                    return;
                }

                // Rééchantillonnage naïf par interpolation linéaire vers
                // 16 kHz (suffisant pour la V1 ; un vrai resampler type
                // `rubato` est une amélioration ultérieure indépendante du
                // reste du pipeline).
                let mut resampled = Vec::with_capacity((mono.len() as f32 * resample_ratio) as usize + 1);
                let mut pos = resample_pos;
                while (pos as usize) + 1 < mono.len() {
                    let i = pos as usize;
                    let frac = pos - i as f32;
                    resampled.push(mono[i] * (1.0 - frac) + mono[i + 1] * frac);
                    pos += 1.0 / resample_ratio;
                }
                resample_pos = (pos - (mono.len() - 1) as f32).max(0.0);

                if resampled.is_empty() {
                    return;
                }

                let speech = vad::is_speech(&resampled);
                silence_run = if speech { 0 } else { silence_run.saturating_add(1) };

                let peak = resampled.iter().fold(0.0f32, |m, s| m.max(s.abs()));
                cb_peak.fetch_max(peak.to_bits(), Ordering::Relaxed);
                if last_level_emit.elapsed() >= Duration::from_millis(50) {
                    last_level_emit = Instant::now();
                    let _ = app.emit("audio_level", peak.min(1.0));
                }

                // Ne transmet pas les silences longs à l'ASR (critère
                // d'acceptation §1.2) ; laisse passer les tout premiers
                // frames silencieuses pour ne pas couper le début d'un mot.
                // Canal non borné : `send` n'échoue que si le pipeline a
                // fermé le `Receiver` (jamais en pratique).
                let keep = speech || silence_run <= SILENCE_FRAMES_BEFORE_CUT;
                if keep && session_samples < MAX_SESSION_SAMPLES {
                    session_samples += resampled.len();
                    if pcm_tx.send(resampled).is_ok() {
                        cb_sent.fetch_add(1, Ordering::Relaxed);
                    } else {
                        cb_dropped.fetch_add(1, Ordering::Relaxed);
                    }
                }
            },
            |e| eprintln!("[audio] erreur stream d'entrée : {e}"),
            None,
        ) {
            Ok(s) => s,
            Err(e) => {
                let _ = ready_tx.send(Err(format!("impossible d'ouvrir le flux d'entrée audio : {e}")));
                return;
            }
        };

        if let Err(e) = stream.pause() {
            eprintln!("[audio] erreur pause initiale : {e}");
        }
        let _ = ready_tx.send(Ok(()));

        while let Ok(cmd) = cmd_rx.recv() {
            match cmd {
                CaptureCommand::Start => {
                    frames_sent.store(0, Ordering::Relaxed);
                    frames_dropped.store(0, Ordering::Relaxed);
                    peak_bits.store(0, Ordering::Relaxed);
                    reset_diag.store(true, Ordering::Relaxed);
                    if let Err(e) = stream.play() {
                        eprintln!("[audio] erreur play : {e}");
                    } else {
                        println!("[audio] capture démarrée (stream.play)");
                    }
                }
                CaptureCommand::Stop => {
                    if let Err(e) = stream.pause() {
                        eprintln!("[audio] erreur pause : {e}");
                    }
                    let sent = frames_sent.load(Ordering::Relaxed);
                    let peak = f32::from_bits(peak_bits.load(Ordering::Relaxed));
                    println!(
                        "[audio] capture arrêtée : {sent} frames transmises, pic d'amplitude {peak:.4} (seuil VAD {:.3})",
                        vad::silence_threshold()
                    );
                    if sent == 0 {
                        eprintln!("[audio] AUCUNE frame transmise — micro muet/coupé, mauvais périphérique par défaut, ou permission micro refusée par Windows.");
                    }
                }
            }
        }
        // `stream` reste vivant jusqu'ici (fin de vie du thread = fin du
        // process, `cmd_tx` n'est jamais droppé avant).
    });

    match ready_rx.recv() {
        Ok(Ok(())) => Ok((cmd_tx, pcm_rx)),
        Ok(Err(e)) => Err(e),
        Err(_) => Err("le thread de capture audio s'est arrêté avant initialisation".into()),
    }
}
