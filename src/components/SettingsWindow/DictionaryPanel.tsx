// Gestion du dictionnaire personnalisé (remplacements de mots/expressions).
// Stub visuel en Phase 1 — le CRUD complet relié à `storage::dictionary`
// côté Rust arrive en Phase 2 (Spec_Frontend.md §2.1). L'en-tête est rendu
// dès maintenant pour que la section existe dans le layout général (§1.3).

export function DictionaryPanel() {
  return (
    <section className="dictionary-panel flex flex-col gap-1">
      <h2 className="text-base font-semibold">Dictionnaire personnalisé</h2>
      <p className="text-sm text-muted-foreground">
        Remplacements de mots et d'expressions appliqués après la
        transcription. Disponible en phase 2.
      </p>
    </section>
  );
}
