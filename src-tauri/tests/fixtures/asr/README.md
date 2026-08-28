# Fixtures de test — `asr::local`

Échantillons audio réels utilisés par le test d'intégration
`asr::local::tests::transcribes_french_and_english_samples`
(`src/asr/local.rs`, `cargo test -- --ignored`).

Source : [FLEURS](https://huggingface.co/datasets/google/fleurs) (Google),
configurations `fr_fr` et `en_us`, split `test`, licence **CC-BY-4.0**. Les
mêmes échantillons français (`fr_00`/`fr_01`/`fr_02`) ont servi au benchmark
ASR de la Session 4 (voir `../../../PROGRESS.md` et
`../../../../01_Recherche/Benchmark_ASR_FR.md`).

- `fr_00.wav`, `fr_01.wav`, `fr_02.wav` : phrases françaises complètes
  (10.2s / 23.4s / 8.3s).
- `fr_short.wav` : extrait de 3s de `fr_00.wav`, pour vérifier la latence
  sur une phrase courte (critère d'acceptation Spec_Backend_Desktop.md
  §1.3 : "sous la seconde sur machine de développement standard").
- `en_00.wav` : phrase anglaise complète (10.6s), pour le critère
  d'acceptation bilingue (le modèle Parakeet-TDT v3 est multilingue).
